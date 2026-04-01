//! # Neural Codegen — Benchmark: Pipeline vs Raw LLM
//!
//! Compares two approaches:
//! - **Pipeline mode**: LLM generates S-expr → nc-parser → nc-ir → nc-codegen → Rust
//! - **Raw mode**: LLM generates Rust directly
//!
//! For each test case, both approaches are run and the output is checked with `cargo check`.
//!
//! ## Usage
//! ```bash
//! export OPENROUTER_API_KEY="sk-or-..."
//! cargo run -p nc-codegen --example benchmark
//! ```

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::Command;

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const MODEL: &str = "anthropic/claude-opus-4";

const SEXP_SYSTEM_PROMPT: &str = r#"You are a DSL generator. Output ONLY S-expressions defining API endpoints. No explanations, no markdown, no code fences.

Format:
(api :method METHOD :path "/path"
     :input INPUT_SPEC
     :output OUTPUT_SPEC
     :auth AUTH_MODE
     :state STATE_DEPS
     :rate-limit "N/period"
     :description "what it does")

Rules:
- :method: GET, POST, PUT, DELETE only
- :path: must start with /
- :input: (json :schema TypeName), (file :max-size "SIZE"), (query param1 param2), or omit
- :output: (json :schema TypeName), text, or omit
- :auth: required, optional, none, or omit
- :state: (db-pool), (cache), (db-pool cache), or omit
- :rate-limit: "N/period" where period is sec, min, hour, day
- :description: short string

Output one or more (api ...) S-expressions. Nothing else."#;

const RUST_SYSTEM_PROMPT: &str = r#"You are a Rust backend developer. Generate a COMPLETE, COMPILABLE Rust source file using axum 0.8.

The output must:
- Include all use statements
- Include all struct definitions with appropriate derives
- Include a Router with all routes
- Include a #[tokio::main] async fn main()
- Use axum 0.8 API (Router, routing::{get,post,put,delete}, Json, etc.)
- Compile with: axum = { version = "0.8", features = ["multipart"] }, tokio = { version = "1", features = ["full"] }, serde = { version = "1", features = ["derive"] }

Output ONLY the Rust source code. No markdown, no explanations, no code fences."#;

const TEST_CASES: &[(&str, &str)] = &[
    ("simple_health", "Create a health check endpoint: GET /health that returns plain text 'OK'."),
    ("crud_users", "Create a REST API for user management: POST /users (create, takes JSON UserRequest, returns JSON User), GET /users/:id (get by id, returns JSON User), DELETE /users/:id (requires auth)."),
    ("file_upload", "Create a file upload endpoint: POST /upload/avatar that accepts multipart file upload with 5MB max size, requires authentication, returns JSON with the uploaded file URL. Rate limit 10/min."),
    ("stateful_api", "Create a blog API with database: POST /posts (create post, requires auth, needs database), GET /posts/:id (public, needs database), PUT /posts/:id (update, requires auth, needs database and cache), DELETE /posts/:id (requires auth, rate limited 5/hour)."),
    ("mixed_io", "Create 3 endpoints: GET /search with query parameters 'q' and 'page', POST /feedback with JSON FeedbackRequest body, GET /metrics that returns plain text."),
    ("auth_variants", "Create endpoints with different auth modes: GET /public/feed (no auth), GET /private/dashboard (required auth), POST /api/webhook (optional auth, takes JSON WebhookPayload)."),
    ("rate_limited", "Create rate-limited endpoints: POST /api/generate (auth required, rate limit 5/min, JSON input GenerateRequest, JSON output GenerateResponse), GET /api/status (no auth, rate limit 100/min, returns text)."),
    ("complex_state", "Create a payment API: POST /payments/transfer (requires auth, needs db-pool and cache, JSON input TransferRequest, JSON output TransferResult, rate limit 10/min), GET /payments/:id (requires auth, needs db-pool, returns JSON Payment)."),
];

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
}

#[derive(Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

struct BenchmarkResult {
    name: String,
    pipeline_compiled: bool,
    pipeline_lines: usize,
    pipeline_error: String,
    raw_compiled: bool,
    raw_lines: usize,
    raw_error: String,
}

#[tokio::main]
async fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║   Neural Codegen — Benchmark: Pipeline vs Raw LLM           ║");
    println!("║   Model: Claude 4.6 Opus | Target: Rust (axum 0.8)          ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let api_key = match std::env::var("OPENROUTER_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            eprintln!("ERROR: OPENROUTER_API_KEY not set.");
            std::process::exit(1);
        }
    };

    let client = reqwest::Client::new();
    let mut results: Vec<BenchmarkResult> = Vec::new();

    for (i, (name, prompt)) in TEST_CASES.iter().enumerate() {
        println!("━━━ Test {}/{}: {} ━━━\n", i + 1, TEST_CASES.len(), name);

        // --- Pipeline mode: LLM → S-expr → IR → Rust ---
        print!("  [Pipeline] Calling Claude for S-expr... ");
        std::io::stdout().flush().unwrap();
        let sexp_output = call_llm(&client, &api_key, SEXP_SYSTEM_PROMPT, prompt).await;
        let pipeline_result = match &sexp_output {
            Ok(sexp) => {
                println!("got {} bytes", sexp.len());
                run_pipeline(sexp)
            }
            Err(e) => {
                println!("API error");
                (false, 0, format!("LLM API error: {e}"))
            }
        };

        // --- Raw mode: LLM → Rust directly ---
        print!("  [Raw LLM]  Calling Claude for Rust... ");
        std::io::stdout().flush().unwrap();
        let rust_output = call_llm(&client, &api_key, RUST_SYSTEM_PROMPT, prompt).await;
        let raw_result = match &rust_output {
            Ok(code) => {
                println!("got {} bytes", code.len());
                let clean = strip_markdown_fences(code);
                check_compilation(&clean)
            }
            Err(e) => {
                println!("API error");
                (false, 0, format!("LLM API error: {e}"))
            }
        };

        let result = BenchmarkResult {
            name: name.to_string(),
            pipeline_compiled: pipeline_result.0,
            pipeline_lines: pipeline_result.1,
            pipeline_error: pipeline_result.2,
            raw_compiled: raw_result.0,
            raw_lines: raw_result.1,
            raw_error: raw_result.2,
        };

        println!("  Pipeline: {} ({} lines)", if result.pipeline_compiled { "✓ COMPILED" } else { "✗ FAILED" }, result.pipeline_lines);
        if !result.pipeline_compiled && !result.pipeline_error.is_empty() {
            println!("    Error: {}", truncate(&result.pipeline_error, 120));
        }
        println!("  Raw LLM:  {} ({} lines)", if result.raw_compiled { "✓ COMPILED" } else { "✗ FAILED" }, result.raw_lines);
        if !result.raw_compiled && !result.raw_error.is_empty() {
            println!("    Error: {}", truncate(&result.raw_error, 120));
        }
        println!();

        results.push(result);
    }

    // --- Summary ---
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                      BENCHMARK RESULTS                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("  {:<20} {:>12} {:>12}", "Test Case", "Pipeline", "Raw LLM");
    println!("  {}", "─".repeat(46));
    for r in &results {
        println!("  {:<20} {:>12} {:>12}",
            r.name,
            if r.pipeline_compiled { "✓ compiled" } else { "✗ failed" },
            if r.raw_compiled { "✓ compiled" } else { "✗ failed" },
        );
    }

    let pipeline_pass = results.iter().filter(|r| r.pipeline_compiled).count();
    let raw_pass = results.iter().filter(|r| r.raw_compiled).count();
    let total = results.len();

    println!("\n  ━━━ Pass@1 Compilation Rate ━━━\n");
    println!("  Pipeline (S-expr → IR → Rust):  {}/{} ({:.0}%)", pipeline_pass, total, 100.0 * pipeline_pass as f64 / total as f64);
    println!("  Raw LLM (direct Rust):          {}/{} ({:.0}%)", raw_pass, total, 100.0 * raw_pass as f64 / total as f64);
    println!();

    if pipeline_pass > raw_pass {
        println!("  Pipeline advantage: +{} test cases ({:.0}% improvement)",
            pipeline_pass - raw_pass,
            100.0 * (pipeline_pass - raw_pass) as f64 / total as f64);
    }

    println!("\n  Model: {MODEL}");
    println!("  Temperature: 0.0 (deterministic)");
    println!("  Target: Rust (axum 0.8 + tokio + serde)");
}

async fn call_llm(client: &reqwest::Client, api_key: &str, system: &str, prompt: &str) -> Result<String, String> {
    let request = ChatRequest {
        model: MODEL.to_string(),
        messages: vec![
            Message { role: "system".into(), content: system.into() },
            Message { role: "user".into(), content: prompt.into() },
        ],
        temperature: 0.0,
    };

    let response = client
        .post(OPENROUTER_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API {status}: {}", truncate(&body, 200)));
    }

    let chat: ChatResponse = response.json().await.map_err(|e| e.to_string())?;
    Ok(chat.choices[0].message.content.clone())
}

fn run_pipeline(sexp_input: &str) -> (bool, usize, String) {
    // Parse
    let nodes = match nc_parser::SexpNode::parse(sexp_input) {
        Ok(n) => n,
        Err(e) => return (false, 0, format!("Parse error: {e}")),
    };

    // Validate IR
    let mut endpoints = Vec::new();
    for node in &nodes {
        match nc_ir::ApiEndpoint::from_sexp(node) {
            Ok(ep) => endpoints.push(ep),
            Err(e) => return (false, 0, format!("IR error: {e}")),
        }
    }

    if endpoints.is_empty() {
        return (false, 0, "No valid endpoints".into());
    }

    // Generate
    let code = nc_codegen::generate(&endpoints);
    let lines = code.lines().count();

    // Check compilation
    let (compiled, _, error) = check_compilation(&code);
    (compiled, lines, error)
}

fn check_compilation(code: &str) -> (bool, usize, String) {
    let lines = code.lines().count();
    let tmpdir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => return (false, lines, format!("tmpdir error: {e}")),
    };

    // Write Cargo.toml
    let cargo_toml = r#"[package]
name = "nc-bench"
version = "0.1.0"
edition = "2021"
[dependencies]
axum = { version = "0.8", features = ["multipart"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
"#;
    let cargo_path = tmpdir.path().join("Cargo.toml");
    std::fs::write(&cargo_path, cargo_toml).ok();

    let src_dir = tmpdir.path().join("src");
    std::fs::create_dir_all(&src_dir).ok();
    std::fs::write(src_dir.join("main.rs"), code).ok();

    let output = Command::new("cargo")
        .arg("check")
        .current_dir(tmpdir.path())
        .output();

    match output {
        Ok(o) => {
            let compiled = o.status.success();
            let stderr = String::from_utf8_lossy(&o.stderr).to_string();
            let error = if compiled {
                String::new()
            } else {
                extract_first_error(&stderr)
            };
            (compiled, lines, error)
        }
        Err(e) => (false, lines, format!("cargo error: {e}")),
    }
}

fn extract_first_error(stderr: &str) -> String {
    for line in stderr.lines() {
        if line.starts_with("error[") || line.starts_with("error:") {
            return line.to_string();
        }
    }
    stderr.lines().last().unwrap_or("unknown error").to_string()
}

fn strip_markdown_fences(code: &str) -> String {
    let mut lines: Vec<&str> = code.lines().collect();
    if lines.first().map(|l| l.starts_with("```")).unwrap_or(false) {
        lines.remove(0);
    }
    if lines.last().map(|l| l.trim() == "```").unwrap_or(false) {
        lines.pop();
    }
    lines.join("\n")
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}...", &s[..max]) }
}
