use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::task::JoinHandle;

use crate::state::State;

pub fn spawn(state: Arc<State>) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            if let Err(e) = process_next(&state).await {
                eprintln!("[worker] error: {}", e);
            }
        }
    })
}

async fn process_next(state: &State) -> Result<bool> {
    let claimed = {
        let conn = state.conn.lock().unwrap();
        conn.execute(
            "UPDATE runs SET status = 'running', started_at = datetime('now') WHERE id = (SELECT id FROM runs WHERE status = 'queued' ORDER BY id LIMIT 1) AND status = 'queued'",
            [],
        )?
    };
    if claimed == 0 {
        return Ok(false);
    }

    let run_id = {
        let conn = state.conn.lock().unwrap();
        conn.query_row(
            "SELECT id FROM runs WHERE status = 'running' ORDER BY started_at DESC LIMIT 1",
            [],
            |row| row.get::<_, i64>(0),
        )?
    };

    let run = state.get_run(run_id)?;
    let endpoint = match run.endpoint_id {
        Some(ep_id) => Some(state.get_eval_endpoint(ep_id)?),
        None => None,
    };

    let where_str = endpoint
        .as_ref()
        .map(|ep| format!("endpoint {}", ep.name))
        .unwrap_or_else(|| format!("run #{}", run.id));
    eprintln!("[worker] running #{} ({} / {}) on {}", run.id, run.kind, run.task, where_str);

    let result: Result<(String, Vec<crate::state::RunResult>)> = if let Some(ep) = endpoint {
        if run.kind == "bench" {
            crate::bench_http::run_bench_http(&run, &ep).await
        } else {
            // eval harnesses are blocking subprocesses — run on blocking pool
            let run_clone = run.clone();
            let ep_clone = ep.clone();
            tokio::task::spawn_blocking(move || crate::harness::run_harness(&run_clone, &ep_clone))
                .await
                .context("worker task panicked")?
        }
    } else {
        anyhow::bail!("Run #{} has no endpoint_id — legacy machine-based runs are no longer supported", run.id)
    };

    let conn = state.conn.lock().unwrap();
    match result {
        Ok((stdout, results)) => {
            for r in &results {
                conn.execute(
                    "INSERT INTO results (run_id, metric, value, unit, extra) VALUES (?, ?, ?, ?, ?)",
                    rusqlite::params![run_id, r.metric, r.value, r.unit, r.extra],
                )?;
            }
            conn.execute(
                "UPDATE runs SET status = 'done', stdout = ?, finished_at = datetime('now') WHERE id = ?",
                rusqlite::params![stdout, run_id],
            )?;
        }
        Err(e) => {
            eprintln!("[worker] run #{} failed: {}", run_id, e);
            conn.execute(
                "UPDATE runs SET status = 'failed', error = ?, finished_at = datetime('now') WHERE id = ?",
                rusqlite::params![e.to_string(), run_id],
            )?;
        }
    }
    drop(conn);

    Ok(true)
}
