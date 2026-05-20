use std::path::PathBuf;
use anyhow::{Result, Context};
use tokio::fs;
use rusqlite::params;
use crate::state::State;

pub async fn move_model(state: &State, model_id: i64, target_library_id: i64) -> Result<()> {
    let (_model_name, model_relative_path, source_library_path) = {
        let conn = state.conn.lock().unwrap();
        conn.query_row(
            "SELECT m.name, m.path, l.path 
             FROM models m 
             JOIN libraries l ON m.library_id = l.id 
             WHERE m.id = ?",
            params![model_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )?
    };

    let target_library_path: String = {
        let conn = state.conn.lock().unwrap();
        conn.query_row(
            "SELECT path FROM libraries WHERE id = ?",
            params![target_library_id],
            |row| row.get(0),
        )?
    };

    let source_full_path = PathBuf::from(&source_library_path).join(&model_relative_path);
    let target_full_path = PathBuf::from(&target_library_path).join(&model_relative_path);

    if let Some(parent) = target_full_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    if let Err(e) = fs::rename(&source_full_path, &target_full_path).await {
        if e.kind() == std::io::ErrorKind::CrossesDevices {
            fs::copy(&source_full_path, &target_full_path)
                .await
                .context("Failed to copy model during cross-drive move")?;
            fs::remove_file(&source_full_path)
                .await
                .context("Failed to remove source file after cross-drive copy")?;
        } else {
            return Err(e).context("Failed to move model file");
        }
    }

    state.conn.lock().unwrap().execute(
        "UPDATE models SET library_id = ? WHERE id = ?",
        params![target_library_id, model_id],
    )?;

    Ok(())
}
