use {
    crate::{fun::*, types::*},
    anyhow::Context,
    async_stream::{stream, try_stream},
    futures::{self as futs, Stream, StreamExt, TryFutureExt},
    std::time::Instant,
    tokio::io::{self, AsyncReadExt, AsyncWriteExt},
    unwrap_or::unwrap_ok_or,
};

pub fn clone_dir_structure<'a>(
    dir: &'a FileMeta,
    dst: &'a FileMeta,
) -> impl Stream<Item = anyhow::Result<(Vec<FileMeta>, FileMeta)>> + 'a {
    let sm = dir.to_owned();

    stream! {
        let dm = async {
            let id = create_dir(&sm.name, &dst.id).await?;
            get_meta(&id).await
        }
        .await;

        let dm = unwrap_ok_or!(dm, e, {
            yield Err(e);
            return;
        });

        let mut src_stack = vec![sm];
        let mut dst_stack = vec![dm];

        while let (Some(sm), Some(dm)) = (src_stack.pop(), dst_stack.pop()) {
            let list = list_meta(&sm.id).await;
            let list = unwrap_ok_or!(list, e, {
                yield Err(e);
                continue;
            });

            let mut files = vec![];
            for await r in list {
                let sm = unwrap_ok_or!(r, e, {
                    yield Err(e);
                    continue;
                });

                if !matches!(sm.file_type, FileType::Dir) {
                    files.push(sm);
                    continue;
                }

                let dm = create_dir(&sm.name, &dm.id)
                    .and_then(|id| async move { get_meta(&id).await })
                    .await;

                let dm = unwrap_ok_or!(dm, e, {
                    yield Err(e);
                    continue;
                });

                src_stack.push(sm);
                dst_stack.push(dm);
            }
            yield Ok((files, dm));
        }
    }
}

pub fn dfs(file: &FileMeta) -> impl Stream<Item = anyhow::Result<FileMeta>> {
    let file = file.to_owned();

    let mut stack = vec![file];

    stream! {
        while let Some(m) = stack.pop() {
            if !matches!(m.file_type, FileType::Dir) {
                yield Ok(m);
                continue;
            }

            let list = list_meta(&m.id).await;
            let list = unwrap_ok_or!(list, e, {
                yield Ok(m);
                yield Err(e);
                continue;
            });

            yield Ok(m);

            for await r in list {
                let f = unwrap_ok_or!(r, e, {
                    yield Err(e);
                    continue;
                });

                stack.push(f);
            }
        }
    }
}

pub fn copy_file<'a>(
    src: &'a FileMeta,
    dst: &'a FileMeta,
) -> impl Stream<Item = anyhow::Result<u64>> + 'a {
    try_stream! {
        let dm = create_file(&src.name, &dst.id)
            .and_then(|id| async move { get_meta(&id).await })
            .await?;

        let (rd, wr) = futs::future::try_join(read(&src.id), write(&dm.id, true)).await?;

        let mut reader = io::BufReader::new(rd);
        let mut writer = io::BufWriter::new(wr);
        let mut buf = vec![0; 10_000_000];

        loop {
            let bytes = async {
                let bytes = reader
                    .read(&mut buf)
                    .await
                    .with_context(|| format!("Error while reading file {}", src.name))?;

                writer
                    .write_all(&buf[..bytes])
                    .await
                    .with_context(|| format!("Error while writing to file {}", dm.name))?;

                anyhow::Ok(bytes)
            }
            .await?;

            if bytes == 0 {
                break;
            }

            yield bytes as u64;
        }

        writer.flush().await?;
    }
}

pub fn copy_all<'a>(
    files: &'a [FileMeta],
    dst: &'a FileMeta,
    prog_interval: u128,
) -> impl Stream<Item = anyhow::Result<CopyProg>> + 'a {
    let mut prog = CopyProg::default();

    let mut instant = Instant::now();

    let mut cp = vec![];
    let mut cp1 = vec![];

    stream! {
        for f in files.iter() {
            if !matches!(f.file_type, FileType::Dir) {
                prog.files.total += 1;
                prog.size.total += f.size;

                if instant.elapsed().as_millis() >= prog_interval {
                    instant = Instant::now();
                    yield Ok(prog.clone());
                }

                cp.push((f, dst));
                continue;
            }

            let cps = clone_dir_structure(f, dst);

            for await r in cps {
                let (files, dir) = unwrap_ok_or!(r, e, {
                    yield Err(e);
                    continue;
                });

                cp1.push((files, dir));

                let (files, _) = cp1.last().unwrap();

                prog.files.total += files.len() as u64;
                prog.size.total += files.iter().map(|f| f.size).sum::<u64>();

                if instant.elapsed().as_millis() >= prog_interval {
                    instant = Instant::now();
                    yield Ok(prog.clone());
                }
            }
        }

        fn get_iter(val: &(Vec<FileMeta>, FileMeta)) -> impl Iterator<Item = (&FileMeta, &FileMeta)> {
            let (files, dir) = val;
            files.iter().map(move |f| (f, &*dir))
        }

        let cp = cp
            .into_iter()
            .chain(cp1.iter().flat_map(get_iter));

        'outer: for (f, d) in cp {
            prog.current.name = f.name.clone();
            prog.current.prog = Progress {
                total: f.size,
                ..Default::default()
            };

            if instant.elapsed().as_millis() >= prog_interval {
                instant = Instant::now();
                yield Ok(prog.clone());
            }

            for await r in copy_file(f, d) {
                let bytes = unwrap_ok_or!(r, e, {
                    yield Err(e);
                    continue 'outer;
                });

                prog.size.done += bytes;
                prog.current.prog.done += bytes;

                if instant.elapsed().as_millis() >= prog_interval {
                    instant = Instant::now();
                    yield Ok(prog.clone());
                }
            }

            prog.files.done += 1;

            if instant.elapsed().as_millis() >= prog_interval {
                instant = Instant::now();
                yield Ok(prog.clone());
            }
        }

        yield Ok(prog.clone());
    }
}

pub fn mv_all<'a>(
    files: &'a [FileMeta],
    dir: &'a FileMeta,
) -> impl Stream<Item = anyhow::Result<Progress>> + 'a {
    let mut prog = Progress::default();

    fn get_fut<'a>(
        fd: (&'a FileId, &'a FileId),
    ) -> impl futs::Future<Output = anyhow::Result<FileId>> + 'a {
        let (f, d) = fd;
        mv(f, d)
    }

    fn get_iter<'a>(
        files: &'a [FileMeta],
        dir: &'a FileMeta,
    ) -> impl Iterator<Item = (&'a FileId, &'a FileId)> + 'a {
        files.iter().map(|f| (&f.id, &dir.id))
    }

    let s = futs::stream::iter(get_iter(files, dir))
        .map(get_fut)
        .buffer_unordered(1000);

    stream! {
        for await r in s {
            let _ = unwrap_ok_or!(r, e, {
                yield Err(e);
                continue;
            });

            prog.done += 1;
            yield Ok(prog.clone());
        }
    }
}

pub fn delete_all(files: &[FileMeta]) -> impl Stream<Item = anyhow::Result<Progress>> + '_ {
    let mut fls = vec![];
    let mut fls1 = vec![];
    let mut drs = vec![];

    let mut prog = Progress::default();

    stream! {
        for f in files {
            if !matches!(f.file_type, FileType::Dir) {
                prog.total += 1;
                yield Ok(prog.clone());
                fls.push(f);
                continue;
            }

            for await r in dfs(f) {
                let f = unwrap_ok_or!(r, e, {
                    yield Err(e);
                    continue;
                });

                if !matches!(f.file_type, FileType::Dir) {
                    fls1.push(f);
                } else {
                    drs.push(f);
                }

                prog.total += 1;
                yield Ok(prog.clone());
            }
        }

        fn get_iter<'a>(i: Vec<&'a FileMeta>, j: &'a [FileMeta]) -> impl Iterator<Item = &'a FileMeta> {
            i.into_iter().chain(j.iter())
        }

        fn get_fut(file: &FileMeta) -> impl futs::Future<Output = anyhow::Result<bool>> + '_ {
            delete_file(&file.id)
        }

        let fls_stream = futs::stream::iter(get_iter(fls, &fls1[..]))
            .map(get_fut)
            .buffer_unordered(1000);

        let drs_stream =
            futs::stream::iter(drs.iter().rev()).then(|d| async move { delete_dir(&d.id).await });

        let del_stream = fls_stream.chain(drs_stream);

        for await r in del_stream {
            let _ = unwrap_ok_or!(r, e, {
                yield Err(e);
                continue;
            });

            prog.done += 1;
            yield Ok(prog.clone());
        }
    }
}

pub fn rename_all(rn: &[(FileMeta, String)]) -> impl Stream<Item = anyhow::Result<Progress>> + '_ {
    let mut prog = Progress::default();

    fn get_fut(rn: &(FileMeta, String)) -> impl futs::Future<Output = anyhow::Result<FileId>> + '_ {
        let (f, n) = rn;
        rename(&f.id, n)
    }

    fn get_iter(rn: &[(FileMeta, String)]) -> impl Iterator<Item = &(FileMeta, String)> + '_ {
        rn.iter()
    }

    let s = futs::stream::iter(get_iter(rn))
        .map(get_fut)
        .buffer_unordered(1000);

    stream! {
        for await r in s {
            let _ = unwrap_ok_or!(r, e, {
                yield Err(e);
                continue;
            });

            prog.done += 1;
            yield Ok(prog.clone());
        }
    }
}
