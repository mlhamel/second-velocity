use std::path::Path;

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use rusqlite::{Connection, OptionalExtension, params};

fn default_active_quests_json() -> String {
    "[]".to_owned()
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlayerState {
    pub credits: u32,
    pub cargo: Vec<(String, u32)>,
    pub ship_id: String,
    pub system: String,
    pub position: (f32, f32),
    pub fuel: f32,
    pub shields: f32,
    pub armor: f32,
    #[serde(default = "default_active_quests_json")]
    pub active_quests_json: String,
    #[serde(default)]
    pub equipped_outfits: Option<Vec<String>>,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            credits: 500,
            cargo: Vec::new(),
            ship_id: "shuttle".to_owned(),
            system: "sol".to_owned(),
            position: (0.0, 0.0),
            fuel: 120.0,
            shields: 40.0,
            armor: 25.0,
            active_quests_json: default_active_quests_json(),
            equipped_outfits: None,
        }
    }
}

#[derive(Debug)]
pub enum StorageError {
    Sql(rusqlite::Error),
    Argon(argon2::password_hash::Error),
    NotFound,
}

impl std::error::Error for StorageError {}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sql(e) => write!(f, "storage: {e}"),
            Self::Argon(e) => write!(f, "hash: {e}"),
            Self::NotFound => write!(f, "not found"),
        }
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sql(e)
    }
}

impl From<argon2::password_hash::Error> for StorageError {
    fn from(e: argon2::password_hash::Error) -> Self {
        Self::Argon(e)
    }
}

pub trait Storage: Send + Sync + 'static {
    fn create_account(&self, username: &str, password: &str) -> Result<PlayerState, StorageError>;
    fn verify_login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Option<PlayerState>, StorageError>;
    fn save_player(&self, username: &str, state: &PlayerState) -> Result<(), StorageError>;
    fn close(&self);
}

pub struct SqliteStorage {
    conn: std::sync::Mutex<Connection>,
}

impl SqliteStorage {
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS accounts (
                username TEXT PRIMARY KEY,
                password_hash TEXT NOT NULL,
                credits INTEGER NOT NULL DEFAULT 500,
                cargo_json TEXT NOT NULL DEFAULT '[]',
                ship_id TEXT NOT NULL DEFAULT 'shuttle',
                system TEXT NOT NULL DEFAULT 'sol',
                pos_x REAL NOT NULL DEFAULT 0,
                pos_y REAL NOT NULL DEFAULT 0,
                fuel REAL NOT NULL DEFAULT 120,
                shields REAL NOT NULL DEFAULT 40,
                armor REAL NOT NULL DEFAULT 25,
                active_quests_json TEXT NOT NULL DEFAULT '[]',
                equipped_outfits_json TEXT
            );",
        )?;
        Self::migrate_columns(&conn)?;
        Ok(Self {
            conn: std::sync::Mutex::new(conn),
        })
    }

    fn migrate_columns(conn: &Connection) -> Result<(), StorageError> {
        let mut stmt = conn.prepare("PRAGMA table_info(accounts)")?;
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if !cols.iter().any(|c| c == "active_quests_json") {
            conn.execute(
                "ALTER TABLE accounts ADD COLUMN active_quests_json TEXT NOT NULL DEFAULT '[]'",
                [],
            )?;
        }
        if !cols.iter().any(|c| c == "equipped_outfits_json") {
            conn.execute(
                "ALTER TABLE accounts ADD COLUMN equipped_outfits_json TEXT",
                [],
            )?;
        }
        Ok(())
    }

    pub fn open_memory() -> Result<Self, StorageError> {
        Self::open(Path::new(":memory:"))
    }

    fn row_to_state(row: &rusqlite::Row<'_>) -> rusqlite::Result<PlayerState> {
        let cargo_json: String = row.get("cargo_json")?;
        let cargo: Vec<(String, u32)> = serde_json::from_str(&cargo_json).unwrap_or_default();
        let active_quests_json: String = row
            .get("active_quests_json")
            .unwrap_or_else(|_| default_active_quests_json());
        let raw: Option<String> = row.get("equipped_outfits_json")?;
        let equipped_outfits = raw
            .map(|raw| serde_json::from_str::<Vec<String>>(&raw))
            .transpose()
            .map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?;
        Ok(PlayerState {
            credits: row.get("credits")?,
            cargo,
            ship_id: row.get("ship_id")?,
            system: row.get("system")?,
            position: (row.get("pos_x")?, row.get("pos_y")?),
            fuel: row.get("fuel")?,
            shields: row.get("shields")?,
            armor: row.get("armor")?,
            active_quests_json,
            equipped_outfits,
        })
    }
}

fn state_columns() -> &'static str {
    "credits, cargo_json, ship_id, system, pos_x, pos_y, fuel, shields, armor, active_quests_json, equipped_outfits_json"
}

impl Storage for SqliteStorage {
    fn create_account(&self, username: &str, password: &str) -> Result<PlayerState, StorageError> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)?
            .to_string();

        let state = PlayerState::default();
        let cargo_json = serde_json::to_string(&state.cargo).unwrap_or_else(|_| "[]".into());
        self.conn.lock().unwrap().execute(
            "INSERT INTO accounts (username, password_hash, credits, cargo_json, ship_id, system, pos_x, pos_y, fuel, shields, armor, active_quests_json, equipped_outfits_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                username,
                hash,
                state.credits,
                cargo_json,
                state.ship_id,
                state.system,
                state.position.0,
                state.position.1,
                state.fuel,
                state.shields,
                state.armor,
                state.active_quests_json,
                Option::<String>::None,
            ],
        )?;
        Ok(state)
    }

    fn verify_login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Option<PlayerState>, StorageError> {
        let row: Option<(String, String)> = {
            let conn = self.conn.lock().unwrap();
            conn.query_row(
                "SELECT password_hash, cargo_json FROM accounts WHERE username = ?1",
                params![username],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
        };

        let Some((stored_hash, _)) = row else {
            return Ok(None);
        };

        let parsed = PasswordHash::new(&stored_hash)?;
        if Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_err()
        {
            return Err(StorageError::NotFound);
        }

        let state: Option<PlayerState> = {
            let conn = self.conn.lock().unwrap();
            conn.query_row(
                &format!(
                    "SELECT {cols} FROM accounts WHERE username = ?1",
                    cols = state_columns()
                ),
                params![username],
                Self::row_to_state,
            )
            .optional()?
        };
        Ok(state)
    }

    fn save_player(&self, username: &str, state: &PlayerState) -> Result<(), StorageError> {
        let cargo_json = serde_json::to_string(&state.cargo).unwrap_or_else(|_| "[]".into());
        let outfits_json = state
            .equipped_outfits
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
        let changed = self.conn.lock().unwrap().execute(
            "UPDATE accounts SET credits=?2, cargo_json=?3, ship_id=?4, system=?5, pos_x=?6, pos_y=?7, fuel=?8, shields=?9, armor=?10, active_quests_json=?11, equipped_outfits_json=?12
             WHERE username=?1",
            params![
                username,
                state.credits,
                cargo_json,
                state.ship_id,
                state.system,
                state.position.0,
                state.position.1,
                state.fuel,
                state.shields,
                state.armor,
                state.active_quests_json,
                outfits_json,
            ],
        )?;
        if changed == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    fn close(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_login() {
        let storage = SqliteStorage::open_memory().expect("open");
        let state = storage.create_account("alice", "pass123").expect("create");
        assert_eq!(state.credits, 500);
        assert_eq!(state.active_quests_json, "[]");

        let loaded = storage
            .verify_login("alice", "pass123")
            .expect("verify")
            .expect("found");
        assert_eq!(loaded.credits, 500);

        let bad = storage.verify_login("alice", "wrong");
        assert!(
            matches!(bad, Err(StorageError::NotFound)),
            "wrong password must return NotFound, got {bad:?}"
        );

        let missing = storage.verify_login("bob", "pass").expect("verify");
        assert!(missing.is_none());
    }

    #[test]
    fn loadout_null_empty_and_equipped_states_roundtrip_distinctly() {
        let storage = SqliteStorage::open_memory().unwrap();
        let mut saved = storage
            .create_account("loadout-fixture", "fixture-only")
            .unwrap();
        assert_eq!(saved.equipped_outfits, None);
        for outfits in [
            Some(vec![]),
            Some(vec!["light-blaster".into(), "shield-generator-1".into()]),
            None,
        ] {
            saved.equipped_outfits = outfits.clone();
            storage.save_player("loadout-fixture", &saved).unwrap();
            let restored = storage
                .verify_login("loadout-fixture", "fixture-only")
                .unwrap()
                .unwrap();
            assert_eq!(restored.equipped_outfits, outfits);
        }
    }

    #[test]
    fn corrupt_equipment_json_and_missing_save_accounts_are_errors() {
        let storage = SqliteStorage::open_memory().unwrap();
        storage
            .create_account("corrupt-fixture", "fixture-only")
            .unwrap();
        storage.conn.lock().unwrap().execute(
            "UPDATE accounts SET equipped_outfits_json='not JSON' WHERE username='corrupt-fixture'", [],
        ).unwrap();
        assert!(matches!(
            storage.verify_login("corrupt-fixture", "fixture-only"),
            Err(StorageError::Sql(_))
        ));
        assert!(matches!(
            storage.save_player("missing", &PlayerState::default()),
            Err(StorageError::NotFound)
        ));
    }

    #[test]
    fn save_and_reload() {
        let storage = SqliteStorage::open_memory().expect("open");
        storage.create_account("carol", "pw").expect("create");

        let mut state = PlayerState {
            credits: 999,
            cargo: vec![("food".to_owned(), 10)],
            ship_id: "turncoat".to_owned(),
            system: "alpha-centauri".to_owned(),
            position: (200.0, 300.0),
            fuel: 95.0,
            shields: 30.0,
            armor: 20.0,
            active_quests_json: r#"{"quests":[],"completed":["newhaven-relief"]}"#.to_owned(),
            equipped_outfits: Some(vec!["light-blaster".into()]),
        };
        storage.save_player("carol", &state).expect("save");

        let loaded = storage
            .verify_login("carol", "pw")
            .expect("verify")
            .expect("found");
        assert_eq!(loaded.credits, 999);
        assert_eq!(loaded.cargo, state.cargo);
        assert_eq!(loaded.ship_id, "turncoat");
        assert_eq!(loaded.system, "alpha-centauri");
        assert_eq!(loaded.position, (200.0, 300.0));
        assert_eq!(loaded.active_quests_json, state.active_quests_json);

        state.credits = 500;
        storage.save_player("carol", &state).expect("save2");
        let reloaded = storage
            .verify_login("carol", "pw")
            .expect("verify")
            .expect("found");
        assert_eq!(reloaded.credits, 500);
    }

    #[test]
    fn legacy_schema_migrates_active_quests_column() {
        let path = std::env::temp_dir().join(format!(
            "evr-storage-migrate-test-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).expect("open legacy db");
            conn.execute_batch(
                "CREATE TABLE accounts (
                    username TEXT PRIMARY KEY,
                    password_hash TEXT NOT NULL,
                    credits INTEGER NOT NULL DEFAULT 500,
                    cargo_json TEXT NOT NULL DEFAULT '[]',
                    ship_id TEXT NOT NULL DEFAULT 'shuttle',
                    system TEXT NOT NULL DEFAULT 'sol',
                    pos_x REAL NOT NULL DEFAULT 0,
                    pos_y REAL NOT NULL DEFAULT 0,
                    fuel REAL NOT NULL DEFAULT 120,
                    shields REAL NOT NULL DEFAULT 40,
                    armor REAL NOT NULL DEFAULT 25
                );",
            )
            .expect("create legacy table");
        }
        let storage = SqliteStorage::open(&path).expect("open with migration");
        let created = storage
            .create_account("legacy_user", "pw")
            .expect("create after migration");
        assert_eq!(created.active_quests_json, "[]");
        assert_eq!(created.equipped_outfits, None);
        let _ = std::fs::remove_file(&path);
    }
}
