//! Trainer launch pipeline and process monitoring.
//!
//! Turns the manual
//! `protontricks-launch --appid <ID> /path/to/trainer.exe` into one call:
//! validate the trainer, check that Protontricks exists, wait for the game to
//! be running, then run the trainer inside the game's own prefix.
//!
//! The command is always built as separate arguments, never through a shell.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::thread;
use std::time::{Duration, Instant};

use pt_core::AppId;

/// Name of the Protontricks binary that launches an executable in a game prefix.
pub const LAUNCHER: &str = "protontricks-launch";

/// Everything that can stop a launch before or during execution.
#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("trainer not found or unreadable: {path}")]
    TrainerUnreadable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("trainer path is not a regular file: {0}")]
    TrainerNotAFile(PathBuf),
    #[error("trainer must be a Windows executable (.exe): {0}")]
    TrainerNotExe(PathBuf),
    #[error(
        "`{LAUNCHER}` not found in PATH. Install Protontricks \
         (Flatpak: `flatpak install com.github.Matoking.protontricks`, \
         or your distro package)"
    )]
    LauncherMissing,
    #[error("game {0} is not running. Start it in Steam first, then try again")]
    GameNotRunning(AppId),
    #[error("gave up waiting for game {app_id} after {waited:?}")]
    WaitTimedOut { app_id: AppId, waited: Duration },
    #[error("failed to start `{LAUNCHER}`")]
    Spawn(#[source] io::Error),
}

/// A trainer executable that passed validation.
///
/// Imported **by reference**: the file is never copied or modified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trainer {
    path: PathBuf,
}

impl Trainer {
    /// Validates the path and resolves it to an absolute, canonical path.
    ///
    /// Canonicalizing guarantees the value handed to the child process always
    /// starts with `/`, so a file named `--help.exe` can never be read as a flag.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, LaunchError> {
        let path = path.as_ref();
        let canonical =
            fs::canonicalize(path).map_err(|source| LaunchError::TrainerUnreadable {
                path: path.to_owned(),
                source,
            })?;
        if !canonical.is_file() {
            return Err(LaunchError::TrainerNotAFile(canonical));
        }
        let is_exe = canonical
            .extension()
            .and_then(OsStr::to_str)
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
        if !is_exe {
            return Err(LaunchError::TrainerNotExe(canonical));
        }
        Ok(Self { path: canonical })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// How a launch should behave.
#[derive(Debug, Clone, Copy)]
pub struct LaunchOptions {
    /// `None`: the game must already be running.
    /// `Some(d)`: wait up to `d` for the game to appear.
    pub wait_for_game: Option<Duration>,
    /// How often to look for the game while waiting.
    pub poll_interval: Duration,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            wait_for_game: Some(Duration::from_secs(120)),
            poll_interval: Duration::from_secs(1),
        }
    }
}

/// Builds the exact command, without running it.
pub fn build_command(launcher: &Path, app_id: AppId, trainer: &Trainer) -> Command {
    let mut cmd = Command::new(launcher);
    cmd.arg("--appid")
        .arg(app_id.to_string())
        .arg(trainer.path());
    cmd
}

/// Finds an executable in `PATH` (same lookup a shell would do).
pub fn find_in_path(name: &str) -> Option<PathBuf> {
    find_in(name, std::env::var_os("PATH")?)
}

fn find_in(name: &str, path_var: OsString) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(name))
        .find(|p| {
            fs::metadata(p)
                .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
        })
}

/// Is the game running? Looks for Steam's `reaper ... AppId=<id>` wrapper
/// process, which exists for every game started through Steam.
pub fn is_game_running(app_id: AppId) -> bool {
    game_running_in(Path::new("/proc"), app_id)
}

fn game_running_in(proc_root: &Path, app_id: AppId) -> bool {
    let needle = format!("AppId={app_id}");
    let Ok(entries) = fs::read_dir(proc_root) else {
        return false;
    };
    entries
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .bytes()
                .all(|b| b.is_ascii_digit())
        })
        .any(|e| {
            fs::read(e.path().join("cmdline"))
                .map(|raw| raw.split(|&b| b == 0).any(|arg| arg == needle.as_bytes()))
                .unwrap_or(false) // process exited mid-scan: ignore
        })
}

/// Polls until the game shows up or the timeout expires.
pub fn wait_for_game(
    app_id: AppId,
    timeout: Duration,
    interval: Duration,
    mut on_waiting: impl FnMut(),
) -> Result<(), LaunchError> {
    let start = Instant::now();
    let mut announced = false;
    loop {
        if is_game_running(app_id) {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            return Err(LaunchError::WaitTimedOut {
                app_id,
                waited: timeout,
            });
        }
        if !announced {
            on_waiting();
            announced = true;
        }
        thread::sleep(interval);
    }
}

/// Full pipeline: validate, check dependencies, wait for the game, run.
///
/// Blocks until the trainer process exits and returns its exit status.
pub fn launch(
    app_id: AppId,
    trainer: &Trainer,
    options: LaunchOptions,
    on_waiting: impl FnMut(),
) -> Result<ExitStatus, LaunchError> {
    let launcher = find_in_path(LAUNCHER).ok_or(LaunchError::LauncherMissing)?;

    match options.wait_for_game {
        Some(timeout) => wait_for_game(app_id, timeout, options.poll_interval, on_waiting)?,
        None if !is_game_running(app_id) => return Err(LaunchError::GameNotRunning(app_id)),
        None => {}
    }

    build_command(&launcher, app_id, trainer)
        .status()
        .map_err(LaunchError::Spawn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn id(n: u32) -> AppId {
        AppId::new(n).unwrap()
    }

    #[test]
    fn command_uses_separate_args_in_the_right_order() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir
            .path()
            .join("God of War Ragnarok v1.0 Plus 26 Trainer.exe");
        fs::write(&exe, b"MZ").unwrap();
        let trainer = Trainer::from_path(&exe).unwrap();

        let cmd = build_command(
            Path::new("/usr/bin/protontricks-launch"),
            id(2322010),
            &trainer,
        );
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args.len(), 3, "path with spaces must stay ONE argument");
        assert_eq!(args[0], "--appid");
        assert_eq!(args[1], "2322010");
        assert_eq!(args[2], trainer.path().as_os_str());
    }

    #[test]
    fn rejects_missing_non_exe_and_directories() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            Trainer::from_path(dir.path().join("nope.exe")),
            Err(LaunchError::TrainerUnreadable { .. })
        ));
        let txt = dir.path().join("trainer.txt");
        fs::write(&txt, b"x").unwrap();
        assert!(matches!(
            Trainer::from_path(&txt),
            Err(LaunchError::TrainerNotExe(_))
        ));
        let sub = dir.path().join("folder.exe");
        fs::create_dir(&sub).unwrap();
        assert!(matches!(
            Trainer::from_path(&sub),
            Err(LaunchError::TrainerNotAFile(_))
        ));
    }

    #[test]
    fn accepts_uppercase_extension_and_dash_prefixed_names() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("--help.EXE");
        fs::write(&exe, b"MZ").unwrap();
        let t = Trainer::from_path(&exe).unwrap();
        assert!(t.path().is_absolute());
    }

    #[test]
    fn detects_steam_reaper_process_in_fake_proc() {
        let proc = tempfile::tempdir().unwrap();
        let p = proc.path().join("4242");
        fs::create_dir(&p).unwrap();
        fs::write(
            p.join("cmdline"),
            b"reaper\0SteamLaunch\0AppId=2322010\0--\0game.exe\0",
        )
        .unwrap();
        let other = proc.path().join("77");
        fs::create_dir(&other).unwrap();
        fs::write(other.join("cmdline"), b"bash\0").unwrap();
        fs::create_dir(proc.path().join("self-not-a-pid")).unwrap();

        assert!(game_running_in(proc.path(), id(2322010)));
        assert!(
            !game_running_in(proc.path(), id(232201)),
            "prefix of another id must not match"
        );
        assert!(!game_running_in(proc.path(), id(999)));
    }

    #[test]
    fn path_lookup_requires_the_executable_bit() {
        use std::os::unix::fs::PermissionsExt;
        let bin = tempfile::tempdir().unwrap();
        let tool = bin.path().join("protontricks-launch");
        fs::write(&tool, b"#!/bin/sh\n").unwrap();
        let path_var = OsString::from(bin.path());

        assert_eq!(find_in(LAUNCHER, path_var.clone()), None);
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(find_in(LAUNCHER, path_var), Some(tool));
    }

    #[test]
    fn wait_times_out_when_game_never_appears() {
        let err = wait_for_game(
            id(1),
            Duration::from_millis(30),
            Duration::from_millis(10),
            || {},
        )
        .unwrap_err();
        assert!(matches!(err, LaunchError::WaitTimedOut { .. }));
    }
}
