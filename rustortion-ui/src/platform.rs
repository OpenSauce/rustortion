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
        let result = open_command(&path).spawn();

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

/// Build (but do not run) the command that reveals `path`.
///
/// Separate from [`open_directory`] purely so it can be asserted on without
/// launching anything: a test that actually spawned the opener would pop a file
/// manager — and an error dialog, for a path that doesn't exist — on the machine
/// running `cargo test`.
fn open_command(path: &Path) -> Command {
    let mut command = Command::new(opener());
    command
        .arg(path)
        // Detach the child's stdio: inheriting the host's handles can wedge a
        // plugin host, and nothing reads the output anyway.
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
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

    /// Asserts what would be run, without running it. Nothing in this module's
    /// tests may actually spawn the opener: doing so pops a file manager window
    /// (and an error dialog, for a path that doesn't exist) on whatever machine is
    /// running the suite, and would be flaky on a headless CI box besides.
    #[test]
    fn command_is_the_opener_with_the_path_as_its_only_argument() {
        let command = open_command(Path::new("/tmp/some nam dir"));

        assert_eq!(command.get_program(), opener());
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args, ["/tmp/some nam dir"]);
    }

    /// A path with spaces or shell metacharacters is passed as one argument, never
    /// through a shell — so it needs no quoting and cannot be word-split.
    #[test]
    fn path_is_passed_as_a_single_unquoted_argument() {
        let nasty = Path::new("/tmp/nam models; rm -rf $HOME/'quoted'");
        let command = open_command(nasty);

        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args.len(), 1, "must be exactly one argument");
        assert_eq!(args[0], nasty.as_os_str());
    }
}
