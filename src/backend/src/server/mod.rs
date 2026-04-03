use axum::{
    routing::{get, post},
    Router, Json, extract::{State, Path},
    response::IntoResponse,
};
use tower_http::services::ServeDir;
use std::sync::Arc;
use anyhow::Result;
use crate::state::{State as AppState, Library, Model};

pub struct ServerState {
    pub app_state: Arc<AppState>,
}

pub async fn start_server(app_state: Arc<AppState>) -> Result<()> {
    let server_state = Arc::new(ServerState { app_state });

    let app = Router::new()
        .route("/api/libraries", get(list_libraries).post(add_library))
        .route("/api/libraries/scan", post(scan_libraries_api))
        .route("/api/system/reset", post(reset_db_api))
        .route("/api/models", get(list_models))
        .route("/api/models/download", post(download_model_api))
        .route("/api/models/{id}/move", post(move_model_api))
        .route("/api/engines/start", post(start_engine_api))
        .route("/api/engines/build", post(build_engine_api))
        .route("/api/benchmarks", post(run_benchmark_api).get(list_benchmarks_stub))
        .route("/api/engines", get(list_engines_api))
        .route("/", get(serve_index))
        .route("/models", get(serve_index))
        .route("/servers", get(serve_index))
        .route("/benchmarks", get(serve_index))
        .route("/settings", get(serve_index))
        .nest_service("/assets", ServeDir::new("src/frontend/dist/assets"))
        .nest_service("/css", ServeDir::new("src/frontend/dist/css"))
        .nest_service("/js", ServeDir::new("src/frontend/dist/js"))
        .nest_service("/static", ServeDir::new("src/frontend/static"))
        .fallback_service(ServeDir::new("src/frontend/dist/templates"))
        .with_state(server_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:5556").await?;
    println!("Server listening on http://0.0.0.0:5556");
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn serve_index() -> impl IntoResponse {
    let content = tokio::fs::read_to_string("src/frontend/dist/templates/index.html").await.unwrap();
    axum::response::Html(content)
}

async fn list_libraries(State(state): State<Arc<ServerState>>) -> Json<Vec<Library>> {
    let conn = state.app_state.conn.lock().unwrap();
    let mut stmt = conn.prepare("SELECT id, name, path FROM libraries").unwrap();
    let libs = stmt.query_map([], |row| {
        Ok(Library { id: row.get(0)?, name: row.get(1)?, path: row.get(2)? })
    }).unwrap().map(|r| r.unwrap()).collect();
    Json(libs)
}

async fn add_library(State(state): State<Arc<ServerState>>, Json(lib): Json<Library>) -> Json<Library> {
    let conn = state.app_state.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO libraries (name, path) VALUES (?, ?)",
        [&lib.name, &lib.path],
    ).unwrap();
    Json(lib)
}

async fn scan_libraries_api(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    let libs = {
        let conn = state.app_state.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, path FROM libraries").unwrap();
        stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))
            .unwrap().map(|r| r.unwrap()).collect::<Vec<_>>()
    };

    let mut found = 0;
    for (lib_id, lib_path) in libs {
        let mut walker = walkdir::WalkDir::new(&lib_path).into_iter();
        while let Some(Ok(entry)) = walker.next() {
            if entry.file_type().is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".gguf") {
                        let full_path = entry.path();
                        let relative_path = full_path.strip_prefix(&lib_path).unwrap().to_string_lossy().into_owned();
                        
                        let family = if let Some(parent) = entry.path().parent() {
                            parent.file_name().and_then(|n| n.to_str()).map(|n| n.to_string())
                        } else {
                            None
                        };
                        
                        let quant = if let Some(idx) = name.rfind('-') {
                            let q = &name[idx+1..name.len()-5];
                            if q.starts_with('Q') || q.starts_with("IQ") {
                                Some(q.to_string())
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let conn = state.app_state.conn.lock().unwrap();
                        conn.execute(
                            "INSERT OR IGNORE INTO models (name, library_id, path, format, family, quant) VALUES (?, ?, ?, ?, ?, ?)",
                            (&relative_path, lib_id, &relative_path, "GGUF", &family, &quant),
                        ).unwrap();
                        found += 1;
                    }
                }
            }
        }
    }
    format!("Scanned libraries, found {} new GGUF models", found)
}

async fn reset_db_api(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    let conn = state.app_state.conn.lock().unwrap();
    conn.execute("DELETE FROM models", []).unwrap();
    conn.execute("DELETE FROM libraries", []).unwrap();
    conn.execute("DELETE FROM engines", []).unwrap();
    
    let home_dir = std::env::var("HOME").unwrap();
    let default_path = std::path::PathBuf::from(home_dir).join(".backyard").join("models");
    let path_str = default_path.to_str().unwrap();
    conn.execute(
        "INSERT OR IGNORE INTO libraries (name, path) VALUES (?, ?)",
        ["Default", path_str],
    ).unwrap();

    "Database reset successfully"
}

async fn list_models(State(state): State<Arc<ServerState>>) -> Json<Vec<Model>> {
    Json(state.app_state.list_models().unwrap())
}

#[derive(serde::Deserialize)]
struct DownloadRequest {
    repo_id: String,
    filename: String,
}

async fn download_model_api(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<DownloadRequest>,
) -> impl IntoResponse {
    let url = format!("https://huggingface.co/{}/resolve/main/{}?download=true", req.repo_id, req.filename);
    let default_path = {
        let conn = state.app_state.conn.lock().unwrap();
        conn.query_row("SELECT path FROM libraries WHERE name = 'Default'", [], |row| row.get::<_, String>(0)).unwrap()
    };
    let dest_path = std::path::PathBuf::from(default_path).join(&req.filename);
    let state_clone = Arc::clone(&state);
    let filename_clone = req.filename.clone();
    
    tokio::spawn(async move {
        let response = reqwest::get(url).await.unwrap();
        let mut file = tokio::fs::File::create(dest_path).await.unwrap();
        let mut content = std::io::Cursor::new(response.bytes().await.unwrap());
        tokio::io::copy(&mut content, &mut file).await.unwrap();

        let conn = state_clone.app_state.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO models (name, library_id, path, format) VALUES (?, ?, ?, ?)",
            (&filename_clone, 1, &filename_clone, "GGUF"),
        ).unwrap();
    });

    "Download started"
}

async fn move_model_api(
    State(state): State<Arc<ServerState>>,
    Path(model_id): Path<i64>,
    Json(target_lib): Json<i64>,
) -> impl IntoResponse {
    crate::state::fs::move_model(&state.app_state, model_id, target_lib).await.unwrap();
    "Model moved"
}

#[derive(serde::Deserialize)]
struct StartEngineRequest {
    model_id: i64,
    gpu_type: String,
}

async fn start_engine_api(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<StartEngineRequest>,
) -> impl IntoResponse {
    let orchestrator = crate::orchestrator::Orchestrator::new().unwrap();
    let container_id = orchestrator.start_engine(req.model_id, &state.app_state, &req.gpu_type).await.unwrap();
    Json(container_id)
}

#[derive(serde::Deserialize)]
struct BuildEngineRequest {
    gpu_type: String,
}

async fn build_engine_api(
    State(_state): State<Arc<ServerState>>,
    Json(req): Json<BuildEngineRequest>,
) -> impl IntoResponse {
    let dockerfile = match req.gpu_type.as_str() {
        "nvidia" => "containers/llama.cpp/Dockerfile.cuda",
        "amd" => "containers/llama.cpp/Dockerfile.rocm",
        _ => "containers/llama.cpp/Dockerfile.cuda",
    };
    let tag = match req.gpu_type.as_str() {
        "nvidia" => "backyard:llama-cuda",
        "amd" => "backyard:llama-rocm",
        _ => "backyard:llama-cuda",
    };
    let status = std::process::Command::new("docker")
        .args(["build", "-t", tag, "-f", dockerfile, "."])
        .status()
        .unwrap();
    if status.success() { "Build successful" } else { "Build failed" }
}

async fn list_benchmarks_stub() -> impl IntoResponse {
    "[]"
}

#[derive(serde::Deserialize)]
struct BenchmarkRequest {
    model_id: i64,
    gpu_type: String,
}

async fn run_benchmark_api(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<BenchmarkRequest>,
) -> impl IntoResponse {
    let (model_path, library_path): (String, String) = {
        let conn = state.app_state.conn.lock().unwrap();
        conn.query_row(
            "SELECT m.path, l.path FROM models m JOIN libraries l ON m.library_id = l.id WHERE m.id = ?",
            [req.model_id],
            |row| Ok((row.get(0)?, row.get(1)?))
        ).unwrap()
    };
    let image = match req.gpu_type.as_str() {
        "nvidia" => "backyard:llama-cuda",
        "amd" => "backyard:llama-rocm",
        _ => "backyard:llama-cuda",
    };
    let vol_mount = format!("{}:/models:ro", library_path);
    let model_arg = format!("/models/{}", model_path);
    let output = std::process::Command::new("docker")
        .args([
            "run", "--rm", "--gpus", "all", "--entrypoint", "llama-bench",
            "-v", &vol_mount, image, "-m", &model_arg, "-p", "512", "-n", "128"
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if output.status.success() { stdout } else { format!("Benchmark failed: {}", stderr) }
}

#[derive(serde::Serialize)]
struct EngineInfo {
    name: String,
    tag: String,
    image_id: String,
    created_at: String,
    status: String,
}

async fn list_engines_api() -> Json<Vec<EngineInfo>> {
    let output = std::process::Command::new("docker")
        .args([
            "images", 
            "--format", "{{.Repository}}\t{{.Tag}}\t{{.ID}}\t{{.CreatedAt}}",
            "backyard*"
        ])
        .output()
        .unwrap();
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut engines = Vec::new();
    
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 4 {
            let repo = parts[0];
            let tag = parts[1];
            let id = parts[2];
            let created = parts[3];
            
            // Check if any container is running with this image
            let ps_output = std::process::Command::new("docker")
                .args(["ps", "--filter", &format!("ancestor={}:{}", repo, tag), "--format", "{{.Status}}"])
                .output()
                .unwrap();
            let ps_stdout = String::from_utf8_lossy(&ps_output.stdout);
            let status = if ps_stdout.is_empty() { "stopped".to_string() } else { ps_stdout.trim().to_string() };

            engines.push(EngineInfo {
                name: repo.to_string(),
                tag: tag.to_string(),
                image_id: id.to_string(),
                created_at: created.to_string(),
                status,
            });
        }
    }
    Json(engines)
}
