//! Where ipabazooka keeps its settings, log and sign-in data:
//!
//! * Windows: `%APPDATA%\ipabazooka`
//! * Linux (and other Unix-like systems): `$XDG_CONFIG_HOME/ipabazooka`,
//!   which is usually `~/.config/ipabazooka`. The folder is private to the
//!   user (mode 700) because it holds the signing certificate's key.

use std::path::PathBuf;

/// The app's data folder (not created here).
pub fn data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("ipabazooka")
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(std::env::temp_dir)
            .join("ipabazooka")
    }
}

/// Linux: creates the data folder if needed and makes it private to the
/// user, since it holds the signing certificate's key. (Windows creates the
/// folder when something is first saved, as before.)
#[cfg(unix)]
pub fn ensure_data_dir() -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    dir
}

/// Writes a file that only this user can read on Linux (mode 600).
pub fn write_private(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?;
        // an older file may have been created with wider permissions
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        f.write_all(data)
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, data)
    }
}

/// Opens a text file (like the saved log) in the system's text viewer.
/// Returns false when nothing could be started.
pub fn open_text_file(path: &std::path::Path) -> bool {
    #[cfg(windows)]
    {
        std::process::Command::new("notepad.exe").arg(path).spawn().is_ok()
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open").arg(path).spawn().is_ok()
    }
}
