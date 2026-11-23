//! Tauri adapters for ProtonTrainer's domain crates.

use pt_core::AppId;
use pt_database::{Database, ExecutionRecord, GameRecord, TrainerRecord};
use pt_proton::{detect_game_proton, ProtonInfo};
use pt_steam::{discover_installations, list_games, SteamGame};
use pt_trainers::ImportedTrainer;
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::Manager;

struct AppState(Arc<Mutex<Database>>);

#[derive(Debug, Serialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
}
#[tauri::command]
fn get_app_info() -> AppInfo {
    AppInfo {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[derive(Debug, Serialize)]
struct GameView {
    app_id: u32,
    name: String,
    install_dir: String,
    library_path: String,
    prefix: Option<String>,
    proton_version: Option<String>,
}
impl GameView {
    fn from(game: SteamGame, p: ProtonInfo) -> Self {
        Self {
            app_id: game.app_id.get(),
            name: game.name,
            install_dir: game.install_dir.to_string_lossy().into_owned(),
            library_path: game.library_path.to_string_lossy().into_owned(),
            prefix: p.prefix.map(|p| p.to_string_lossy().into_owned()),
            proton_version: p.version,
        }
    }
}
#[derive(Debug, Serialize)]
struct TrainerView {
    id: i64,
    name: String,
    exe_path: String,
    file_size: u64,
}
impl From<TrainerRecord> for TrainerView {
    fn from(t: TrainerRecord) -> Self {
        Self {
            id: t.id,
            name: t.name,
            exe_path: t.exe_path,
            file_size: t.file_size,
        }
    }
}
#[derive(Debug, Serialize)]
struct HistoryView {
    id: i64,
    app_id: u32,
    trainer_id: Option<i64>,
    started_at: String,
    finished_at: Option<String>,
    status: String,
    exit_code: Option<i32>,
    error: Option<String>,
}
impl From<ExecutionRecord> for HistoryView {
    fn from(e: ExecutionRecord) -> Self {
        Self {
            id: e.id,
            app_id: e.app_id,
            trainer_id: e.trainer_id,
            started_at: e.started_at,
            finished_at: e.finished_at,
            status: e.status,
            exit_code: e.exit_code,
            error: e.error,
        }
    }
}

fn database(state: &AppState) -> Result<std::sync::MutexGuard<'_, Database>, String> {
    state
        .0
        .lock()
        .map_err(|_| "banco de dados indisponível".to_owned())
}
fn steam_root(db: &Database) -> Result<Option<PathBuf>, String> {
    db.setting("steam_root")
        .map(|v| v.map(PathBuf::from))
        .map_err(|e| e.to_string())
}

#[tauri::command(async)]
async fn list_games_command(state: tauri::State<'_, AppState>) -> Result<Vec<GameView>, String> {
    let db = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let configured_root = {
            let db = db
                .lock()
                .map_err(|_| "banco de dados indisponível".to_owned())?;
            steam_root(&db)?
        };
        let installations =
            discover_installations(configured_root.as_deref()).map_err(|e| e.to_string())?;
        let games = list_games(&installations).map_err(|e| e.to_string())?;
        {
            let db = db
                .lock()
                .map_err(|_| "banco de dados indisponível".to_owned())?;
            for g in &games {
                db.upsert_game(&GameRecord {
                    app_id: g.app_id.get(),
                    name: g.name.clone(),
                    install_dir: g.install_dir.to_string_lossy().into_owned(),
                    library_path: g.library_path.to_string_lossy().into_owned(),
                })
                .map_err(|e| e.to_string())?;
            }
        }
        Ok(games
            .into_iter()
            .map(|g| {
                let fallback_root = PathBuf::new();
                let root = installations
                    .iter()
                    .find(|i| i.libraries.contains(&g.library_path))
                    .map(|i| i.root.as_path())
                    .unwrap_or(&fallback_root);
                let info = detect_game_proton(&g, root).unwrap_or(ProtonInfo {
                    prefix: None,
                    version: None,
                });
                GameView::from(g, info)
            })
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command(async)]
fn list_trainers_command(
    state: tauri::State<'_, AppState>,
    app_id: Option<u32>,
) -> Result<Vec<TrainerView>, String> {
    database(&state)?
        .list_trainers(app_id)
        .map(|v| v.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}
#[tauri::command(async)]
fn import_trainer_command(
    state: tauri::State<'_, AppState>,
    path: String,
    app_id: u32,
) -> Result<TrainerView, String> {
    let trainer = ImportedTrainer::validate(&path).map_err(|e| e.to_string())?;
    let db = database(&state)?;
    let id = db
        .add_trainer(
            &trainer.name,
            &trainer.path.to_string_lossy(),
            trainer.file_size,
        )
        .map_err(|e| e.to_string())?;
    db.associate(app_id, id).map_err(|e| e.to_string())?;
    Ok(TrainerView {
        id,
        name: trainer.name,
        exe_path: trainer.path.to_string_lossy().into_owned(),
        file_size: trainer.file_size,
    })
}
#[tauri::command(async)]
fn remove_trainer_command(
    state: tauri::State<'_, AppState>,
    trainer_id: i64,
) -> Result<(), String> {
    database(&state)?
        .remove_trainer(trainer_id)
        .map_err(|e| e.to_string())
}
#[tauri::command(async)]
fn set_steam_root_command(state: tauri::State<'_, AppState>, path: String) -> Result<(), String> {
    if path.trim().is_empty() {
        return database(&state)?
            .clear_setting("steam_root")
            .map_err(|e| e.to_string());
    }
    let p = PathBuf::from(&path);
    if !p.is_absolute() {
        return Err("o caminho da Steam precisa ser absoluto".into());
    }
    if !p.join("steamapps/libraryfolders.vdf").is_file() {
        return Err("não encontrei steamapps/libraryfolders.vdf nesse caminho".into());
    }
    database(&state)?
        .set_setting("steam_root", &path)
        .map_err(|e| e.to_string())
}
#[tauri::command(async)]
fn get_steam_root_command(state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    database(&state)?
        .setting("steam_root")
        .map_err(|e| e.to_string())
}
#[tauri::command(async)]
fn history_command(state: tauri::State<'_, AppState>) -> Result<Vec<HistoryView>, String> {
    database(&state)?
        .history()
        .map(|v| v.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
struct LaunchResult {
    success: bool,
    exit_code: Option<i32>,
    message: String,
}
#[tauri::command]
async fn launch_trainer_command(
    state: tauri::State<'_, AppState>,
    app_id: u32,
    trainer_id: i64,
) -> Result<LaunchResult, String> {
    let app_id = AppId::new(app_id).map_err(|e| e.to_string())?;
    let shared_db = state.0.clone();
    let (launch_trainer, run_id, launch_options) = {
        let db = shared_db
            .lock()
            .map_err(|_| "banco de dados indisponível")?;
        let trainer = db
            .list_trainers(Some(app_id.get()))
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|t| t.id == trainer_id)
            .ok_or("trainer não está associado a este jogo")?;
        let imported = ImportedTrainer::validate(&trainer.exe_path).map_err(|e| e.to_string())?;
        if imported.file_size != trainer.file_size {
            return Err("trainer file changed since import; import it again".into());
        }
        let launch_trainer =
            pt_execution::Trainer::from_path(&imported.path).map_err(|e| e.to_string())?;
        let run_id = db
            .start_execution(app_id.get(), trainer_id)
            .map_err(|e| e.to_string())?;
        let options = pt_execution::LaunchOptions {
            wait_for_game: Some(Duration::from_secs(120)),
            steam_root: steam_root(&db)?,
            quiet: true,
            ..Default::default()
        };
        (launch_trainer, run_id, options)
    };
    let result = tauri::async_runtime::spawn_blocking(move || {
        pt_execution::launch(app_id, &launch_trainer, launch_options, || {})
    })
    .await
    .map_err(|e| e.to_string());
    let result = match result {
        Ok(result) => result,
        Err(message) => {
            let db = shared_db
                .lock()
                .map_err(|_| "banco de dados indisponível")?;
            db.finish_execution(run_id, None, Some("launch task failed"))
                .map_err(|e| e.to_string())?;
            return Err(message);
        }
    };
    let db = shared_db
        .lock()
        .map_err(|_| "banco de dados indisponível")?;
    match result {
        Ok(status) => {
            let code = status.code();
            db.finish_execution(run_id, code, None)
                .map_err(|e| e.to_string())?;
            Ok(LaunchResult {
                success: status.success(),
                exit_code: code,
                message: if status.success() {
                    "trainer encerrado com sucesso".into()
                } else {
                    "trainer encerrou com erro".into()
                },
            })
        }
        Err(e) => {
            let message = e.to_string();
            db.finish_execution(run_id, None, Some("trainer launch failed"))
                .map_err(|db_e| db_e.to_string())?;
            Err(message)
        }
    }
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let path = app.path().app_data_dir()?;
            std::fs::create_dir_all(&path)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
            }
            let db = Database::open(path.join("protontrainer.sqlite3"))
                .map_err(std::io::Error::other)?;
            app.manage(AppState(Arc::new(Mutex::new(db))));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            list_games_command,
            list_trainers_command,
            import_trainer_command,
            remove_trainer_command,
            set_steam_root_command,
            get_steam_root_command,
            history_command,
            launch_trainer_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running ProtonTrainer");
}
