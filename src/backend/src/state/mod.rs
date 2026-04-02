use rusqlite::{params, Connection};
use std::path::PathBuf;
use anyhow::{Result, Context};
use serde::{Serialize, Deserialize};
use std::sync::Mutex;

pub mod fs;

#[derive(Debug, Serialize, Deserialize)]
pub struct Library {
    pub id: i64,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Model {
    pub id: i64,
    pub name: String,
    pub library_id: i64,
    pub path: String, // Relative to library root
    pub format: String,
    pub family: Option<String>,
    pub quant: Option<String>,
}

pub struct State {
    pub conn: Mutex<Connection>,
}

impl State {
    pub fn init() -> Result<Self> {
        let home_dir = std::env::var("HOME").context("HOME env var not set")?;
        let backyard_dir = PathBuf::from(&home_dir).join(".backyard");
        std::fs::create_dir_all(&backyard_dir)?;
        
        let db_path = backyard_dir.join("data.db");
        let conn = Connection::open(db_path)?;
        
        let state = Self { conn: Mutex::new(conn) };
        state.create_tables()?;
        state.ensure_default_library(&home_dir)?;
        
        Ok(state)
    }

    fn create_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS libraries (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS models (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                library_id INTEGER NOT NULL,
                path TEXT NOT NULL,
                format TEXT NOT NULL,
                family TEXT,
                quant TEXT,
                FOREIGN KEY(library_id) REFERENCES libraries(id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS engines (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                supported_formats TEXT NOT NULL,
                default_port INTEGER NOT NULL
            )",
            [],
        )?;

        Ok(())
    }

    fn ensure_default_library(&self, home_dir: &str) -> Result<()> {
        let default_path = PathBuf::from(home_dir).join(".backyard").join("models");
        std::fs::create_dir_all(&default_path)?;
        
        let path_str = default_path.to_str().context("Invalid path")?;
        
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO libraries (name, path) VALUES (?, ?)",
            params!["Default", path_str],
        )?;
        
        Ok(())
    }

    pub fn list_models(&self) -> Result<Vec<Model>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name, library_id, path, format, family, quant FROM models")?;
        let model_iter = stmt.query_map([], |row| {
            Ok(Model {
                id: row.get(0)?,
                name: row.get(1)?,
                library_id: row.get(2)?,
                path: row.get(3)?,
                format: row.get(4)?,
                family: row.get(5)?,
                quant: row.get(6)?,
            })
        })?;

        let mut models = Vec::new();
        for model in model_iter {
            models.push(model?);
        }
        Ok(models)
    }
}
