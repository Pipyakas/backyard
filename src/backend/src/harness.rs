use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::state::{EvalEndpoint, Run, RunResult};

/// Directory containing the eval harnesses (tau2-bench, micro_swe_tool_eval.py, ...).
/// Override with BACKYARD_EVALS_DIR.
pub fn evals_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("BACKYARD_EVALS_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let default = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../evals");
    if default.exists() {
        Ok(default)
    } else {
        bail!("Evals directory not found (set BACKYARD_EVALS_DIR)")
    }
}

pub fn run_harness(run: &Run, endpoint: &EvalEndpoint) -> Result<(String, Vec<RunResult>)> {
    match run.task.as_str() {
        "micro_swe" => run_micro_swe(run, endpoint),
        "tbench" => run_tbench(run, endpoint),
        "deepswe" => run_deepswe(run, endpoint),
        "tau2_telecom" => run_tau2(run, endpoint, "telecom"),
        "tau2_retail" => run_tau2(run, endpoint, "retail"),
        "tau2_airline" => run_tau2(run, endpoint, "airline"),
        "tau2_mock" => run_tau2(run, endpoint, "mock"),
        "livecodebench" => run_livecodebench(run, endpoint),
        "perf_takehome" => run_perf_takehome(run, endpoint),
        "swebench" => run_swebench(run, endpoint),
        "toolathlon" => run_toolathlon(run, endpoint),
        "frontierswe" => run_frontier_swe(run, endpoint),
        "osworld" => run_osworld(run, endpoint),
        other => bail!("Unknown eval harness: {}", other),
    }
}

fn model_name(endpoint: &EvalEndpoint) -> String {
    endpoint.model.clone().unwrap_or_else(|| "default".to_string())
}

fn endpoint_base_root(endpoint: &EvalEndpoint) -> String {
    let b = endpoint.base_url.trim_end_matches('/');
    b.strip_suffix("/v1").unwrap_or(b).to_string()
}

/// Endpoint auth for python drivers: OPENAI_API_KEY carries the key, CCGW_PROVIDER
/// tells drivers to send `x-ccgw-key` instead of Bearer (ccgw gateway auth).
fn endpoint_driver_env(endpoint: &EvalEndpoint) -> Vec<(&'static str, String)> {
    vec![
        ("OPENAI_API_KEY", endpoint.api_key.clone().unwrap_or_else(|| "none".to_string())),
        ("CCGW_PROVIDER", endpoint.provider.clone().unwrap_or_else(|| "generic".to_string())),
    ]
}

fn run_capture(program: &str, args: &[&str], cwd: Option<&std::path::Path>) -> Result<String> {
    run_capture_env(program, args, cwd, &[])
}

fn run_capture_env(program: &str, args: &[&str], cwd: Option<&std::path::Path>, extra_env: &[(&str, String)]) -> Result<String> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1");
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let output = cmd.output().context("failed to run harness")?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    // Harnesses may exit nonzero for valid-but-failing results (e.g. micro_swe
    // returns 1 when a case fails verification). Keep the output; callers
    // decide whether it parses into results.
    Ok(format!("{}{}", stdout, stderr))
}

fn find_harbor() -> String {
    for p in [
        "/home/quynn/.local/bin/harbor",
        "/root/.local/bin/harbor",
        "/usr/local/bin/harbor",
    ] {
        if std::path::Path::new(p).exists() {
            return p.to_string();
        }
    }
    "harbor".to_string()
}

fn find_pier() -> String {
    for p in [
        "/home/quynn/.local/bin/pier",
        "/root/.local/bin/pier",
        "/usr/local/bin/pier",
    ] {
        if std::path::Path::new(p).exists() {
            return p.to_string();
        }
    }
    "pier".to_string()
}

fn endpoint_sandbox_url(endpoint: &EvalEndpoint) -> String {
    let root = endpoint_base_root(endpoint);
    if root.contains("127.0.0.1") || root.contains("localhost") {
        let port = root
            .rsplit(':')
            .next()
            .and_then(|p| p.trim_end_matches('/').parse::<u32>().ok())
            .unwrap_or(8095);
        format!("http://172.17.0.1:{}/v1", port)
    } else {
        format!("{}/v1", root)
    }
}

fn write_endpoint_env_file(dir: &std::path::Path, run_id: i64, prefix: &str, endpoint: &EvalEndpoint) -> Result<std::path::PathBuf> {
    let env_file = dir.join(format!("{}-run-{}.env", prefix, run_id));
    std::fs::write(
        &env_file,
        format!(
            "OPENAI_API_KEY={}\nOPENAI_BASE_URL={}\n",
            endpoint.api_key.clone().unwrap_or_else(|| "none".to_string()),
            endpoint_sandbox_url(endpoint)
        ),
    )?;
    Ok(env_file)
}

fn newest_job_dir(jobs_dir: &std::path::Path) -> Option<PathBuf> {
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    if let Ok(entries) = std::fs::read_dir(jobs_dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                if let Ok(meta) = p.metadata() {
                    let t = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                    if newest.as_ref().map(|(nt, _)| t > *nt).unwrap_or(true) {
                        newest = Some((t, p));
                    }
                }
            }
        }
    }
    newest.map(|(_, p)| p)
}

fn run_deepswe(run: &Run, endpoint: &EvalEndpoint) -> Result<(String, Vec<RunResult>)> {
    let dir = evals_dir()?;
    let jobs_dir = dir.join("jobs");
    std::fs::create_dir_all(&jobs_dir)?;
    let tasks_dir = dir.join("deep-swe/tasks");
    if !tasks_dir.exists() {
        bail!("deep-swe tasks not found in {}", tasks_dir.display());
    }

    let config: serde_json::Value = serde_json::from_str(&run.config).unwrap_or_default();
    let task = config
        .get("task")
        .and_then(|v| v.as_str())
        .unwrap_or("abs-stepped-slices")
        .to_string();
    let task_path = if task.contains('/') {
        task.clone()
    } else {
        format!("tasks/{}", task)
    };

    let env_file = write_endpoint_env_file(&dir, run.id, "ds", endpoint)?;
    let args = [
        "run",
        "-p",
        &task_path,
        "-a",
        "mini-swe-agent",
        "-m",
        &format!("openai/{}", model_name(endpoint)),
        "--env-file",
        env_file.to_str().unwrap(),
        "-n",
        "1",
        "--jobs-dir",
        jobs_dir.to_str().unwrap(),
    ];
    let out = run_capture(&find_pier(), &args, Some(&tasks_dir))?;
    let _ = std::fs::remove_file(&env_file);

    let Some(dirp) = newest_job_dir(&jobs_dir) else {
        bail!("no pier job output found in {}", jobs_dir.display());
    };
    let results_path = dirp.join("result.json");
    let text = std::fs::read_to_string(&results_path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {}", results_path.display(), e))?;
    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    let mut results = Vec::new();
    if let Some(evals) = parsed.get("stats").and_then(|s| s.get("evals")).and_then(|e| e.as_object()) {
        for (_name, eval) in evals {
            if let Some(metrics) = eval.get("metrics").and_then(|m| m.as_array()) {
                for m in metrics {
                    for key in ["partial", "reward", "f2p", "p2p"] {
                        if let Some(v) = m.get(key).and_then(|v| v.as_f64()) {
                            results.push(RunResult {
                                id: 0,
                                run_id: 0,
                                metric: format!("deepswe_{}", key),
                                value: v,
                                unit: Some("score".into()),
                                extra: Some(format!("{{\"task\":\"{}\"}}", task)),
                            });
                        }
                    }
                }
            }
        }
    }
    if results.is_empty() {
        bail!("no deepswe metrics in {}", results_path.display());
    }
    Ok((out, results))
}

fn run_tbench(run: &Run, endpoint: &EvalEndpoint) -> Result<(String, Vec<RunResult>)> {
    let dir = evals_dir()?;
    let jobs_dir = dir.join("jobs");
    std::fs::create_dir_all(&jobs_dir)?;

    let config: serde_json::Value = serde_json::from_str(&run.config).unwrap_or_default();
    let task = config
        .get("task")
        .and_then(|v| v.as_str())
        .unwrap_or("terminal-bench/break-filter-js-from-html")
        .to_string();

    // The endpoint must be reachable from inside the docker sandbox: rewrite
    // loopback hosts to the docker0 gateway.
    let env_file = write_endpoint_env_file(&dir, run.id, "tb", endpoint)?;

    let args = [
        "run",
        "-d",
        "terminal-bench/terminal-bench-2",
        "-a",
        "pi",
        "-m",
        &format!("openai/{}", model_name(endpoint)),
        "--env-file",
        env_file.to_str().unwrap(),
        "-n",
        "1",
        "-k",
        "1",
        "-y",
        "--jobs-dir",
        jobs_dir.to_str().unwrap(),
        "-i",
        &task,
    ];
    let out = run_capture(&find_harbor(), &args, Some(&dir))?;
    let _ = std::fs::remove_file(&env_file);

    // Parse the newest job result.json.
    let newest = newest_job_dir(&jobs_dir);
    let Some(dirp) = newest else {
        bail!("no harbor job output found in {}", jobs_dir.display());
    };
    let results_path = dirp.join("result.json");
    let text = std::fs::read_to_string(&results_path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {}", results_path.display(), e))?;
    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    let mut results = Vec::new();
    if let Some(evals) = parsed.get("stats").and_then(|s| s.get("evals")).and_then(|e| e.as_object()) {
        for (_name, eval) in evals {
            if let Some(metrics) = eval.get("metrics").and_then(|m| m.as_array()) {
                for m in metrics {
                    if let Some(mean) = m.get("mean").and_then(|v| v.as_f64()) {
                        results.push(RunResult {
                            id: 0,
                            run_id: 0,
                            metric: "tbench_reward".into(),
                            value: mean,
                            unit: Some("reward".into()),
                            extra: Some(format!("{{\"task\":\"{}\"}}", task)),
                        });
                    }
                }
            }
        }
    }
    if results.is_empty() {
        bail!("no tbench metrics in {}", results_path.display());
    }
    Ok((out, results))
}


fn parse_micro_swe(stdout: &str) -> Result<Vec<RunResult>> {
    // The full results JSON comes first (from --output); stdout may contain
    // extra harness chatter, so only parse stdout when the file path parse
    // hasn't happened. Prefer direct parse of the whole input.
    let parsed: serde_json::Value = serde_json::from_str(stdout)
        .or_else(|_| {
            // fallback: find the results object inside noisy stdout
            stdout
                .find("\"cases\"")
                .and_then(|idx| {
                    let start = idx.saturating_sub(64);
                    let tail = &stdout[start..];
                    let obj_start = tail.rfind('{')?;
                    serde_json::from_str(&tail[obj_start..]).ok()
                })
                .ok_or_else(|| anyhow::anyhow!("no results JSON in harness output"))
        })
        .map_err(|e| anyhow::anyhow!("bad harness JSON: {}", e))?;
    let cases = parsed
        .get("cases")
        .and_then(|c| c.as_array())
        .ok_or_else(|| anyhow::anyhow!("no cases array in harness output"))?;

    let mut results = Vec::new();
    let mut passed = 0usize;
    let mut total = 0usize;
    for case in cases {
        let name = case.get("case").and_then(|v| v.as_str()).unwrap_or("?");
        let ok = case.get("passed").and_then(|v| v.as_bool()).unwrap_or(false);
        results.push(RunResult {
            id: 0,
            run_id: 0,
            metric: format!("mswe@{}", name),
            value: if ok { 1.0 } else { 0.0 },
            unit: Some("pass".into()),
            extra: None,
        });
        passed += usize::from(ok);
        total += 1;
    }
    if total == 0 {
        bail!("no micro_swe cases in output:\n{}", stdout.lines().take(20).collect::<Vec<_>>().join("\n"));
    }
    results.insert(
        0,
        RunResult {
            id: 0,
            run_id: 0,
            metric: "mswe_pass_rate".into(),
            value: passed as f64 / total as f64,
            unit: Some("pass".into()),
            extra: Some(format!("{{\"total\":{},\"passed\":{}}}", total, passed)),
        },
    );
    Ok(results)
}

fn run_micro_swe(run: &Run, endpoint: &EvalEndpoint) -> Result<(String, Vec<RunResult>)> {
    let dir = evals_dir()?;
    let script = dir.join("micro_swe_tool_eval.py");
    if !script.exists() {
        bail!("micro_swe_tool_eval.py not found in {}", dir.display());
    }
    let config: serde_json::Value = serde_json::from_str(&run.config).unwrap_or_default();
    let max_rounds = config.get("max_rounds").and_then(|v| v.as_i64()).unwrap_or(10).to_string();
    let max_tokens = config.get("max_tokens").and_then(|v| v.as_i64()).unwrap_or(1024).to_string();
    let out_path = dir.join(format!("results/run-{}.json", run.id));
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let out = run_capture_env(
        "python3",
        &[
            script.to_str().unwrap(),
            "--base-url",
            &format!("{}/v1", endpoint_base_root(endpoint)),
            "--model",
            &model_name(endpoint),
            "--max-rounds",
            &max_rounds,
            "--max-tokens",
            &max_tokens,
            "--no-thinking",
            "--api-key",
            &endpoint.api_key.clone().unwrap_or_default(),
            "--output",
            out_path.to_str().unwrap(),
        ],
        Some(&dir),
        &endpoint_driver_env(endpoint),
    )?;
    // The harness writes full results to --output; prefer that over stdout.
    let results_text = std::fs::read_to_string(&out_path).unwrap_or_default();
    let source = if !results_text.trim().is_empty() {
        results_text
    } else {
        out.clone()
    };
    let results = parse_micro_swe(&source)?;
    Ok((out, results))
}

fn parse_tau2(stdout: &str) -> Result<(String, Vec<RunResult>)> {
    // The tau2 CLI writes per-task simulations into data/simulations/<dir>/results.json.
    // The final line of stdout usually names the save dir; fall back to scanning
    // the simulations directory for the newest results.json written by this run.
    let mut results = Vec::new();
    let mut completed = 0usize;
    for line in stdout.lines() {
        let Some(payload) = line.strip_prefix("REWARD ") else { continue };
        let parts: Vec<&str> = payload.splitn(2, ' ').collect();
        if parts.len() == 2 {
            if let Ok(v) = parts[1].parse::<f64>() {
                results.push(RunResult {
                    id: 0,
                    run_id: 0,
                    metric: format!("tau2@{}", parts[0]),
                    value: v,
                    unit: Some("reward".into()),
                    extra: None,
                });
                completed += 1;
            }
        }
    }
    if completed == 0 {
        bail!("no tau2 rewards parsed from output:\n{}", stdout.lines().take(25).collect::<Vec<_>>().join("\n"));
    }
    let avg = results.iter().map(|r| r.value).sum::<f64>() / completed as f64;
    results.insert(
        0,
        RunResult {
            id: 0,
            run_id: 0,
            metric: "tau2_avg_reward".into(),
            value: avg,
            unit: Some("reward".into()),
            extra: Some(format!("{{\"tasks\":{}}}", completed)),
        },
    );
    Ok((stdout.to_string(), results))
}

fn run_tau2(run: &Run, endpoint: &EvalEndpoint, domain: &str) -> Result<(String, Vec<RunResult>)> {
    let dir = evals_dir()?.join("tau2-bench");
    if !dir.exists() {
        bail!("tau2-bench not found in {}", dir.display());
    }
    // Prefer the venv tau2 binary created by `uv sync`; fall back to `uv run tau2`.
    let tau2_exe = dir.join(".venv/Scripts/tau2.exe");
    let tau2_exe_unix = dir.join(".venv/bin/tau2");
    let (program, base_args): (String, Vec<String>) = if tau2_exe.exists() {
        (tau2_exe.to_string_lossy().into_owned(), Vec::new())
    } else if tau2_exe_unix.exists() {
        (tau2_exe_unix.to_string_lossy().into_owned(), Vec::new())
    } else {
        ("uv".to_string(), vec!["run".to_string(), "tau2".to_string()])
    };

    let config: serde_json::Value = serde_json::from_str(&run.config).unwrap_or_default();
    let num_tasks = config.get("num_tasks").and_then(|v| v.as_i64()).unwrap_or(5).to_string();
    let num_trials = config.get("num_trials").and_then(|v| v.as_i64()).unwrap_or(1).to_string();
    let llm_args = serde_json::json!({
        "api_base": format!("{}/v1", endpoint_base_root(endpoint)),
        "api_key": endpoint.api_key.clone().unwrap_or_else(|| "none".to_string()),
    })
    .to_string();

    let mut args: Vec<String> = base_args;
    args.extend([
        "run".to_string(),
        "--domain".to_string(),
        domain.to_string(),
        "--agent-llm".to_string(),
        format!("openai/{}", model_name(endpoint)),
        "--user-llm".to_string(),
        format!("openai/{}", model_name(endpoint)),
        "--agent-llm-args".to_string(),
        llm_args.clone(),
        "--user-llm-args".to_string(),
        llm_args,
        "--num-tasks".to_string(),
        num_tasks,
        "--num-trials".to_string(),
        num_trials,
    ]);
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let out = run_capture(&program, &arg_refs, Some(&dir))?;

    // Collect per-task rewards from the newest tau2 results file written by this run.
    let sims = dir.join("data/simulations");
    if sims.exists() {
        let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
        if let Ok(entries) = std::fs::read_dir(&sims) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    if let Ok(meta) = p.metadata() {
                        let t = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                        if newest.as_ref().map(|(nt, _)| t > *nt).unwrap_or(true) {
                            newest = Some((t, p));
                        }
                    }
                }
            }
        }
        if let Some((_, dirp)) = newest {
            let results_path = dirp.join("results.json");
            if let Ok(text) = std::fs::read_to_string(&results_path) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(sims) = parsed.get("simulations").and_then(|v| v.as_array()) {
                        let mut collected: Vec<RunResult> = Vec::new();
                        for s in sims {
                            let task = s.get("task_id").and_then(|v| v.as_str()).unwrap_or("?");
                            let reward = s.get("reward").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let reason = s.get("termination_reason").and_then(|v| v.as_str()).unwrap_or("");
                            if reason == "success" || reason == "max_steps_reached" {
                                collected.push(RunResult {
                                    id: 0,
                                    run_id: 0,
                                    metric: format!("tau2@{}", task),
                                    value: reward,
                                    unit: Some("reward".into()),
                                    extra: None,
                                });
                            }
                        }
                        if !collected.is_empty() {
                            let avg = collected.iter().map(|r| r.value).sum::<f64>() / collected.len() as f64;
                            let mut all = vec![RunResult {
                                id: 0,
                                run_id: 0,
                                metric: "tau2_avg_reward".into(),
                                value: avg,
                                unit: Some("reward".into()),
                                extra: Some(format!("{{\"tasks\":{}}}", collected.len())),
                            }];
                            all.extend(collected);
                            return Ok((out, all));
                        }
                    }
                }
            }
        }
    }
    parse_tau2(&out)
}

fn run_livecodebench(run: &Run, endpoint: &EvalEndpoint) -> Result<(String, Vec<RunResult>)> {
    let dir = evals_dir()?.join("livecodebench");
    if !dir.join("lcb_runner").exists() {
        bail!("livecodebench not found in {}", dir.display());
    }
    let config: serde_json::Value = serde_json::from_str(&run.config).unwrap_or_default();
    let scenario = config.get("scenario").and_then(|v| v.as_str()).unwrap_or("codegeneration");
    let release = config.get("release_version").and_then(|v| v.as_str()).unwrap_or("release_v6");
    let n = config.get("n").and_then(|v| v.as_i64()).unwrap_or(5).to_string();
    // Upstream runner has no custom-endpoint flag: route via env (see oai_runner.py
    // BACKYARD_MODEL/OPENAI_BASE_URL patch). --model must be a valid store key;
    // use gpt-4o-mini as the OpenAIChat routing key, real id goes on the wire.
    // run_capture has no env param, so export via a wrapper: set env in a shell cmd.
    let base_v1 = format!("{}/v1", endpoint_base_root(endpoint));
    let out = {
        let mut cmd = std::process::Command::new("python3");
        cmd.args([
            "-m", "lcb_runner.runner.main",
            "--model", "gpt-4o-mini-2024-07-18",
            "--scenario", scenario,
            "--release_version", release,
            "--n", &n,
            "--evaluate",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1")
        .env("OPENAI_BASE_URL", &base_v1)
        .env("OPENAI_API_KEY", endpoint.api_key.clone().unwrap_or_else(|| "none".to_string()))
        .env("CCGW_PROVIDER", endpoint.provider.clone().unwrap_or_else(|| "generic".to_string()))
        .env("BACKYARD_MODEL", model_name(endpoint))
        .current_dir(&dir);
        let output = cmd.output().context("failed to run harness")?;
        format!("{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr))
    };
    // Parse eval metrics from stdout/stderr (lcb prints pass@k)
    let mut results = Vec::new();
    for line in out.lines() {
        if let Some(v) = line.strip_prefix("LCB_PASS@1 ") { if let Ok(f) = v.trim().parse::<f64>() {
            results.push(RunResult { id: 0, run_id: 0, metric: "lcb_pass_at_1".into(), value: f, unit: Some("pass".into()), extra: None });
        }}
        if line.contains("pass@1") {
            if let Some(cap) = line.split("pass@1").nth(1) {
                let tok = cap.split(|c: char| !c.is_ascii_digit() && c != '.').find(|s| !s.is_empty());
                if let Some(s) = tok { if let Ok(f) = s.parse::<f64>() {
                    if f > 0.0 && f <= 1.0 { results.push(RunResult { id: 0, run_id: 0, metric: "lcb_pass_at_1".into(), value: f, unit: Some("pass".into()), extra: None }); }
                }}
            }
        }
    }
    if results.is_empty() {
        // Try output JSON files: newest *_eval.json under output/ (model_repr dir
        // varies with routing key; glob by mtime instead of guessing).
        let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
        let out_dir = dir.join("output");
        for entry in walkdir::WalkDir::new(&out_dir).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path().to_path_buf();
            if p.extension().and_then(|e| e.to_str()) == Some("json")
                && p.file_name().and_then(|n| n.to_str()).map(|n| n.ends_with("_eval.json")).unwrap_or(false)
            {
                if let Ok(meta) = p.metadata() {
                    let t = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                    if newest.as_ref().map(|(nt, _)| t > *nt).unwrap_or(true) {
                        newest = Some((t, p));
                    }
                }
            }
        }
        if let Some((_, out_path)) = newest {
            if let Ok(text) = std::fs::read_to_string(&out_path) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(v) = parsed.get("pass@1").and_then(|v| v.as_f64()) {
                        results.push(RunResult { id: 0, run_id: 0, metric: "lcb_pass_at_1".into(), value: v, unit: Some("pass".into()), extra: Some(format!("{{\"file\":\"{}\"}}", out_path.display())) });
                    }
                }
            }
        }
    }
    if results.is_empty() { bail!("no livecodebench metrics in output (see stdout)"); }
    Ok((out, results))
}

fn run_swebench(run: &Run, endpoint: &EvalEndpoint) -> Result<(String, Vec<RunResult>)> {
    let dir = evals_dir()?.join("swe-bench");
    if !dir.exists() { bail!("swe-bench not found in {}", dir.display()); }
    let config: serde_json::Value = serde_json::from_str(&run.config).unwrap_or_default();
    let dataset = config.get("dataset").and_then(|v| v.as_str()).unwrap_or("princeton-nlp/SWE-bench_Verified");
    let split = config.get("split").and_then(|v| v.as_str()).unwrap_or("test");
    let n = config.get("n").and_then(|v| v.as_i64()).unwrap_or(10).to_string();
    let out = run_capture(
        "python3",
        &[
            "-m", "swebench.harness.run_evaluation",
            "--dataset_name", dataset,
            "--split", split,
            "--model_name", &format!("openai/{}", model_name(endpoint)),
            "--max_workers", &n,
        ],
        Some(&dir),
    )?;
    let mut results = Vec::new();
    for line in out.lines() {
        if line.contains("resolved") || line.contains("pass_rate") {
            for tok in line.split_whitespace() {
                if let Ok(f) = tok.trim_matches(|c: char| !c.is_ascii_digit() && c != '.').parse::<f64>() {
                    if f > 0.0 && f <= 1.0 { results.push(RunResult { id: 0, run_id: 0, metric: "swebench_resolved".into(), value: f, unit: Some("rate".into()), extra: None }); }
                }
            }
        }
    }
    if results.is_empty() { bail!("no swebench metrics in output"); }
    Ok((out, results))
}

fn run_toolathlon(_run: &Run, endpoint: &EvalEndpoint) -> Result<(String, Vec<RunResult>)> {
    let dir = evals_dir()?.join("toolathlon");
    if !dir.exists() { bail!("toolathlon not found in {}", dir.display()); }
    let out = run_capture(
        "python3",
        &["-m", "toolathlon.run", "--model", &format!("openai/{}", model_name(endpoint)), "--api_base", &format!("{}/v1", endpoint_base_root(endpoint))],
        Some(&dir),
    )?;
    let mut results = Vec::new();
    for line in out.lines() {
        if line.to_lowercase().contains("score") || line.to_lowercase().contains("success") {
            for tok in line.split(|c: char| c == ':' || c == '=' || c == ',') {
                if let Ok(f) = tok.trim().parse::<f64>() { if f > 0.0 && f <= 1.0 { results.push(RunResult { id: 0, run_id: 0, metric: "toolathlon_score".into(), value: f, unit: Some("score".into()), extra: None }); } }
            }
        }
    }
    if results.is_empty() { bail!("no toolathlon metrics in output"); }
    Ok((out, results))
}

fn run_frontier_swe(_run: &Run, endpoint: &EvalEndpoint) -> Result<(String, Vec<RunResult>)> {
    let dir = evals_dir()?.join("frontier-swe");
    if !dir.exists() { bail!("frontier-swe not found in {}", dir.display()); }
    let out = run_capture("python3", &["run.py", "--model", &format!("openai/{}", model_name(endpoint)), "--api_base", &format!("{}/v1", endpoint_base_root(endpoint))], Some(&dir))?;
    let mut results: Vec<RunResult> = Vec::new();
    // frontier-swe outputs per-task dominance; aggregate
    for line in out.lines() { if line.contains("dominance") { for tok in line.split_whitespace() { if let Ok(f) = tok.parse::<f64>() { results.push(RunResult { id: 0, run_id: 0, metric: "frontierswe_score".into(), value: f, unit: Some("score".into()), extra: None }); } } } }
    if results.is_empty() { bail!("no frontier-swe metrics"); }
    Ok((out, results))
}

fn run_perf_takehome(run: &Run, endpoint: &EvalEndpoint) -> Result<(String, Vec<RunResult>)> {
    let dir = evals_dir()?;
    let script = dir.join("perf_takehome_eval.py");
    if !script.exists() {
        bail!("perf_takehome_eval.py not found in {}", dir.display());
    }
    if !dir.join("perf-takehome/perf_takehome.py").exists() {
        bail!("perf-takehome checkout not found in {}", dir.display());
    }
    let config: serde_json::Value = serde_json::from_str(&run.config).unwrap_or_default();
    let max_rounds = config.get("max_rounds").and_then(|v| v.as_i64()).unwrap_or(12).to_string();
    let max_tokens = config.get("max_tokens").and_then(|v| v.as_i64()).unwrap_or(4096).to_string();
    let out_path = dir.join(format!("results/run-{}.json", run.id));
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let out = run_capture_env(
        "python3",
        &[
            script.to_str().unwrap(),
            "--base-url",
            &format!("{}/v1", endpoint_base_root(endpoint)),
            "--model",
            &model_name(endpoint),
            "--max-rounds",
            &max_rounds,
            "--max-tokens",
            &max_tokens,
            "--no-thinking",
            "--api-key",
            &endpoint.api_key.clone().unwrap_or_default(),
            "--output",
            out_path.to_str().unwrap(),
        ],
        Some(&dir),
        &endpoint_driver_env(endpoint),
    )?;
    let results_text = std::fs::read_to_string(&out_path).unwrap_or_default();
    let source = if !results_text.trim().is_empty() { results_text } else { out.clone() };
    let parsed: serde_json::Value = serde_json::from_str(&source)
        .map_err(|e| anyhow::anyhow!("bad perf_takehome JSON: {}", e))?;
    let scoring = parsed.get("scoring").ok_or_else(|| anyhow::anyhow!("no scoring in perf_takehome output"))?;
    let mut results = Vec::new();
    let correct = scoring.get("correct").and_then(|v| v.as_bool()).unwrap_or(false);
    let untouched = scoring.get("tests_untouched").and_then(|v| v.as_bool()).unwrap_or(false);
    let valid = correct && untouched;
    let cycles = scoring.get("cycles").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let speedup = scoring.get("speedup").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let beaten = scoring.get("thresholds_beaten").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0) as f64;
    let extra = serde_json::json!({
        "cycles": scoring.get("cycles"), "correct": correct,
        "tests_untouched": untouched, "thresholds_beaten": scoring.get("thresholds_beaten"),
    }).to_string();
    results.push(RunResult { id: 0, run_id: 0, metric: "perf_speedup".into(), value: if valid { speedup } else { 0.0 }, unit: Some("x".into()), extra: Some(extra.clone()) });
    results.push(RunResult { id: 0, run_id: 0, metric: "perf_cycles".into(), value: cycles, unit: Some("cycles".into()), extra: Some(extra.clone()) });
    results.push(RunResult { id: 0, run_id: 0, metric: "perf_thresholds".into(), value: if valid { beaten } else { 0.0 }, unit: Some("count".into()), extra: Some(extra) });
    Ok((out, results))
}

fn run_osworld(_run: &Run, _endpoint: &EvalEndpoint) -> Result<(String, Vec<RunResult>)> {
    bail!("osworld requires VM/desktop snapshot — run via osworld-v2 docker profile (see evals/osworld-v2/README.md)")
}
