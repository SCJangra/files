use anyhow::Context;
use futures::{self as futs, Stream, StreamExt, TryFutureExt};
use futures_async_stream::{stream as a_stream, stream_block, try_stream as a_try_stream};
use std::{path, sync::Arc, task::Poll};
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use unwrap_or::unwrap_ok_or;

use super::{file_source::*, types::*};

pub async fn get_meta(id: &FileId) -> anyhow::Result<FileMeta> {
    let FileId(source, id) = id;
    match source {
        FileSource::Local => local::get_meta(path::Path::new(id)).await,
    }
}

pub async fn list_meta(
    id: &FileId,
) -> anyhow::Result<impl Stream<Item = anyhow::Result<FileMeta>>> {
    let FileId(source, id) = id;

    match source {
        FileSource::Local => local::list_meta(path::Path::new(id)).await,
    }
}

pub async fn read(id: &FileId) -> anyhow::Result<impl AsyncRead> {
    let FileId(source, id) = id;
    match source {
        FileSource::Local => local::read(path::Path::new(id)).await,
    }
}

pub async fn write(id: &FileId, overwrite: bool) -> anyhow::Result<impl AsyncWrite> {
    let FileId(source, id) = id;
    match source {
        FileSource::Local => local::write(path::Path::new(id), overwrite).await,
    }
}

pub async fn create_file(name: &str, dir: &FileId) -> anyhow::Result<FileId> {
    let FileId(source, id) = dir;
    match source {
        FileSource::Local => local::create_file(name, path::Path::new(id)).await,
    }
}

pub async fn create_dir(name: &str, dir: &FileId) -> anyhow::Result<FileId> {
    let FileId(source, id) = dir;
    match source {
        FileSource::Local => local::create_dir(name, path::Path::new(id)).await,
    }
}

pub async fn rename(id: &FileId, new_name: &str) -> anyhow::Result<FileId> {
    let FileId(source, id) = id;
    match source {
        FileSource::Local => local::rename(path::Path::new(id), new_name).await,
    }
}

pub async fn delete_file(file: &FileId) -> anyhow::Result<bool> {
    let FileId(source, id) = file;
    match source {
        FileSource::Local => local::delete_file(path::Path::new(id)).await,
    }
}

pub async fn delete_dir(dir: &FileId) -> anyhow::Result<bool> {
    let FileId(source, id) = dir;
    match source {
        FileSource::Local => local::delete_dir(path::Path::new(id)).await,
    }
}

#[allow(clippy::needless_lifetimes)]
#[a_stream(item = anyhow::Result<FileMeta>)]
pub async fn dfs(file: &FileMeta) {
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

#[a_stream(item = anyhow::Result<CopyProg>)]
pub async fn copy(files: Vec<Arc<FileMeta>>, dst: Arc<FileMeta>) {
    let mut prog = CopyProg::default();

    let mut cp = vec![];

    for f in files.into_iter() {
        if !matches!(f.file_type, FileType::Dir) {
            prog.files.total += 1;
            prog.size.total += f.size;

            yield Ok(prog.clone());

            cp.push((f, dst.clone()));
            continue;
        }

        let cps = clone_dir_structure(&f, &dst).await;
        let cps = unwrap_ok_or!(cps, e, {
            yield Poll::Ready(Err(e));
            continue;
        });

        #[for_await]
        for r in cps {
            let (f, d) = unwrap_ok_or!(r, e, {
                yield Poll::Ready(Err(e));
                continue;
            });

            prog.files.total += 1;
            prog.size.total += f.size;

            yield Ok(prog.clone());

            cp.push((Arc::new(f), d));
        }
    }

    for (f, d) in cp.into_iter() {
        prog.current.name = f.name.clone();
        prog.current.prog = Progress {
            total: f.size,
            ..Default::default()
        };

        yield Ok(prog.clone());

        #[for_await]
        for r in copy_file(f.clone(), d) {
            let bytes = unwrap_ok_or!(r, e, {
                yield Poll::Ready(Err(e));
                continue;
            });

            prog.size.done += bytes;
            prog.current.prog.done += bytes;

            yield Ok(prog.clone());
        }

        prog.files.done += 1;

        yield Ok(prog.clone());
    }
}

pub async fn mv<'a>(
    files: &'a [FileMeta],
    dir: &'a FileMeta,
) -> impl Stream<Item = anyhow::Result<FileMeta>> + 'a {
    futs::stream::iter(files.iter())
        .map(|f| move_file(&f.id, &dir.id).and_then(|id| async move { get_meta(&id).await }))
        .buffer_unordered(1000)
}

async fn clone_dir_structure(
    dir: &FileMeta,
    dst: &FileMeta,
) -> anyhow::Result<impl Stream<Item = anyhow::Result<(FileMeta, Arc<FileMeta>)>>> {
    let sm = dir.to_owned();
    let dm = async {
        let id = create_dir(&sm.name, &dst.id).await?;
        get_meta(&id).await
    }
    .await?;

    let s = stream_block! {
        let mut src_stack = vec![sm];
        let mut dst_stack = vec![dm];

        while let (Some(sm), Some(dm)) = (src_stack.pop(), dst_stack.pop()) {
            let dm = Arc::new(dm);

            let list = list_meta(&sm.id).await;
            let list = unwrap_ok_or!(list, e, {
                yield Poll::Ready(Err(e));
                continue;
            });

            #[for_await]
            for r in list {
                let sm = unwrap_ok_or!(r, e, {
                    yield Poll::Ready(Err(e));
                    continue;
                });

                if !matches!(sm.file_type, FileType::Dir) {
                    yield Ok((sm, dm.clone()));
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
        }
    };

    Ok(s)
}

#[allow(clippy::needless_lifetimes)]
#[a_try_stream(ok = u64, error = anyhow::Error)]
async fn copy_file(src: Arc<FileMeta>, dst: Arc<FileMeta>) {
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

async fn move_file(file: &FileId, dir: &FileId) -> anyhow::Result<FileId> {
    match (&file.0, &dir.0) {
        (FileSource::Local, FileSource::Local) => {
            local::move_file(path::Path::new(&file.1), path::Path::new(&dir.1)).await
        }
    }
}
