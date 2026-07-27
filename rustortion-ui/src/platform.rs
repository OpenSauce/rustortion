//! Small platform shims for the shared GUI.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use log::warn;

/// The platform's "open this in the file manager" command.
///
/// Deliberately a plain process spawn rather than a file-dialog crate: `rfd` pulls
/// in gtk3, which breaks CI (the same reason the NAM model picker is a pick-list and
/// not a file browser). Every target here ships its opener as part of the base OS.
const fn opener() -> &'static str {
    if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        // Linux/BSD: part of xdg-utils, present on any desktop that can run a GUI.
        "xdg-open"
    }
}

/// Reveal `path` in the user's file manager.
///
/// Returns immediately: the spawn and the subsequent reap both happen on a detached
/// thread, so a slow-starting file manager can never stall the GUI. The child is
/// waited on rather than dropped, because a dropped [`std::process::Child`] leaves a
/// zombie on Unix for the lifetime of the process — one per click, otherwise.
///
/// Failures are logged, not surfaced: the button is a convenience, and the path it
/// would have opened is already displayed next to it.
pub fn open_directory(path: &Path) {
    let path: PathBuf = path.to_path_buf();
    std::thread::spawn(move || {
        let result = Command::new(opener())
            .arg(&path)
            // Detach the child's stdio: inheriting the host's handles can wedge a
            // plugin host, and nothing reads the output anyway.
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        match result {
            Ok(mut child) => {
                if let Err(e) = child.wait() {
                    warn!("Failed to wait on '{}': {e}", opener());
                }
            }
            Err(e) => warn!(
                "Failed to open '{}' with '{}': {e}",
                path.display(),
                opener()
            ),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opener_is_the_platform_command() {
        // One assertion per target so a wrong `cfg!` arm can't pass by accident.
        #[cfg(target_os = "windows")]
        assert_eq!(opener(), "explorer");
        #[cfg(target_os = "macos")]
        assert_eq!(opener(), "open");
        #[cfg(all(unix, not(target_os = "macos")))]
        assert_eq!(opener(), "xdg-open");
    }

    /// Opening a path that cannot exist must log and move on, never panic or block —
    /// the whole point of doing the work on a detached thread.
    #[test]
    fn missing_path_does_not_panic() {
        open_directory(Path::new("/nonexistent-rustortion-nam-dir-for-tests"));
    }
}
