use futures as futs;
use std::path;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync, task,
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

pub async fn list_meta(id: &FileId) -> anyhow::Result<Vec<FileMeta>> {
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
    source: &FileId,
    dest: &FileId,
) -> anyhow::Result<(impl AsyncRead, impl AsyncWrite)> {
    let fm = get_meta(source).await?;
    let dest = create_file(&fm.name, dest).await?;

    let rw = futs::try_join!(read(source), write(&dest, true))?;
    Ok(rw)
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

pub fn walk(dir: &FileId) -> sync::mpsc::UnboundedReceiver<anyhow::Result<FileMeta>> {
    let (s, r) = sync::mpsc::unbounded_channel();
    let dir = dir.to_owned();
    task::spawn(async move {
        let m = match get_meta(&dir).await {
            Err(e) => {
                s.send(Err(e))
                    .map_err(|_| anyhow::anyhow!("Walk cancelled!"))?;

                return anyhow::Ok(());
            }
            Ok(m) => m,
        };

        let mut stack = vec![m];

        while let Some(f) = stack.pop() {
            if let FileType::Dir = f.file_type {
                let mut l = match list_meta(&f.id).await {
                    Err(e) => {
                        s.send(Err(e))
                            .map_err(|_| anyhow::anyhow!("Walk cancelled!"))?;

                        continue;
                    }
                    Ok(l) => l,
                };

                s.send(Ok(f))
                    .map_err(|_| anyhow::anyhow!("Walk cancelled!"))?;

                while let Some(f) = l.pop() {
                    stack.push(f);
                }
            } else {
                s.send(Ok(f))
                    .map_err(|_| anyhow::anyhow!("Walk cancelled!"))?;
            }
        }

        anyhow::Ok(())
    });

    return r;
}
