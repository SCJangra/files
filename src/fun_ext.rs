use {
    crate::{fun::*, types::*},
    anyhow::Context,
    futures::{self as futs, StreamExt, TryFutureExt},
    futures_async_stream::{stream, try_stream},
    std::task::Poll,
    tokio::io::{self, AsyncReadExt, AsyncWriteExt},
    unwrap_or::unwrap_ok_or,
};

#[try_stream(ok = u64, error = anyhow::Error)]
pub async fn copy_file<'a>(src: &'a FileMeta, dst: &'a FileMeta) {
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
}

#[stream(item = anyhow::Result<CopyProg>)]
pub async fn copy<'a>(files: &'a [FileMeta], dst: &'a FileMeta) {
    let mut prog = CopyProg::default();

    let mut cp = vec![];
    let mut cp1 = vec![];

    for f in files.iter() {
        if !matches!(f.file_type, FileType::Dir) {
            prog.files.total += 1;
            prog.size.total += f.size;

            yield Ok(prog.clone());

            cp.push((f, dst));
            continue;
        }

        let cps = clone_dir_structure(f, dst);

        #[for_await]
        for r in cps {
            let (files, dir) = unwrap_ok_or!(r, e, {
                yield Poll::Ready(Err(e));
                continue;
            });

            cp1.push((files, dir));

            let (files, _) = cp1.last().unwrap();

            prog.files.total += files.len() as u64;
            prog.size.total += files.iter().map(|f| f.size).sum::<u64>();

            yield Ok(prog.clone());
        }
    }

    fn get_iter(val: &(Vec<FileMeta>, FileMeta)) -> impl Iterator<Item = (&FileMeta, &FileMeta)> {
        let (files, dir) = val;
        files.iter().map(|f| (f, &*dir))
    }

    let cp = cp.into_iter().chain(cp1.iter().flat_map(get_iter));

    'outer: for (f, d) in cp {
        prog.current.name = f.name.clone();
        prog.current.prog = Progress {
            total: f.size,
            ..Default::default()
        };

        yield Ok(prog.clone());

        #[for_await]
        for r in copy_file(f, d) {
            let bytes = unwrap_ok_or!(r, e, {
                yield Poll::Ready(Err(e));
                continue 'outer;
            });

            prog.size.done += bytes;
            prog.current.prog.done += bytes;

            yield Ok(prog.clone());
        }

        prog.files.done += 1;

        yield Ok(prog.clone());
    }
}

#[stream(item = anyhow::Result<Progress>)]
pub async fn mv<'a>(files: &'a [FileMeta], dir: &'a FileMeta) {
    let mut prog = Progress::default();

    fn get_fut<'a>(
        fd: (&'a FileId, &'a FileId),
    ) -> impl futs::Future<Output = anyhow::Result<FileId>> + 'a {
        let (f, d) = fd;
        move_file(f, d)
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

    #[for_await]
    for r in s {
        let _ = unwrap_ok_or!(r, e, {
            yield Poll::Ready(Err(e));
            continue;
        });

        prog.done += 1;
        yield Ok(prog.clone());
    }
}

#[stream(item = anyhow::Result<(Vec<FileMeta>, FileMeta)>)]
pub async fn clone_dir_structure<'a>(dir: &'a FileMeta, dst: &'a FileMeta) {
    let sm = dir.to_owned();
    let dm = async {
        let id = create_dir(&sm.name, &dst.id).await?;
        get_meta(&id).await
    }
    .await;

    let dm = unwrap_ok_or!(dm, e, {
        yield Poll::Ready(Err(e));
        return;
    });

    let mut src_stack = vec![sm];
    let mut dst_stack = vec![dm];

    while let (Some(sm), Some(dm)) = (src_stack.pop(), dst_stack.pop()) {
        let list = list_meta(&sm.id).await;
        let list = unwrap_ok_or!(list, e, {
            yield Poll::Ready(Err(e));
            continue;
        });

        let mut files = vec![];
        #[for_await]
        for r in list {
            let sm = unwrap_ok_or!(r, e, {
                yield Poll::Ready(Err(e));
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
                yield Poll::Ready(Err(e));
                continue;
            });

            src_stack.push(sm);
            dst_stack.push(dm);
        }
        yield Ok((files, dm));
    }
}

#[allow(clippy::needless_lifetimes)]
#[stream(item = anyhow::Result<FileMeta>)]
pub async fn dfs<'a>(file: &'a FileMeta) {
    let file = file.to_owned();

    let mut stack = vec![file];

    while let Some(m) = stack.pop() {
        if !matches!(m.file_type, FileType::Dir) {
            yield Ok(m);
            continue;
        }

        let list = list_meta(&m.id).await;
        let list = unwrap_ok_or!(list, e, {
            yield Poll::Ready(Ok(m));
            yield Poll::Ready(Err(e));
            continue;
        });

        yield Ok(m);

        #[for_await]
        for r in list {
            let f = unwrap_ok_or!(r, e, {
                yield Poll::Ready(Err(e));
                continue;
            });

            stack.push(f);
        }
    }
}

#[allow(clippy::needless_lifetimes)]
#[stream(item = anyhow::Result<Progress>)]
pub async fn delete<'a>(files: &'a [FileMeta]) {
    let mut fls = vec![];
    let mut fls1 = vec![];
    let mut drs = vec![];

    let mut prog = Progress::default();

    for f in files {
        if !matches!(f.file_type, FileType::Dir) {
            prog.total += 1;
            yield Ok(prog.clone());
            fls.push(f);
            continue;
        }

        #[for_await]
        for r in dfs(f) {
            let f = unwrap_ok_or!(r, e, {
                yield Poll::Ready(Err(e));
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

    #[for_await]
    for r in del_stream {
        let _ = unwrap_ok_or!(r, e, {
            yield Poll::Ready(Err(e));
            continue;
        });

        prog.done += 1;
        yield Ok(prog.clone());
    }
}
