use anyhow::{Context, Result};
use serde::Deserialize;
use std::time::Instant;

use crate::state::{EvalEndpoint, RunResult};

#[derive(Debug, Deserialize)]
struct BenchHttpConfig {
    #[serde(default = "default_prompt")]
    prompt: String,
    #[serde(default = "default_iterations")]
    iterations: usize,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default = "default_temperature")]
    temperature: f32,
    #[serde(default)]
    stream: bool,
}

fn default_prompt() -> String {
    "Write a short story about a robot learning to paint.".into()
}
fn default_iterations() -> usize {
    5
}
fn default_temperature() -> f32 {
    0.7
}

fn endpoint_chat_url(ep: &EvalEndpoint) -> String {
    let base = ep.base_url.trim_end_matches('/');
    let root = base.strip_suffix("/v1").unwrap_or(base);
    // Ollama native API uses /api/chat; everything else is OpenAI-compatible
    if root.contains("ollama") && !base.ends_with("/chat/completions") {
        // allow direct ollama base; caller should provide full URL but handle fallback
        format!("{}/v1/chat/completions", root)
    } else if base.ends_with("/chat/completions") {
        base.to_string()
    } else {
        format!("{}/v1/chat/completions", root)
    }
}

/// Attach endpoint auth: ccgw uses `x-ccgw-key`, everything else Bearer.
fn apply_auth(req: reqwest::RequestBuilder, ep: &EvalEndpoint) -> reqwest::RequestBuilder {
    let key = ep.api_key.clone().unwrap_or_default();
    if key.is_empty() {
        return req;
    }
    if ep.provider.as_deref() == Some("ccgw") {
        req.header("x-ccgw-key", key)
    } else {
        req.bearer_auth(key)
    }
}

pub async fn run_bench_http(run: &crate::state::Run, endpoint: &EvalEndpoint) -> Result<(String, Vec<RunResult>)> {
    let cfg: BenchHttpConfig = serde_json::from_str(&run.config).unwrap_or(BenchHttpConfig {
        prompt: default_prompt(),
        iterations: default_iterations(),
        max_tokens: Some(128),
        temperature: default_temperature(),
        stream: false,
    });

    let url = endpoint_chat_url(endpoint);
    let model = endpoint.model.clone().unwrap_or_else(|| "default".into());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let iterations = cfg.iterations.clamp(1, 100);
    let mut latencies: Vec<f64> = Vec::new();
    let mut ttfts: Vec<f64> = Vec::new();
    let mut tps_vals: Vec<f64> = Vec::new();
    let mut failures = 0usize;
    let mut log_lines: Vec<String> = Vec::new();

    for i in 0..iterations {
        let body = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": cfg.prompt}],
            "max_tokens": cfg.max_tokens.unwrap_or(128),
            "temperature": cfg.temperature,
            "stream": cfg.stream,
        });

        let start = Instant::now();
        let req = apply_auth(client.post(&url).json(&body), endpoint);

        let resp = req.send().await;
        match resp {
            Ok(r) => {
                if !r.status().is_success() {
                    let status = r.status();
                    let text = r.text().await.unwrap_or_default();
                    failures += 1;
                    log_lines.push(format!("[{i}] HTTP {}: {}", status, &text[..text.len().min(500)]));
                    continue;
                }

                if cfg.stream {
                    // For streaming, measure TTFT as time to first chunk
                    // reqwest streaming: read bytes stream
                    use futures_util::StreamExt;
                    let mut stream = r.bytes_stream();
                    let mut first_chunk_at: Option<f64> = None;
                    let mut total_bytes = 0usize;
                    while let Some(chunk) = stream.next().await {
                        if first_chunk_at.is_none() {
                            first_chunk_at = Some(start.elapsed().as_secs_f64() * 1000.0);
                        }
                        if let Ok(b) = chunk {
                            total_bytes += b.len();
                        }
                    }
                    let total_ms = start.elapsed().as_secs_f64() * 1000.0;
                    latencies.push(total_ms);
                    if let Some(ttft) = first_chunk_at {
                        ttfts.push(ttft);
                    }
                    log_lines.push(format!("[{i}] stream total={:.0}ms ttft={:.0}ms bytes={}", total_ms, first_chunk_at.unwrap_or(0.0), total_bytes));
                    // approximate tps not available for stream without token counts
                } else {
                    let total_ms = start.elapsed().as_secs_f64() * 1000.0;
                    let json: serde_json::Value = r.json().await.context("parse chat completion json")?;
                    let usage = &json["usage"];
                    let prompt_tokens = usage["prompt_tokens"].as_u64().unwrap_or(0);
                    let completion_tokens = usage["completion_tokens"].as_u64().unwrap_or(0);
                    let latency = total_ms;
                    latencies.push(latency);
                    // tps = completion_tokens / (latency_s)
                    if completion_tokens > 0 && latency > 0.0 {
                        let tps = completion_tokens as f64 / (latency / 1000.0);
                        tps_vals.push(tps);
                    }
                    let content = json["choices"][0]["message"]["content"].as_str().unwrap_or("");
                    log_lines.push(format!(
                        "[{i}] latency={:.0}ms prompt={} completion={} preview={}",
                        latency,
                        prompt_tokens,
                        completion_tokens,
                        &content[..content.len().min(120)].replace('\n', " ")
                    ));
                }
            }
            Err(e) => {
                failures += 1;
                log_lines.push(format!("[{i}] request error: {}", e));
            }
        }
    }

    let success = iterations - failures;
    let success_rate = if iterations > 0 {
        success as f64 / iterations as f64
    } else {
        0.0
    };

    let mut results = Vec::new();
    let extra = serde_json::json!({
        "model": model,
        "endpoint": endpoint.name,
        "iterations": iterations,
        "success": success,
        "failures": failures,
    })
    .to_string();

    if !latencies.is_empty() {
        let mean_latency = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let p50 = percentile(&latencies, 50.0);
        let p95 = percentile(&latencies, 95.0);
        results.push(RunResult { id: 0, run_id: run.id, metric: "latency_ms".into(), value: mean_latency, unit: Some("ms".into()), extra: Some(extra.clone()) });
        results.push(RunResult { id: 0, run_id: run.id, metric: "latency_p50_ms".into(), value: p50, unit: Some("ms".into()), extra: Some(extra.clone()) });
        results.push(RunResult { id: 0, run_id: run.id, metric: "latency_p95_ms".into(), value: p95, unit: Some("ms".into()), extra: Some(extra.clone()) });
        // legacy alias for leaderboard
        results.push(RunResult { id: 0, run_id: run.id, metric: "tg_tps".into(), value: tps_vals.iter().copied().sum::<f64>() / tps_vals.len().max(1) as f64, unit: Some("tok/s".into()), extra: Some(extra.clone()) });
    }
    if !ttfts.is_empty() {
        let mean_ttft = ttfts.iter().sum::<f64>() / ttfts.len() as f64;
        results.push(RunResult { id: 0, run_id: run.id, metric: "ttft_ms".into(), value: mean_ttft, unit: Some("ms".into()), extra: Some(extra.clone()) });
        results.push(RunResult { id: 0, run_id: run.id, metric: "ttft_p50_ms".into(), value: percentile(&ttfts, 50.0), unit: Some("ms".into()), extra: Some(extra.clone()) });
    }
    if !tps_vals.is_empty() {
        let mean_tps = tps_vals.iter().sum::<f64>() / tps_vals.len() as f64;
        results.push(RunResult { id: 0, run_id: run.id, metric: "tps".into(), value: mean_tps, unit: Some("tok/s".into()), extra: Some(extra.clone()) });
    }
    results.push(RunResult { id: 0, run_id: run.id, metric: "success_rate".into(), value: success_rate, unit: None, extra: Some(extra.clone()) });

    // Ensure at least one result even if all failed (so run is not empty)
    if results.is_empty() {
        results.push(RunResult { id: 0, run_id: run.id, metric: "success_rate".into(), value: 0.0, unit: None, extra: Some(extra) });
    }

    let stdout = log_lines.join("\n");
    Ok((stdout, results))
}

fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[idx.clamp(0, sorted.len() - 1)]
}

fn api_root(base_url: &str) -> String {
    // Stored base_urls are inconsistent: some include /v1, some don't.
    let b = base_url.trim_end_matches('/');
    b.strip_suffix("/v1").unwrap_or(b).to_string()
}

pub async fn probe_endpoint(endpoint: &EvalEndpoint) -> Result<String> {
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build()?;
    let root = api_root(&endpoint.base_url);
    // Try /v1/models first (OpenAI-compatible)
    let models_url = format!("{}/v1/models", root);
    let req = apply_auth(client.get(&models_url), endpoint);
    let resp = req.send().await?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if status.is_success() {
        return Ok(text[..text.len().min(2000)].to_string());
    }
    // Fallback: try chat completions with minimal prompt
    let chat_url = endpoint_chat_url(endpoint);
    let model = endpoint.model.clone().unwrap_or_else(|| "default".into());
    let req2 = apply_auth(client.post(&chat_url).json(&serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 4
    })), endpoint);
    let resp2 = req2.send().await?;
    let s2 = resp2.status();
    let t2 = resp2.text().await.unwrap_or_default();
    Ok(format!("models: {} {}\nchat: {} {}", status, &text[..text.len().min(500)], s2, &t2[..t2.len().min(500)]))
}
