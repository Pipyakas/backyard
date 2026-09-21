use anyhow::Result;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::RngCore;
use rusqlite::Connection;

use crate::state::User;

pub const SESSION_COOKIE: &str = "backyard_session";
const SESSION_TTL_DAYS: i64 = 30;

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?
        .to_string();
    Ok(hash)
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn count_users(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?)
}

pub fn create_user(conn: &Connection, username: &str, password: &str) -> Result<i64> {
    let hash = hash_password(password)?;
    conn.execute(
        "INSERT INTO users (username, password_hash) VALUES (?, ?)",
        rusqlite::params![username, hash],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn authenticate(conn: &Connection, username: &str, password: &str) -> Result<Option<User>> {
    let row = conn
        .query_row(
            "SELECT id, username, password_hash FROM users WHERE username = ?",
            [username],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;

    Ok(row.and_then(|(id, uname, hash)| {
        if verify_password(password, &hash) {
            Some(User { id, username: uname })
        } else {
            None
        }
    }))
}

pub fn create_session(conn: &Connection, user_id: i64) -> Result<String> {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let token = bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();

    conn.execute(
        "INSERT INTO sessions (token, user_id, expires_at) VALUES (?, ?, datetime('now', ?))",
        rusqlite::params![token, user_id, format!("+{} days", SESSION_TTL_DAYS)],
    )?;
    Ok(token)
}

pub fn validate_session(conn: &Connection, token: &str) -> Result<Option<User>> {
    conn.execute(
        "DELETE FROM sessions WHERE expires_at < datetime('now')",
        [],
    )?;
    let user = conn
        .query_row(
            "SELECT u.id, u.username FROM sessions s JOIN users u ON s.user_id = u.id WHERE s.token = ? AND s.expires_at >= datetime('now')",
            [token],
            |row| {
                Ok(User {
                    id: row.get(0)?,
                    username: row.get(1)?,
                })
            },
        )
        .optional()?;
    Ok(user)
}

pub fn delete_session(conn: &Connection, token: &str) -> Result<()> {
    conn.execute("DELETE FROM sessions WHERE token = ?", [token])?;
    Ok(())
}

use rusqlite::OptionalExtension;
