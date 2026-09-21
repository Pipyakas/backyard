use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use tower_http::services::ServeDir;
use crate::state::State as AppState;
use crate::auth;

fn frontend_dist() -> String {
    std::env::var("FRONTEND_DIST").unwrap_or_else(|_| {
        // In container: /app/frontend/dist; in dev: <manifest>/../frontend/dist
        let dev = format!("{}/../frontend/dist", env!("CARGO_MANIFEST_DIR"));
        if std::path::Path::new(&dev).exists() { dev } else { "/app/frontend/dist".into() }
    })
}

pub struct ServerState {
    pub app_state: Arc<AppState>,
}

type RouterState = Arc<ServerState>;

pub async fn start_server(app_state: Arc<AppState>) -> Result<()> {
    let server_state = Arc::new(ServerState { app_state: app_state.clone() });
    crate::worker::spawn(app_state);

    let host = std::env::var("BACKYARD_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("PORT")
        .or_else(|_| std::env::var("BACKYARD_PORT"))
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let auth_disabled = std::env::var("BACKYARD_AUTH_DISABLED").map(|v| v == "1" || v.to_lowercase() == "true").unwrap_or(false);

    // Public routes (no auth)
    let public = Router::new()
        .route("/api/health", get(health_api))
        .route("/api/machines", get(list_machines))
        .route("/api/runs", get(list_runs))
        .route("/api/runs/{id}", get(get_run_api))
        .route("/api/runs/{id}/results", get(get_run_results_api))
        .route("/api/leaderboard", get(leaderboard))
        .route("/api/endpoints", get(list_endpoints))
        .route("/api/evals/endpoints", get(list_endpoints))
        .route("/api/auth/status", get(auth_status_api))
        .route("/api/auth/me", get(auth_me_api))
        .route("/api/auth/login", post(login_api))
        .route("/api/auth/local/setup", post(setup_api))
        .route("/api/auth/logout", post(logout_api));

    // Admin routes (require auth unless disabled)
    let mut admin = Router::new()
        .route("/api/benchmarks", post(create_benchmark_api))
        .route("/api/runs", post(create_run_legacy_api))
        .route("/api/endpoints", post(add_endpoint_api))
        .route("/api/evals/endpoints", post(add_endpoint_api))
        .route("/api/endpoints/{id}", axum::routing::delete(delete_endpoint_api))
        .route("/api/evals/endpoints/{id}", axum::routing::delete(delete_endpoint_api))
        .route("/api/endpoints/{id}/health", post(probe_endpoint_api))
        .route("/api/evals/runs", post(create_eval_run_api))
        .route("/api/system/reset", post(reset_db_api));

    if !auth_disabled {
        admin = admin.route_layer(middleware::from_fn_with_state(server_state.clone(), require_admin));
    }

    let dist = frontend_dist();
    // Fallback source tree for /static (dev runs and images built before the
    // dist/static copy step): <manifest>/../frontend/static.
    let src_static = format!("{}/../frontend/static", env!("CARGO_MANIFEST_DIR"));
    let dist_clone = dist.clone();
    let app = public
        .merge(admin)
        .route("/", get(serve_index))
        .route("/auth", get(serve_auth))
        .route("/benchmarks", get(serve_index))
        .route("/evals", get(serve_index))
        .route("/endpoints", get(serve_index))
        .route("/settings", get(serve_index))
        .nest_service("/assets", ServeDir::new(format!("{}/assets", dist)))
        .nest_service(
            "/static",
            ServeDir::new(format!("{}/static", dist_clone)).fallback(ServeDir::new(src_static)),
        )
        .fallback(get(serve_index))
        .with_state(server_state);

    let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, port)).await?;
    eprintln!("Backyard listening on http://{}:{} (auth_disabled={})", host, port, auth_disabled);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn serve_index() -> impl IntoResponse {
    let dist = frontend_dist();
    let content = tokio::fs::read_to_string(format!("{}/index.html", dist))
        .await
        .or_else(|_| std::fs::read_to_string(format!("{}/index.html", dist)))
        .or_else(|_| std::fs::read_to_string(format!("{}/templates/index.html", dist)))
        .unwrap_or_else(|_| "<h1>Frontend not built — run `npm run build` in src/frontend</h1>".into());
    axum::response::Html(content)
}

async fn serve_auth() -> impl IntoResponse {
    let dist = frontend_dist();
    let content = tokio::fs::read_to_string(format!("{}/auth.html", dist))
        .await
        .or_else(|_| std::fs::read_to_string(format!("{}/auth.html", dist)))
        .or_else(|_| std::fs::read_to_string(format!("{}/templates/auth.html", dist)))
        .unwrap_or_else(|_| "<h1>Auth page missing</h1>".into());
    axum::response::Html(content)
}

async fn health_api() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "success": true, "status": "ok" }))
}

// ---- auth middleware ----
fn cookie_value(header: &str, name: &str) -> Option<String> {
    header.split(';').find_map(|part| {
        let part = part.trim();
        let (k, v) = part.split_once('=')?;
        if k.trim() == name { Some(v.trim().to_string()) } else { None }
    })
}

async fn require_admin(State(state): State<RouterState>, req: Request<Body>, next: Next) -> Response {
    let token = req.headers().get(header::COOKIE).and_then(|v| v.to_str().ok()).and_then(|c| cookie_value(c, auth::SESSION_COOKIE));
    let authorized = match token {
        Some(t) => {
            let conn = state.app_state.conn.lock().unwrap();
            auth::validate_session(&conn, &t).map(|u| u.is_some()).unwrap_or(false)
        }
        None => false,
    };
    if authorized {
        next.run(req).await
    } else {
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "success": false, "error": "Authentication required" }))).into_response()
    }
}

fn session_cookie(token: &str) -> String {
    format!("{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000", auth::SESSION_COOKIE, token)
}
fn clear_cookie() -> String {
    format!("{}=; Path=/; HttpOnly; Max-Age=0", auth::SESSION_COOKIE)
}

// ---- auth handlers ----
async fn auth_status_api(State(state): State<RouterState>) -> Json<serde_json::Value> {
    let conn = state.app_state.conn.lock().unwrap();
    let has_users = auth::count_users(&conn).unwrap_or(0) > 0;
    let auth_disabled = std::env::var("BACKYARD_AUTH_DISABLED").map(|v| v == "1" || v.to_lowercase() == "true").unwrap_or(false);
    Json(serde_json::json!({ "configured": has_users, "has_users": has_users, "auth_disabled": auth_disabled }))
}

async fn auth_me_api(State(state): State<RouterState>, headers: HeaderMap) -> Response {
    let token = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()).and_then(|c| cookie_value(c, auth::SESSION_COOKIE));
    match token {
        Some(t) => {
            let conn = state.app_state.conn.lock().unwrap();
            match auth::validate_session(&conn, &t) {
                Ok(Some(user)) => Json(serde_json::json!({ "success": true, "user": user })).into_response(),
                _ => (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "success": false, "error": "Not logged in" }))).into_response(),
            }
        }
        None => (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "success": false, "error": "Not logged in" }))).into_response(),
    }
}

#[derive(Deserialize)]
struct Credentials { username: String, password: String }

async fn setup_api(State(state): State<RouterState>, Json(creds): Json<Credentials>) -> Response {
    let conn = state.app_state.conn.lock().unwrap();
    if auth::count_users(&conn).unwrap_or(0) > 0 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": "Admin account already configured" }))).into_response();
    }
    if creds.username.len() < 3 || creds.password.len() < 8 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": "Username (3+ chars) and password (8+ chars) required" }))).into_response();
    }
    match auth::create_user(&conn, &creds.username, &creds.password) {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "success": false, "error": e.to_string() }))).into_response(),
    }
}

async fn login_api(State(state): State<RouterState>, Json(creds): Json<Credentials>) -> Response {
    let conn = state.app_state.conn.lock().unwrap();
    match auth::authenticate(&conn, &creds.username, &creds.password) {
        Ok(Some(user)) => match auth::create_session(&conn, user.id) {
            Ok(token) => (StatusCode::OK, [(header::SET_COOKIE, session_cookie(&token)), (header::CONTENT_TYPE, "application/json".to_string())], serde_json::json!({ "success": true, "user": user }).to_string()).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "success": false, "error": e.to_string() }))).into_response(),
        },
        _ => (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "success": false, "error": "Invalid username or password" }))).into_response(),
    }
}

async fn logout_api(State(state): State<RouterState>, headers: HeaderMap) -> Response {
    let token = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()).and_then(|c| cookie_value(c, auth::SESSION_COOKIE));
    if let Some(t) = token { let conn = state.app_state.conn.lock().unwrap(); let _ = auth::delete_session(&conn, &t); }
    (StatusCode::OK, [(header::SET_COOKIE, clear_cookie())], Json(serde_json::json!({ "success": true }))).into_response()
}

// ---- endpoints ----
async fn list_endpoints(State(state): State<RouterState>) -> Json<serde_json::Value> {
    let endpoints = state.app_state.list_eval_endpoints().unwrap_or_default();
    let out: Vec<serde_json::Value> = endpoints.iter().map(|e| serde_json::json!({
        "id": e.id, "name": e.name, "base_url": e.base_url, "has_api_key": e.api_key.is_some(), "model": e.model, "provider": e.provider, "created_at": e.created_at
    })).collect();
    Json(serde_json::json!({ "success": true, "endpoints": out }))
}

#[derive(Deserialize)]
struct EndpointRequest { name: String, base_url: String, api_key: Option<String>, model: Option<String>, provider: Option<String> }

async fn add_endpoint_api(State(state): State<RouterState>, Json(req): Json<EndpointRequest>) -> Response {
    if req.name.is_empty() || req.base_url.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": "name and base_url required" }))).into_response();
    }
    match state.app_state.add_eval_endpoint_with_provider(&req.name, &req.base_url, req.api_key.as_deref(), req.model.as_deref(), req.provider.as_deref()) {
        Ok(id) => Json(serde_json::json!({ "success": true, "id": id })).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": e.to_string() }))).into_response(),
    }
}

async fn delete_endpoint_api(State(state): State<RouterState>, Path(id): Path<i64>) -> Response {
    match state.app_state.delete_eval_endpoint(id) {
        Ok(_) => Json(serde_json::json!({ "success": true })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "success": false, "error": e.to_string() }))).into_response(),
    }
}

async fn probe_endpoint_api(State(state): State<RouterState>, Path(id): Path<i64>) -> Response {
    let ep = match state.app_state.get_eval_endpoint(id) {
        Ok(e) => e, Err(e) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({ "success": false, "error": e.to_string() }))).into_response(),
    };
    match crate::bench_http::probe_endpoint(&ep).await {
        Ok(out) => Json(serde_json::json!({ "success": true, "output": out })).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({ "success": false, "error": e.to_string() }))).into_response(),
    }
}

// ---- benchmarks ----
#[derive(Deserialize)]
struct CreateBenchmarkRequest {
    endpoint_id: i64,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    iterations: Option<usize>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    stream: Option<bool>,
}

async fn create_benchmark_api(State(state): State<RouterState>, Json(req): Json<CreateBenchmarkRequest>) -> Response {
    let endpoint = match state.app_state.get_eval_endpoint(req.endpoint_id) {
        Ok(e) => e, Err(_) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({ "success": false, "error": "Endpoint not found" }))).into_response(),
    };
    let local_id = state.app_state.local_machine_id().unwrap_or(1);
    let model_label = endpoint.model.clone().unwrap_or_else(|| endpoint.name.clone());
    let cfg = serde_json::json!({
        "prompt": req.prompt.unwrap_or_else(|| "Write a short story about a robot learning to paint.".into()),
        "iterations": req.iterations.unwrap_or(5),
        "max_tokens": req.max_tokens.unwrap_or(128),
        "temperature": req.temperature.unwrap_or(0.7),
        "stream": req.stream.unwrap_or(false),
    });
    let conn = state.app_state.conn.lock().unwrap();
    let res = conn.execute(
        "INSERT INTO runs (machine_id, endpoint_id, kind, task, model_id, model_label, model_path, config) VALUES (?, ?, 'bench', 'http_bench', 0, ?, '', ?)",
        rusqlite::params![local_id, endpoint.id, model_label, cfg.to_string()],
    );
    match res {
        Ok(_) => {
            let id = conn.last_insert_rowid();
            drop(conn);
            match state.app_state.get_run(id) {
                Ok(run) => Json(serde_json::json!({ "success": true, "run": run })).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "success": false, "error": e.to_string() }))).into_response(),
            }
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "success": false, "error": e.to_string() }))).into_response(),
    }
}

// Legacy bench via machine/model — returns 410
async fn create_run_legacy_api() -> Response {
    (StatusCode::GONE, Json(serde_json::json!({ "success": false, "error": "Machine-based benchmarks removed. Use POST /api/benchmarks with endpoint_id." }))).into_response()
}

// ---- eval runs ----
#[derive(Deserialize)]
struct CreateEvalRunRequest { endpoint_id: i64, harness: String, #[serde(default)] config: serde_json::Value }

async fn create_eval_run_api(State(state): State<RouterState>, Json(req): Json<CreateEvalRunRequest>) -> Response {
    let endpoint = match state.app_state.get_eval_endpoint(req.endpoint_id) {
        Ok(e) => e, Err(_) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({ "success": false, "error": "Endpoint not found" }))).into_response(),
    };
    let known = ["micro_swe","tbench","deepswe","opencode","tau2_telecom","tau2_retail","tau2_airline","tau2_mock","livecodebench","swebench","toolathlon","frontierswe","osworld","perf_takehome"];
    if !known.contains(&req.harness.as_str()) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": "Unknown harness" }))).into_response();
    }
    let local_id = state.app_state.local_machine_id().unwrap_or(1);
    let model_label = endpoint.model.clone().unwrap_or_else(|| endpoint.name.clone());
    let conn = state.app_state.conn.lock().unwrap();
    match conn.execute(
        "INSERT INTO runs (machine_id, endpoint_id, kind, task, model_id, model_label, model_path, config) VALUES (?, ?, 'eval', ?, 0, ?, '', ?)",
        rusqlite::params![local_id, endpoint.id, req.harness, model_label, req.config.to_string()],
    ) {
        Ok(_) => {
            let id = conn.last_insert_rowid();
            drop(conn);
            match state.app_state.get_run(id) {
                Ok(run) => Json(serde_json::json!({ "success": true, "run": run })).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "success": false, "error": e.to_string() }))).into_response(),
            }
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "success": false, "error": e.to_string() }))).into_response(),
    }
}

// ---- runs / results / leaderboard ----
#[derive(Deserialize)]
struct RunQuery { limit: Option<u32> }

async fn list_runs(State(state): State<RouterState>, Query(q): Query<RunQuery>) -> Json<serde_json::Value> {
    let runs = state.app_state.list_runs(q.limit.or(Some(50))).unwrap_or_default();
    let mut out = Vec::new();
    for run in runs {
        let results = state.app_state.list_results(run.id).unwrap_or_default();
        let mut metrics = HashMap::new();
        for r in &results { metrics.entry(r.metric.clone()).or_insert(r.value); }
        out.push(serde_json::json!({
            "id": run.id, "machine_id": run.machine_id, "endpoint_id": run.endpoint_id,
            "kind": run.kind, "task": run.task, "model_id": run.model_id, "model_label": run.model_label,
            "status": run.status, "error": run.error, "created_at": run.created_at,
            "started_at": run.started_at, "finished_at": run.finished_at, "metrics": metrics,
        }));
    }
    Json(serde_json::json!({ "success": true, "runs": out }))
}

async fn get_run_api(State(state): State<RouterState>, Path(id): Path<i64>) -> Response {
    match state.app_state.get_run(id) {
        Ok(run) => Json(serde_json::json!({ "success": true, "run": run })).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "success": false, "error": "Run not found" }))).into_response(),
    }
}

async fn get_run_results_api(State(state): State<RouterState>, Path(id): Path<i64>) -> Response {
    match state.app_state.list_results(id) {
        Ok(results) => Json(serde_json::json!({ "success": true, "results": results })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "success": false, "error": e.to_string() }))).into_response(),
    }
}

const LEADERBOARD_METRICS: &[&str] = &["tps","tg_tps","latency_ms","ttft_ms","score","pass_at_1","rouge_l_f1","success_rate","mswe_pass_rate","tau2_avg_reward","lcb_pass_at_1","swebench_resolved","toolathlon_score","frontierswe_score","deepswe_reward","deepswe_partial","opencode_reward","opencode_partial","tbench_reward","perf_speedup","perf_cycles","perf_thresholds"];

async fn leaderboard(State(state): State<RouterState>) -> Json<serde_json::Value> {
    let conn = state.app_state.conn.lock().unwrap();
    let mut out = HashMap::new();
    for metric in LEADERBOARD_METRICS {
        let sql = "SELECT r.model_label, r.id AS run_id, r.created_at, COALESCE(ep.name, mac.name) AS src, COALESCE(ep.base_url, mac.host) AS host, res.value, res.extra FROM results res JOIN runs r ON r.id = res.run_id LEFT JOIN eval_endpoints ep ON ep.id = r.endpoint_id LEFT JOIN machines mac ON mac.id = r.machine_id WHERE res.metric = ?1 AND r.status = 'done' AND r.id = (SELECT MAX(r2.id) FROM runs r2 JOIN results res2 ON res2.run_id = r2.id WHERE res2.metric = ?1 AND r2.status = 'done' AND r2.model_label = r.model_label) ORDER BY res.value DESC";
        let mut stmt = match conn.prepare(sql) { Ok(s) => s, Err(_) => continue };
        let rows = stmt.query_map([metric], |row| Ok(serde_json::json!({
            "model": row.get::<_, String>(0)?, "run_id": row.get::<_, i64>(1)?, "run_created_at": row.get::<_, String>(2)?,
            "machine": row.get::<_, String>(3)?, "host": row.get::<_, String>(4)?, "value": row.get::<_, f64>(5)?, "extra": row.get::<_, Option<String>>(6)?,
        }))).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap_or_default();
        if !rows.is_empty() { out.insert(metric.to_string(), serde_json::json!(rows)); }
    }
    drop(conn);
    Json(serde_json::json!({ "success": true, "metrics": out }))
}

// ---- legacy compat (kept as bare arrays: the UI iterates the response) ----
async fn list_machines(State(state): State<RouterState>) -> Json<Vec<crate::state::Machine>> {
    Json(state.app_state.list_machines().unwrap_or_default())
}

async fn reset_db_api(State(state): State<RouterState>) -> impl IntoResponse {
    let conn = state.app_state.conn.lock().unwrap();
    conn.execute("DELETE FROM results", []).unwrap();
    conn.execute("DELETE FROM runs", []).unwrap();
    Json(serde_json::json!({ "success": true }))
}
