//! SQLite persistence for discovered games, referenced trainers and runs.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("database operation failed: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("database schema version {0} is newer than the supported version")]
    UnsupportedSchemaVersion(i64),
    #[error("could not secure database file {path}: {source}")]
    FileAccess {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("database path is not a regular file: {0}")]
    NotAFile(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameRecord {
    pub app_id: u32,
    pub name: String,
    pub install_dir: String,
    pub library_path: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainerRecord {
    pub id: i64,
    pub name: String,
    pub exe_path: String,
    pub file_size: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRecord {
    pub id: i64,
    pub app_id: u32,
    pub trainer_id: Option<i64>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct Database {
    connection: Connection,
}
impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let path = path.as_ref();
        ensure_private_file(path)?;
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        let mut db = Self { connection };
        db.initialize()?;
        Ok(db)
    }
    pub fn open_in_memory() -> Result<Self, DatabaseError> {
        let connection = Connection::open_in_memory()?;
        let mut db = Self { connection };
        db.initialize()?;
        Ok(db)
    }
    fn initialize(&mut self) -> Result<(), DatabaseError> {
        self.connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS schema_migrations(
                 version INTEGER PRIMARY KEY,
                 applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );",
        )?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: i64 = tx.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?;
        if current > 1 {
            return Err(DatabaseError::UnsupportedSchemaVersion(current));
        }
        if current == 0 {
            tx.execute_batch(
                "CREATE TABLE games(
                    app_id INTEGER PRIMARY KEY CHECK(app_id > 0),
                    name TEXT NOT NULL,
                    install_dir TEXT NOT NULL,
                    library_path TEXT NOT NULL,
                    last_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                 );
                 CREATE TABLE trainers(
                    id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    exe_path TEXT NOT NULL UNIQUE,
                    file_size INTEGER NOT NULL CHECK(file_size >= 0),
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                 );
                 CREATE TABLE settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE game_trainers(
                    app_id INTEGER NOT NULL REFERENCES games(app_id) ON DELETE CASCADE,
                    trainer_id INTEGER NOT NULL REFERENCES trainers(id) ON DELETE CASCADE,
                    enabled INTEGER NOT NULL DEFAULT 1,
                    PRIMARY KEY(app_id,trainer_id)
                 );
                 CREATE TABLE executions(
                    id INTEGER PRIMARY KEY,
                    app_id INTEGER NOT NULL REFERENCES games(app_id) ON DELETE CASCADE,
                    trainer_id INTEGER REFERENCES trainers(id) ON DELETE SET NULL,
                    started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    finished_at TEXT,
                    status TEXT NOT NULL,
                    exit_code INTEGER,
                    error TEXT
                 );",
            )?;
            tx.execute("INSERT INTO schema_migrations(version) VALUES (1)", [])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn upsert_game(&self, g: &GameRecord) -> Result<(), DatabaseError> {
        self.connection.execute("INSERT INTO games(app_id,name,install_dir,library_path,last_seen_at) VALUES(?1,?2,?3,?4,CURRENT_TIMESTAMP) ON CONFLICT(app_id) DO UPDATE SET name=excluded.name,install_dir=excluded.install_dir,library_path=excluded.library_path,last_seen_at=CURRENT_TIMESTAMP",params![g.app_id,g.name,g.install_dir,g.library_path])?;
        Ok(())
    }
    pub fn list_games(&self) -> Result<Vec<GameRecord>, DatabaseError> {
        let mut q = self.connection.prepare(
            "SELECT app_id,name,install_dir,library_path FROM games ORDER BY name COLLATE NOCASE",
        )?;
        let rows = q.query_map([], |r| {
            Ok(GameRecord {
                app_id: r.get(0)?,
                name: r.get(1)?,
                install_dir: r.get(2)?,
                library_path: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
    pub fn add_trainer(&self, name: &str, path: &str, size: u64) -> Result<i64, DatabaseError> {
        self.connection.execute("INSERT INTO trainers(name,exe_path,file_size) VALUES(?1,?2,?3) ON CONFLICT(exe_path) DO UPDATE SET name=excluded.name,file_size=excluded.file_size",params![name,path,size])?;
        Ok(self
            .connection
            .query_row("SELECT id FROM trainers WHERE exe_path=?1", [path], |r| {
                r.get(0)
            })?)
    }
    pub fn list_trainers(&self, app_id: Option<u32>) -> Result<Vec<TrainerRecord>, DatabaseError> {
        let mut q = if app_id.is_some() {
            self.connection.prepare("SELECT t.id,t.name,t.exe_path,t.file_size FROM trainers t JOIN game_trainers gt ON gt.trainer_id=t.id WHERE gt.app_id=?1 ORDER BY t.name COLLATE NOCASE")?
        } else {
            self.connection.prepare(
                "SELECT id,name,exe_path,file_size FROM trainers ORDER BY name COLLATE NOCASE",
            )?
        };
        let mapper = |r: &rusqlite::Row<'_>| {
            Ok(TrainerRecord {
                id: r.get(0)?,
                name: r.get(1)?,
                exe_path: r.get(2)?,
                file_size: r.get::<_, i64>(3)? as u64,
            })
        };
        let rows = if let Some(id) = app_id {
            q.query_map([id], mapper)?.collect::<Result<_, _>>()?
        } else {
            q.query_map([], mapper)?.collect::<Result<_, _>>()?
        };
        Ok(rows)
    }
    pub fn associate(&self, app_id: u32, trainer_id: i64) -> Result<(), DatabaseError> {
        self.connection.execute(
            "INSERT OR IGNORE INTO game_trainers(app_id,trainer_id) VALUES(?1,?2)",
            params![app_id, trainer_id],
        )?;
        Ok(())
    }
    pub fn remove_trainer(&self, id: i64) -> Result<(), DatabaseError> {
        self.connection
            .execute("DELETE FROM trainers WHERE id=?1", [id])?;
        Ok(())
    }
    pub fn start_execution(&self, app_id: u32, trainer_id: i64) -> Result<i64, DatabaseError> {
        self.connection.execute(
            "INSERT INTO executions(app_id,trainer_id,status) VALUES(?1,?2,'running')",
            params![app_id, trainer_id],
        )?;
        Ok(self.connection.last_insert_rowid())
    }
    pub fn finish_execution(
        &self,
        id: i64,
        exit_code: Option<i32>,
        error: Option<&str>,
    ) -> Result<(), DatabaseError> {
        let status = if error.is_some() {
            "error"
        } else if exit_code == Some(0) {
            "success"
        } else {
            "failed"
        };
        self.connection.execute("UPDATE executions SET finished_at=CURRENT_TIMESTAMP,status=?2,exit_code=?3,error=?4 WHERE id=?1",params![id,status,exit_code,error])?;
        Ok(())
    }
    pub fn history(&self) -> Result<Vec<ExecutionRecord>, DatabaseError> {
        let mut q=self.connection.prepare("SELECT id,app_id,trainer_id,started_at,finished_at,status,exit_code,error FROM executions ORDER BY id DESC LIMIT 100")?;
        let rows = q.query_map([], |r| {
            Ok(ExecutionRecord {
                id: r.get(0)?,
                app_id: r.get(1)?,
                trainer_id: r.get(2)?,
                started_at: r.get(3)?,
                finished_at: r.get(4)?,
                status: r.get(5)?,
                exit_code: r.get(6)?,
                error: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
    pub fn setting(&self, key: &str) -> Result<Option<String>, DatabaseError> {
        Ok(self
            .connection
            .query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), DatabaseError> {
        self.connection.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value])?;
        Ok(())
    }
    pub fn clear_setting(&self, key: &str) -> Result<(), DatabaseError> {
        self.connection
            .execute("DELETE FROM settings WHERE key=?1", [key])?;
        Ok(())
    }
}

#[cfg(unix)]
fn ensure_private_file(path: &Path) -> Result<(), DatabaseError> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut options = OpenOptions::new();
    options.write(true).create_new(true).mode(0o600);
    match options.open(path) {
        Ok(file) => drop(file),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(source) => {
            return Err(DatabaseError::FileAccess {
                path: path.to_path_buf(),
                source,
            })
        }
    }
    let metadata = fs::symlink_metadata(path).map_err(|source| DatabaseError::FileAccess {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.file_type().is_file() {
        return Err(DatabaseError::NotAFile(path.to_path_buf()));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|source| {
        DatabaseError::FileAccess {
            path: path.to_path_buf(),
            source,
        }
    })?;
    Ok(())
}

#[cfg(not(unix))]
fn ensure_private_file(_path: &Path) -> Result<(), DatabaseError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn game(id: u32, name: &str) -> GameRecord {
        GameRecord {
            app_id: id,
            name: name.into(),
            install_dir: format!("/games/{name}"),
            library_path: "/steam".into(),
        }
    }

    #[test]
    fn creates_empty_database_and_applies_schema_once() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.sqlite3");
        {
            let db = Database::open(&path).unwrap();
            assert!(db.list_games().unwrap().is_empty());
        }
        assert!(path.is_file());
        let db = Database::open(&path).unwrap();
        let versions: Vec<i64> = db
            .connection
            .prepare("SELECT version FROM schema_migrations")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(versions, [1]);
    }

    #[test]
    fn concurrent_first_open_applies_schema_once() {
        use std::sync::{Arc, Barrier};
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("concurrent.sqlite3");
        let barrier = Arc::new(Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let path = path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    Database::open(path).map(drop)
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap().unwrap();
        }
        let db = Database::open(&path).unwrap();
        let version: i64 = db
            .connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn initializes_an_existing_empty_database_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("precreated.sqlite3");
        std::fs::File::create(&path).unwrap();
        assert!(Database::open(&path)
            .unwrap()
            .list_games()
            .unwrap()
            .is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn database_file_is_created_with_private_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("private.sqlite3");
        Database::open(&path).unwrap();
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn existing_database_permissions_are_restricted() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("existing.sqlite3");
        std::fs::write(&path, b"").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        Database::open(&path).unwrap();
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn refuses_to_follow_a_database_symlink() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target.sqlite3");
        let link = temp.path().join("link.sqlite3");
        std::fs::write(&target, b"").unwrap();
        symlink(&target, &link).unwrap();
        assert!(matches!(
            Database::open(link),
            Err(DatabaseError::NotAFile(_))
        ));
        assert_eq!(std::fs::read(target).unwrap(), b"");
    }

    #[test]
    fn upserts_records_and_preserves_referential_integrity() {
        let db = Database::open_in_memory().unwrap();
        db.upsert_game(&game(1, "Old name")).unwrap();
        db.upsert_game(&game(1, "New name")).unwrap();
        assert_eq!(db.list_games().unwrap().len(), 1);
        assert_eq!(db.list_games().unwrap()[0].name, "New name");

        let trainer_id = db.add_trainer("trainer", "/trainer.exe", 12).unwrap();
        db.add_trainer("renamed", "/trainer.exe", 14).unwrap();
        db.associate(1, trainer_id).unwrap();
        assert_eq!(db.list_trainers(Some(1)).unwrap()[0].file_size, 14);
        assert!(db.associate(999, trainer_id).is_err());

        let run_id = db.start_execution(1, trainer_id).unwrap();
        db.finish_execution(run_id, Some(0), None).unwrap();
        assert_eq!(db.history().unwrap()[0].status, "success");

        db.remove_trainer(trainer_id).unwrap();
        assert!(db.list_trainers(Some(1)).unwrap().is_empty());
        let record = &db.history().unwrap()[0];
        assert_eq!(record.trainer_id, None);
        assert_eq!(record.app_id, 1);
    }

    #[test]
    fn corrupt_database_fails_without_replacing_contents() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("corrupt.sqlite3");
        std::fs::write(&path, b"not a sqlite database").unwrap();
        assert!(Database::open(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"not a sqlite database");
    }

    #[test]
    fn failed_migration_rolls_back_and_can_be_retried() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("retry.sqlite3");
        let raw = Connection::open(&path).unwrap();
        raw.execute_batch(
            "CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
             CREATE TRIGGER block_first_migration BEFORE INSERT ON schema_migrations
             BEGIN SELECT RAISE(ABORT, 'injected migration failure'); END;",
        )
        .unwrap();
        drop(raw);

        assert!(Database::open(&path).is_err());
        let raw = Connection::open(&path).unwrap();
        let games_table: i64 = raw
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='games'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(games_table, 0);
        raw.execute_batch("DROP TRIGGER block_first_migration;")
            .unwrap();
        drop(raw);

        assert!(Database::open(&path).is_ok());
    }

    #[test]
    fn rejects_unknown_future_migration_version() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("future.sqlite3");
        let raw = Connection::open(&path).unwrap();
        raw.execute_batch("CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);").unwrap();
        raw.execute("INSERT INTO schema_migrations(version) VALUES(99)", [])
            .unwrap();
        drop(raw);
        assert!(matches!(
            Database::open(path),
            Err(DatabaseError::UnsupportedSchemaVersion(99))
        ));
    }
}
