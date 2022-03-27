use anyhow::Context;
use futures::{self as futs, Stream, StreamExt, TryFutureExt, TryStreamExt};
use std::{path, sync::Arc};
use tokio::{
    io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::mpsc::unbounded_channel,
    task,
};
use tokio_stream::wrappers as tsw;
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

pub async fn move_file(file: &FileId, dir: &FileId) -> anyhow::Result<FileId> {
    match (&file.0, &dir.0) {
        (FileSource::Local, FileSource::Local) => {
            local::move_file(path::Path::new(&file.1), path::Path::new(&dir.1)).await
        }
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

pub async fn dfs(id: &FileId) -> anyhow::Result<impl Stream<Item = anyhow::Result<FileMeta>>> {
    let (s, r) = unbounded_channel();
    let m = get_meta(id).await?;

    task::spawn(async move {
        let mut stack = vec![m];

        while let Some(m) = stack.pop() {
            if !matches!(m.file_type, FileType::Dir) {
                s.send(Ok(m))?;
                continue;
            }

            let list = match list_meta(&m.id).await {
                Err(e) => {
                    s.send(Ok(m))?;
                    s.send(Err(e))?;
                    continue;
                }
                Ok(l) => {
                    s.send(Ok(m))?;
                    l
                }
            };

            list.map(|r| match r {
                Err(e) => s.send(Err(e)),
                Ok(m) => {
                    stack.push(m);
                    Ok(())
                }
            })
            .try_for_each(|_| async { Ok(()) })
            .await?;
        }

        anyhow::Ok(())
    });

    let s = tsw::UnboundedReceiverStream::new(r);

    Ok(s)
}

pub async fn copy(
    files: Vec<Arc<FileMeta>>,
    dst: Arc<FileMeta>,
) -> impl Stream<Item = anyhow::Result<CopyProg>> {
    let (s, r) = unbounded_channel();

    let mut prog = CopyProg::default();

    task::spawn(async move {
        let mut cp = vec![];

        for f in files.into_iter() {
            if !matches!(f.file_type, FileType::Dir) {
                prog.files.total += 1;
                prog.size.total += f.size;

                s.send(Ok(prog.clone()))?;

                cp.push((f, dst.clone()));
                continue;
            }

            let mut cps = unwrap_ok_or!(clone_dir_structure(&f.id, &dst.id).await, e, {
                s.send(Err(e))?;
                continue;
            });

            while let Some(r) = cps.next().await {
                let c = unwrap_ok_or!(r, e, {
                    s.send(Err(e))?;
                    continue;
                });

                prog.files.total += 1;
                prog.size.total += c.0.size;

                s.send(Ok(prog.clone()))?;

                cp.push((Arc::new(c.0), c.1));
            }
        }

        for (f, d) in cp.into_iter() {
            let prog_stream = unwrap_ok_or!(copy_file(f.clone(), d).await, e, {
                s.send(Err(e))?;
                continue;
            });

            prog.current.name = f.name.clone();
            prog.current.prog = Progress {
                total: f.size,
                ..Default::default()
            };

            s.send(Ok(prog.clone()))?;

            prog_stream
                .map(|r| {
                    let p = unwrap_ok_or!(r, e, {
                        return s.send(Err(e));
                    });

                    prog.size.done += p;
                    prog.current.prog.done += p;

                    s.send(Ok(prog.clone()))
                })
                .try_for_each(|_| async move { Ok(()) })
                .await?;

            prog.files.done += 1;
            s.send(Ok(prog.clone()))?;
        }

        anyhow::Ok(())
    });

    tsw::UnboundedReceiverStream::new(r)
}

async fn clone_dir_structure(
    dir: &FileId,
    dst: &FileId,
) -> anyhow::Result<impl Stream<Item = anyhow::Result<(FileMeta, Arc<FileMeta>)>>> {
    let (s, r) = unbounded_channel();

    let sm = get_meta(dir).await?;
    let dm = create_dir(&sm.name, dst)
        .and_then(|id| async move { get_meta(&id).await })
        .await?;

    let mut src_stack = vec![sm];
    let mut dst_stack = vec![dm];

    task::spawn(async move {
        while let (Some(sm), Some(dm)) = (src_stack.pop(), dst_stack.pop()) {
            let dm = Arc::new(dm);

            let list = match list_meta(&sm.id).await {
                Err(e) => {
                    s.send(Err(e))?;
                    continue;
                }
                Ok(l) => l,
            };

            futs::pin_mut!(list);

            while let Some(r) = list.next().await {
                let sm = match r {
                    Err(e) => {
                        s.send(Err(e))?;
                        continue;
                    }
                    Ok(m) => m,
                };

                if !matches!(sm.file_type, FileType::Dir) {
                    let copy = (sm, dm.clone());
                    s.send(Ok(copy))?;
                    continue;
                }

                let dm = create_dir(&sm.name, &dm.id)
                    .and_then(|id| async move { get_meta(&id).await })
                    .await?;

                src_stack.push(sm);
                dst_stack.push(dm);
            }
        }

        anyhow::Ok(())
    });

    Ok(tsw::UnboundedReceiverStream::new(r))
}

async fn copy_file(
    src: Arc<FileMeta>,
    dst: Arc<FileMeta>,
) -> anyhow::Result<impl Stream<Item = anyhow::Result<u64>>> {
    let (s, r) = unbounded_channel();

    let dm = create_file(&src.name, &dst.id)
        .and_then(|id| async move { get_meta(&id).await })
        .await?;

    let (rd, wr) = futs::try_join!(read(&src.id), write(&dm.id, true))?;

    let mut reader = io::BufReader::new(rd);
    let mut writer = io::BufWriter::new(wr);
    let mut buf = vec![0; 10_000_000];

    task::spawn(async move {
        loop {
            let res = async {
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
            .await;

            match res {
                Err(e) => {
                    s.send(Err(e))?;
                    break;
                }
                Ok(b) if b == 0 => {
                    break;
                }
                Ok(b) => s.send(Ok(b as u64))?,
            };
        }
        anyhow::Ok(())
    });

    let s = tsw::UnboundedReceiverStream::new(r);

    Ok(s)
}
