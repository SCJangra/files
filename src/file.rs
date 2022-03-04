use anyhow::Context;
use futures::{self as futs, Stream, StreamExt, TryFutureExt, TryStreamExt};
use std::path;
use tokio::{
    io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::mpsc::{unbounded_channel, UnboundedReceiver},
    task,
};
pub use types::*;

mod local;
mod types;

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

pub async fn copy_file(
    src: &FileId,
    dst_dir: &FileId,
) -> anyhow::Result<UnboundedReceiver<anyhow::Result<Progress>>> {
    let (s, r) = unbounded_channel();

    let sm = get_meta(src).await?;
    let dm = create_file(&sm.name, dst_dir)
        .and_then(|id| async move { get_meta(&id).await })
        .await?;

    let (rd, wr) = futs::try_join!(read(src), write(dst_dir, true))?;

    let mut reader = io::BufReader::new(rd);
    let mut writer = io::BufWriter::new(wr);
    let mut prog = Progress {
        total: sm.size,
        ..Default::default()
    };
    let mut buf = [0; 10_000_000];

    task::spawn(async move {
        loop {
            let res = async {
                let bytes = reader
                    .read(&mut buf)
                    .await
                    .with_context(|| format!("Error while reading file {}", sm.name))?;

                writer
                    .write_all(&buf[..bytes])
                    .await
                    .with_context(|| format!("Error while writing to file {}", dm.name))?;

                prog.done += bytes as u64;
                prog.percent = (prog.done as f64) / (prog.total as f64);
                anyhow::Ok(bytes)
            }
            .await;

            match res {
                Err(e) => {
                    s.send(Err(e))?;
                    break;
                }
                Ok(b) if b == 0 => {
                    s.send(Ok(prog.clone()))?;
                    break;
                }
                Ok(_) => s.send(Ok(prog.clone()))?,
            };
        }
        anyhow::Ok(())
    });

    Ok(r)
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

pub async fn delete_file(file: &FileId) -> anyhow::Result<()> {
    let FileId(source, id) = file;
    match source {
        FileSource::Local => local::delete_file(path::Path::new(id)).await,
    }
}

pub async fn delete_dir(dir: &FileId) -> anyhow::Result<()> {
    let FileId(source, id) = dir;
    match source {
        FileSource::Local => local::delete_dir(path::Path::new(id)).await,
    }
}

pub async fn dfs(id: &FileId) -> anyhow::Result<UnboundedReceiver<anyhow::Result<FileMeta>>> {
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

    Ok(r)
}
