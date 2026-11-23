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
use std::io::{self, Read, Seek, SeekFrom};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use pt_core::AppId;
use pt_proton::detect_game_proton;
use pt_steam::{discover_installations, list_games};

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
    #[error("trainer does not have a valid Windows PE signature: {0}")]
    InvalidExecutable(PathBuf),
    #[error(
        "`{LAUNCHER}` not found in PATH. Install Protontricks \
         (Flatpak: `flatpak install com.github.Matoking.protontricks`, \
         or your distro package)"
    )]
    LauncherMissing,
    #[error("game {0} is not running. Start it in Steam first, then try again")]
    GameNotRunning(AppId),
    #[error("Steam game {0} is not installed or could not be detected")]
    GameNotInstalled(AppId),
    #[error("no valid Proton prefix was found for game {0}; start it once in Steam first")]
    PrefixMissing(AppId),
    #[error("Steam discovery failed: {0}")]
    Discovery(String),
    #[error("gave up waiting for game {app_id} after {waited:?}")]
    WaitTimedOut { app_id: AppId, waited: Duration },
    #[error("failed to start `{LAUNCHER}`")]
    Spawn(#[source] io::Error),
}

impl LaunchError {
    /// Error text suitable for stderr/history surfaces that should not retain
    /// a user's absolute filesystem paths or operating-system details.
    pub fn safe_message(&self) -> String {
        match self {
            Self::TrainerUnreadable { .. } => "trainer file is missing or unreadable".into(),
            Self::TrainerNotAFile(_) => "trainer path is not a regular file".into(),
            Self::TrainerNotExe(_) => "trainer must be a Windows .exe file".into(),
            Self::InvalidExecutable(_) => {
                "trainer does not contain a valid Windows PE signature".into()
            }
            Self::LauncherMissing => format!("`{LAUNCHER}` not found; install Protontricks"),
            Self::GameNotRunning(app_id) => format!("game {app_id} is not running"),
            Self::GameNotInstalled(app_id) => {
                format!("Steam game {app_id} is not installed or detectable")
            }
            Self::PrefixMissing(app_id) => {
                format!("no valid Proton prefix was found for game {app_id}")
            }
            Self::Discovery(_) => "Steam discovery failed; check the Steam metadata files".into(),
            Self::WaitTimedOut { app_id, waited } => {
                format!("timed out waiting for game {app_id} after {waited:?}")
            }
            Self::Spawn(_) => format!("failed to start `{LAUNCHER}`"),
        }
    }
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
        let valid_pe =
            has_pe_signature(&canonical).map_err(|source| LaunchError::TrainerUnreadable {
                path: canonical.clone(),
                source,
            })?;
        if !valid_pe {
            return Err(LaunchError::InvalidExecutable(canonical));
        }
        Ok(Self { path: canonical })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn has_pe_signature(path: &Path) -> io::Result<bool> {
    const DOS_HEADER_SIZE: usize = 64;
    let mut file = fs::File::open(path)?;
    let length = file.metadata()?.len();
    if length < DOS_HEADER_SIZE as u64 {
        return Ok(false);
    }
    let mut dos_header = [0_u8; DOS_HEADER_SIZE];
    match file.read_exact(&mut dos_header) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(false),
        Err(error) => return Err(error),
    }
    if &dos_header[..2] != b"MZ" {
        return Ok(false);
    }
    let pe_offset = u32::from_le_bytes(dos_header[0x3c..0x40].try_into().unwrap()) as u64;
    match pe_offset.checked_add(4) {
        Some(end) if end <= length => {}
        _ => return Ok(false),
    }
    file.seek(SeekFrom::Start(pe_offset))?;
    let mut pe_signature = [0_u8; 4];
    match file.read_exact(&mut pe_signature) {
        Ok(()) => Ok(&pe_signature == b"PE\0\0"),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => Err(error),
    }
}

/// How a launch should behave.
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    /// `None`: the game must already be running.
    /// `Some(d)`: wait up to `d` for the game to appear.
    pub wait_for_game: Option<Duration>,
    /// How often to look for the game while waiting.
    pub poll_interval: Duration,
    /// Optional Steam root selected by the user.
    pub steam_root: Option<PathBuf>,
    /// Suppress child stdout/stderr when launched from the GUI.
    pub quiet: bool,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            wait_for_game: Some(Duration::from_secs(120)),
            poll_interval: Duration::from_secs(1),
            steam_root: None,
            quiet: false,
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

fn build_flatpak_command(flatpak: &Path, app_id: AppId, trainer: &Trainer) -> Command {
    let mut command = Command::new(flatpak);
    command
        .args([
            "run",
            "--command=protontricks-launch",
            "com.github.Matoking.protontricks",
        ])
        .arg("--appid")
        .arg(app_id.to_string())
        .arg(trainer.path());
    command
}

fn run_command(mut command: Command) -> Result<ExitStatus, LaunchError> {
    command.status().map_err(LaunchError::Spawn)
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
                .map(|raw| {
                    let args: Vec<_> = raw.split(|&b| b == 0).filter(|a| !a.is_empty()).collect();
                    let executable = args
                        .first()
                        .and_then(|a| Path::new(std::ffi::OsStr::from_bytes(a)).file_name());
                    executable.is_some_and(|name| name == OsStr::new("reaper"))
                        && args.contains(&needle.as_bytes())
                        && args.contains(&b"SteamLaunch".as_slice())
                })
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
    let interval = interval.max(Duration::from_millis(1));
    loop {
        if is_game_running(app_id) {
            return Ok(());
        }
        let elapsed = start.elapsed();
        if elapsed >= timeout {
            return Err(LaunchError::WaitTimedOut {
                app_id,
                waited: timeout,
            });
        }
        if !announced {
            on_waiting();
            announced = true;
        }
        thread::sleep(interval.min(timeout.saturating_sub(elapsed)));
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
    let installations = discover_installations(options.steam_root.as_deref())
        .map_err(|e| LaunchError::Discovery(e.to_string()))?;
    let games = list_games(&installations).map_err(|e| LaunchError::Discovery(e.to_string()))?;
    let game = games
        .iter()
        .find(|game| game.app_id == app_id)
        .ok_or(LaunchError::GameNotInstalled(app_id))?;
    let steam_root = installations
        .iter()
        .find(|installation| installation.libraries.contains(&game.library_path))
        .or_else(|| installations.first())
        .ok_or(LaunchError::GameNotInstalled(app_id))?;
    let proton = detect_game_proton(game, &steam_root.root)
        .map_err(|e| LaunchError::Discovery(e.to_string()))?;
    if proton.prefix.is_none() {
        return Err(LaunchError::PrefixMissing(app_id));
    }

    let launcher = find_in_path(LAUNCHER);
    let flatpak = if launcher.is_none() {
        find_in_path("flatpak")
    } else {
        None
    };
    if launcher.is_none() && flatpak.is_none() {
        return Err(LaunchError::LauncherMissing);
    }

    match options.wait_for_game {
        Some(timeout) => wait_for_game(app_id, timeout, options.poll_interval, on_waiting)?,
        None if !is_game_running(app_id) => return Err(LaunchError::GameNotRunning(app_id)),
        None => {}
    }

    // The user may have changed or replaced the file while we scanned Steam or
    // waited for the game. Revalidate immediately before handing its path off.
    let trainer = Trainer::from_path(trainer.path())?;

    let command = if let Some(launcher) = launcher {
        build_command(&launcher, app_id, &trainer)
    } else {
        build_flatpak_command(&flatpak.expect("checked above"), app_id, &trainer)
    };
    let mut command = command;
    if options.quiet {
        command.stdout(Stdio::null()).stderr(Stdio::null());
    }
    run_command(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn id(n: u32) -> AppId {
        AppId::new(n).unwrap()
    }

    fn write_pe(path: &Path) {
        let mut bytes = vec![0_u8; 68];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&64_u32.to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn command_uses_separate_args_in_the_right_order() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("God of War; touch marker.exe");
        write_pe(&exe);
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
    fn safe_error_text_does_not_include_absolute_paths() {
        let error = LaunchError::TrainerNotExe(PathBuf::from("/home/alice/private/trainer.txt"));
        let message = error.safe_message();
        assert!(!message.contains("/home/alice"));
        assert!(!message.contains("trainer.txt"));
    }

    #[test]
    fn flatpak_command_keeps_arguments_separate() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("trainer.exe");
        write_pe(&exe);
        let trainer = Trainer::from_path(&exe).unwrap();
        let command = build_flatpak_command(Path::new("/usr/bin/flatpak"), id(43), &trainer);
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args[0], "run");
        assert_eq!(args[1], "--command=protontricks-launch");
        assert_eq!(args[2], "com.github.Matoking.protontricks");
        assert_eq!(args[3], "--appid");
        assert_eq!(args[4], "43");
        assert_eq!(args[5], trainer.path().as_os_str());
    }

    #[test]
    fn child_exit_status_is_propagated_without_launching_a_trainer() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        for (name, code) in [("success", 0), ("failure", 23)] {
            let script = dir.path().join(name);
            fs::write(&script, format!("#!/bin/sh\nexit {code}\n")).unwrap();
            fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
            let status = run_command(Command::new(script)).unwrap();
            assert_eq!(status.code(), Some(code));
        }
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
    fn rejects_exe_extension_without_windows_signature() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-really.exe");
        fs::write(&path, b"not an executable").unwrap();
        assert!(matches!(
            Trainer::from_path(path),
            Err(LaunchError::InvalidExecutable(_))
        ));
    }

    #[test]
    fn accepts_uppercase_extension_and_dash_prefixed_names() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("--help.EXE");
        write_pe(&exe);
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
        fs::remove_dir_all(p).unwrap();
        assert!(!game_running_in(proc.path(), id(2322010)));
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
        let started = Instant::now();
        let err = wait_for_game(
            id(u32::MAX),
            Duration::from_millis(30),
            Duration::from_secs(10),
            || {},
        )
        .unwrap_err();
        assert!(matches!(err, LaunchError::WaitTimedOut { .. }));
        assert!(started.elapsed() < Duration::from_millis(250));
    }

    #[test]
    fn zero_poll_interval_is_bounded_and_wait_callback_runs_once() {
        let calls = std::cell::Cell::new(0);
        let err = wait_for_game(
            id(u32::MAX),
            Duration::from_millis(15),
            Duration::ZERO,
            || calls.set(calls.get() + 1),
        )
        .unwrap_err();
        assert!(matches!(err, LaunchError::WaitTimedOut { .. }));
        assert_eq!(calls.get(), 1);
    }
}
