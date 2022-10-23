use std::io;

pub trait IntoIOErr {
    fn into_io_err(self) -> io::Error;
}

impl IntoIOErr for anyhow::Error {
    fn into_io_err(self) -> io::Error {
        let e = self.downcast::<io::Error>();
        match e {
            Ok(e) => e,
            Err(e) => io::Error::new(io::ErrorKind::Other, e),
        }
    }
}

impl IntoIOErr for reqwest::Error {
    fn into_io_err(self) -> io::Error {
        io::Error::new(io::ErrorKind::Other, self)
    }
}

pub fn parse_range_header(range: &str) -> anyhow::Result<(u64, u64)> {
    use anyhow::Context;

    let (_, r) = range
        .split_once('=')
        .with_context(|| format!("invalid range header `{}`", range))?;

    let (start, end) = r
        .split_once('-')
        .with_context(|| format!("invalid range `{}`", r))?;

    let start = start.parse::<u64>()?;
    let end = end.parse::<u64>()?;
    Ok((start, end))
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_parse_range_header() -> anyhow::Result<()> {
        let range = super::parse_range_header("Range: bytes=100-200")?;
        assert_eq!(range, (100, 200));

        let range = super::parse_range_header("");
        assert!(matches!(range, Err(_)));

        Ok(())
    }
}
