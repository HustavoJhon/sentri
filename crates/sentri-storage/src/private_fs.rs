//! The database holds personal data, so its files are owner-only from the
//! moment they are created (no window with default umask permissions).

use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};

/// Creates the parent directory (0700) and the database file (0600) if they
/// do not exist, and tightens permissions of an existing file.
pub(crate) fn prepare_database_file(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        create_private_dir(parent)?;
    }
    match create_private_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => restrict(path),
        Err(e) => Err(e),
    }
}

/// Restricts the `-wal` and `-shm` files that SQLite creates next to `path`.
pub(crate) fn restrict_sidecars(path: &Path) -> io::Result<()> {
    for suffix in ["-wal", "-shm"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        let sidecar = Path::new(&name);
        if sidecar.exists() {
            restrict(sidecar)?;
        }
    }
    Ok(())
}

/// Creates a new empty file readable only by the owner. Fails if it exists.
pub(crate) fn create_private_file(path: &Path) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    options.open(path).map(drop)
}

fn create_private_dir(dir: &Path) -> io::Result<()> {
    if dir.exists() {
        return Ok(());
    }
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);
    builder.create(dir)
}

fn restrict(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
