use rusqlite::Connection;
use std::path::PathBuf;
use anyhow::{Result, Context};
use serde::{Serialize, Deserialize};
use std::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Machine {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub ssh_port: u16,
    pub ssh_user: Option<String>,
    pub ssh_key: Option<String>,
    pub gpu_type: String,
    pub lib_path: Option<String>,
    pub is_local: bool,
    pub description: Option<String>,
    pub last_seen: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RunStatus {
    Queued,
    Running,
    Done,
    Failed,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Queued => "queued",
            RunStatus::Running => "running",
            RunStatus::Done => "done",
            RunStatus::Failed => "failed",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "running" => RunStatus::Running,
            "done" => RunStatus::Done,
            "failed" => RunStatus::Failed,
            _ => RunStatus::Queued,
        }
    }
}

impl Serialize for RunStatus {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RunStatus {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(RunStatus::from_str(&s))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Run {
    pub id: i64,
    pub machine_id: i64,
    pub endpoint_id: Option<i64>,
    pub kind: String,       // "bench" | "eval"
    pub task: String,       // "llama-bench" | "micro_swe" | "tau2_telecom" | ...
    pub model_id: i64,
    pub model_label: String,
    pub model_path: String, // absolute path on the machine (already resolved at queue time)
    pub config: String,     // JSON of params
    pub status: RunStatus,
    pub stdout: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RunResult {
    pub id: i64,
    pub run_id: i64,
    pub metric: String,
    pub value: f64,
    pub unit: Option<String>,
    pub extra: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: i64,
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EvalEndpoint {
    pub id: i64,
    pub name: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub created_at: String,
}

pub struct State {
    pub conn: Mutex<Connection>,
}

fn data_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("BACKYARD_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    if let Ok(url) = std::env::var("DATABASE_URL") {
        // support DATABASE_URL=/data/data.db or sqlite:///data/data.db
        let p = url.strip_prefix("sqlite://").unwrap_or(&url);
        if let Some(parent) = PathBuf::from(p).parent() {
            if !parent.as_os_str().is_empty() {
                return Ok(parent.to_path_buf());
            }
        }
    }
    let home_dir = if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE").context("USERPROFILE env var not set")?
    } else {
        std::env::var("HOME").context("HOME env var not set")?
    };
    Ok(PathBuf::from(&home_dir).join(".backyard"))
}

impl State {
    pub fn init() -> Result<Self> {
        let backyard_dir = data_dir()?;
        std::fs::create_dir_all(&backyard_dir)?;

        let db_path = if let Ok(url) = std::env::var("DATABASE_URL") {
            let p = url.strip_prefix("sqlite://").unwrap_or(&url);
            PathBuf::from(p)
        } else {
            backyard_dir.join("data.db")
        };
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)?;

        let state = Self {
            conn: Mutex::new(conn),
        };
        state.create_tables()?;
        state.ensure_local_machine()?;

        Ok(state)
    }

    fn column_exists(&self, table: &str, column: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
        let cols = stmt.query_map([], |row| row.get::<_, String>(1))?;
        for col in cols {
            if col? == column {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn create_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        

        

        

        conn.execute(
            "CREATE TABLE IF NOT EXISTS machines (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                host TEXT NOT NULL DEFAULT 'local',
                ssh_port INTEGER NOT NULL DEFAULT 22,
                ssh_user TEXT,
                ssh_key TEXT,
                gpu_type TEXT NOT NULL DEFAULT 'nvidia',
                lib_path TEXT,
                is_local INTEGER NOT NULL DEFAULT 0,
                description TEXT DEFAULT '',
                last_seen TEXT
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS runs (
                id INTEGER PRIMARY KEY,
                machine_id INTEGER NOT NULL REFERENCES machines(id),
                endpoint_id INTEGER,
                kind TEXT NOT NULL,
                task TEXT NOT NULL,
                model_id INTEGER NOT NULL,
                model_label TEXT NOT NULL,
                model_path TEXT NOT NULL,
                config TEXT NOT NULL DEFAULT '{}',
                status TEXT NOT NULL DEFAULT 'queued',
                stdout TEXT,
                error TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                started_at TEXT,
                finished_at TEXT
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS results (
                id INTEGER PRIMARY KEY,
                run_id INTEGER NOT NULL REFERENCES runs(id),
                metric TEXT NOT NULL,
                value REAL NOT NULL,
                unit TEXT,
                extra TEXT
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY,
                username TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                token TEXT PRIMARY KEY,
                user_id INTEGER NOT NULL REFERENCES users(id),
                expires_at TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS eval_endpoints (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                base_url TEXT NOT NULL,
                api_key TEXT,
                model TEXT,
                provider TEXT DEFAULT 'generic',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            [],
        )?;

        // Enable WAL mode for better concurrent performance
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;

        // Migrations for columns added after the fact
        drop(conn);
        if !self.column_exists("runs", "endpoint_id")? {
            let conn = self.conn.lock().unwrap();
            conn.execute("ALTER TABLE runs ADD COLUMN endpoint_id INTEGER", [])?;
        }
        if !self.column_exists("eval_endpoints", "provider")? {
            let conn = self.conn.lock().unwrap();
            conn.execute("ALTER TABLE eval_endpoints ADD COLUMN provider TEXT DEFAULT 'generic'", [])?;
        }

        Ok(())
    }

    pub fn ensure_local_machine(&self) -> Result<()> {
        let default_path = data_dir()?.join("models");
        let path_str = default_path.to_str().context("Invalid path")?;

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO machines (name, host, gpu_type, is_local, description) VALUES ('this-machine', 'local', 'nvidia', 1, 'This machine')",
            [],
        )?;
        // Fill in the default models path when the local machine has none set
        conn.execute(
            "UPDATE machines SET lib_path = ? WHERE is_local = 1 AND (lib_path IS NULL OR lib_path = '')",
            [path_str],
        )?;
        Ok(())
    }

    pub fn local_machine_id(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let id = conn.query_row(
            "SELECT id FROM machines WHERE is_local = 1 LIMIT 1",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(id)
    }

    pub fn list_machines(&self) -> Result<Vec<Machine>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, host, ssh_port, ssh_user, ssh_key, gpu_type, lib_path, is_local, description, last_seen FROM machines ORDER BY is_local DESC, name",
        )?;
        let iter = stmt.query_map([], |row| {
            Ok(Machine {
                id: row.get(0)?,
                name: row.get(1)?,
                host: row.get(2)?,
                ssh_port: row.get(3)?,
                ssh_user: row.get(4)?,
                ssh_key: row.get(5)?,
                gpu_type: row.get(6)?,
                lib_path: row.get(7)?,
                is_local: row.get::<_, i64>(8)? == 1,
                description: row.get(9)?,
                last_seen: row.get(10)?,
            })
        })?;
        let mut machines = Vec::new();
        for m in iter {
            machines.push(m?);
        }
        Ok(machines)
    }

    pub fn list_runs(&self, limit: Option<u32>) -> Result<Vec<Run>> {
        let conn = self.conn.lock().unwrap();
        let sql = match limit {
            Some(n) => format!(
                "SELECT id, machine_id, endpoint_id, kind, task, model_id, model_label, model_path, config, status, stdout, error, created_at, started_at, finished_at FROM runs ORDER BY id DESC LIMIT {}",
                n
            ),
            None => "SELECT id, machine_id, endpoint_id, kind, task, model_id, model_label, model_path, config, status, stdout, error, created_at, started_at, finished_at FROM runs ORDER BY id DESC".to_string(),
        };
        let mut stmt = conn.prepare(&sql)?;
        let iter = stmt.query_map([], |row| Ok(row_to_run(row)?))?;
        let mut runs = Vec::new();
        for r in iter {
            runs.push(r?);
        }
        Ok(runs)
    }

    pub fn get_run(&self, id: i64) -> Result<Run> {
        let conn = self.conn.lock().unwrap();
        let run = conn.query_row(
            "SELECT id, machine_id, endpoint_id, kind, task, model_id, model_label, model_path, config, status, stdout, error, created_at, started_at, finished_at FROM runs WHERE id = ?",
            [id],
            |row| Ok(row_to_run(row)?),
        )?;
        Ok(run)
    }

    pub fn list_results(&self, run_id: i64) -> Result<Vec<RunResult>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, run_id, metric, value, unit, extra FROM results WHERE run_id = ? ORDER BY id",
        )?;
        let iter = stmt.query_map([run_id], |row| {
            Ok(RunResult {
                id: row.get(0)?,
                run_id: row.get(1)?,
                metric: row.get(2)?,
                value: row.get(3)?,
                unit: row.get(4)?,
                extra: row.get(5)?,
            })
        })?;
        let mut results = Vec::new();
        for r in iter {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn list_eval_endpoints(&self) -> Result<Vec<EvalEndpoint>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, base_url, api_key, model, provider, created_at FROM eval_endpoints ORDER BY name",
        )?;
        let iter = stmt.query_map([], |row| {
            Ok(EvalEndpoint {
                id: row.get(0)?,
                name: row.get(1)?,
                base_url: row.get(2)?,
                api_key: row.get(3)?,
                model: row.get(4)?,
                provider: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        let mut endpoints = Vec::new();
        for e in iter {
            endpoints.push(e?);
        }
        Ok(endpoints)
    }

    pub fn get_eval_endpoint(&self, id: i64) -> Result<EvalEndpoint> {
        let conn = self.conn.lock().unwrap();
        let endpoint = conn.query_row(
            "SELECT id, name, base_url, api_key, model, provider, created_at FROM eval_endpoints WHERE id = ?",
            [id],
            |row| {
                Ok(EvalEndpoint {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    base_url: row.get(2)?,
                    api_key: row.get(3)?,
                    model: row.get(4)?,
                    provider: row.get(5)?,
                    created_at: row.get(6)?,
                })
            },
        )?;
        Ok(endpoint)
    }

    pub fn add_eval_endpoint_with_provider(
        &self,
        name: &str,
        base_url: &str,
        api_key: Option<&str>,
        model: Option<&str>,
        provider: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO eval_endpoints (name, base_url, api_key, model, provider) VALUES (?, ?, ?, ?, ?)",
            rusqlite::params![name, base_url, api_key, model, provider.unwrap_or("generic")],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn delete_eval_endpoint(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM eval_endpoints WHERE id = ?", [id])?;
        Ok(())
    }
}

fn row_to_run(row: &rusqlite::Row) -> rusqlite::Result<Run> {
    Ok(Run {
        id: row.get(0)?,
        machine_id: row.get(1)?,
        endpoint_id: row.get(2)?,
        kind: row.get(3)?,
        task: row.get(4)?,
        model_id: row.get(5)?,
        model_label: row.get(6)?,
        model_path: row.get(7)?,
        config: row.get(8)?,
        status: RunStatus::from_str(&row.get::<_, String>(9)?),
        stdout: row.get(10)?,
        error: row.get(11)?,
        created_at: row.get(12)?,
        started_at: row.get(13)?,
        finished_at: row.get(14)?,
    })
}
