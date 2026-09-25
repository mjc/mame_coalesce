use std::{
    io,
    path::{Path, PathBuf},
};

/// An owner-only temporary directory removed when its guard is dropped.
pub struct PrivateTempDir {
    path: PathBuf,
}

impl PrivateTempDir {
    pub fn create(prefix: &str) -> io::Result<Self> {
        for _ in 0..10 {
            let path = std::env::temp_dir().join(format!("{prefix}{}", uuid::Uuid::new_v4()));
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a private temporary directory",
        ))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PrivateTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
