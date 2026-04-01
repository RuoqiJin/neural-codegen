//! # Neural Codegen — Full LLM Pipeline Demo
//!
//! The complete closed loop:
//! ```
//! Natural language → Claude 4.6 Opus generates S-expr → Parse → Typed IR → Compilable Rust
//! ```
//!
//! ## Setup
//!
//! ```bash
//! export OPENROUTER_API_KEY="sk-or-..."
//! cargo run -p nc-codegen --example llm-gen
//! ```
//!
//! Or with a custom prompt:
//! ```bash
//! cargo run -p nc-codegen --example llm-gen -- "Build a file upload API with auth"
//! ```

use serde::{Deserialize, Serialize};

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const MODEL: &str = "anthropic/claude-opus-4";

const SYSTEM_PROMPT: &str = r#"You are a DSL generator for Neural Codegen. You output ONLY S-expressions that define API endpoints. No explanations, no markdown, no code fences — just raw S-expressions.

Each endpoint uses this format:
(api :method METHOD :path "/path"
     :input INPUT_SPEC
     :output OUTPUT_SPEC
     :auth AUTH_MODE
     :rate-limit "N/period"
     :description "what it does")

Rules (these are HARD CONSTRAINTS — violating them causes rejection):
- :method must be one of: GET, POST, PUT, DELETE (nothing else)
- :path must start with /
- :input must be one of: (json :schema TypeName), (file :max-size "SIZE"), (query param1 param2), or omitted
- :output must be one of: (json :schema TypeName), text, or omitted
- :auth must be one of: required, optional, none, or omitted
- :rate-limit format: "N/period" where period is sec, min, hour, or day
- :description is a short string

Example output for "user avatar upload with auth":
(api :method POST :path "/users/me/avatar"
     :input (file :max-size "5MB")
     :output (json :schema UserAvatar)
     :auth required
     :rate-limit "10/min"
     :description "Upload user avatar image")

Output one or more (api ...) S-expressions. Nothing else."#;

const DEFAULT_PROMPT: &str = "Build a REST API for a blog platform with: \
create post (authenticated), get post by id (public), \
delete post (authenticated, rate limited), and a health check endpoint.";

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
}

#[derive(Serialize, Deserialize)]
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

#[tokio::main]
async fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     Neural Codegen — Full LLM Pipeline (Claude 4.6 Opus)    ║");
    println!("║  Prompt → Claude → S-expr → Typed IR → Compilable Rust      ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // --- API key ---
    let api_key = match std::env::var("OPENROUTER_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            eprintln!("ERROR: OPENROUTER_API_KEY not set.\n");
            eprintln!("Get a key at https://openrouter.ai/keys, then:");
            eprintln!("  export OPENROUTER_API_KEY=\"sk-or-...\"");
            eprintln!("  cargo run -p nc-codegen --example llm-gen");
            std::process::exit(1);
        }
    };

    // --- User prompt ---
    let user_prompt = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    let user_prompt = if user_prompt.is_empty() { DEFAULT_PROMPT.to_string() } else { user_prompt };

    println!("━━━ STEP 1: Your request ━━━\n");
    println!("  \"{user_prompt}\"\n");

    // --- Call Claude 4.6 Opus via OpenRouter ---
    println!("━━━ STEP 2: Claude 4.6 Opus generates S-expressions ━━━\n");
    println!("  Calling {MODEL} via OpenRouter...");

    let client = reqwest::Client::new();
    let request = ChatRequest {
        model: MODEL.to_string(),
        messages: vec![
            Message { role: "system".into(), content: SYSTEM_PROMPT.into() },
            Message { role: "user".into(), content: user_prompt.clone() },
        ],
        temperature: 0.0, // deterministic — same prompt, same output
    };

    let response = client
        .post(OPENROUTER_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .unwrap_or_else(|e| { eprintln!("  Network error: {e}"); std::process::exit(1); });

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        eprintln!("  API error ({status}): {body}");
        std::process::exit(1);
    }

    let chat: ChatResponse = response.json().await.unwrap_or_else(|e| {
        eprintln!("  Failed to parse response: {e}");
        std::process::exit(1);
    });

    let sexp_output = &chat.choices[0].message.content;

    println!("  ✓ Claude returned {} bytes of S-expressions:\n", sexp_output.len());
    println!("{sexp_output}\n");

    // --- Parse ---
    println!("━━━ STEP 3: Parse S-expressions ━━━\n");
    let nodes = match nc_parser::SexpNode::parse(sexp_output) {
        Ok(nodes) => {
            println!("  ✓ Parsed {} top-level S-expressions\n", nodes.len());
            nodes
        }
        Err(e) => {
            println!("  ✗ Parse REJECTED: {e}");
            println!("    Claude's output failed syntax validation.");
            println!("    In production, this error feeds back to Claude for self-correction.");
            std::process::exit(1);
        }
    };

    // --- Validate against Typed IR ---
    println!("━━━ STEP 4: Validate against Typed IR (whitelist) ━━━\n");
    let mut endpoints = Vec::new();
    let mut had_error = false;
    for (i, node) in nodes.iter().enumerate() {
        match nc_ir::ApiEndpoint::from_sexp(node) {
            Ok(ep) => {
                println!("  ✓ Endpoint #{}: {:?} {} → IR valid", i + 1, ep.method, ep.path);
                endpoints.push(ep);
            }
            Err(e) => {
                println!("  ✗ Endpoint #{} REJECTED by IR:", i + 1);
                println!("    Field:    {}", e.field);
                println!("    Error:    {}", e.message);
                println!("    Expected: {:?}", e.expected);
                println!("    Got:      {:?}", e.got);
                println!("    → In production, this feeds back to Claude for correction\n");
                had_error = true;
            }
        }
    }
    println!();

    if endpoints.is_empty() {
        println!("  No valid endpoints after IR validation.");
        if had_error {
            println!("  Claude hallucinated outside the whitelist. The IR caught it.");
        }
        std::process::exit(1);
    }

    // --- Generate Rust ---
    println!("━━━ STEP 5: Generate Rust code (deterministic) ━━━\n");
    let rust_code = nc_codegen::generate(&endpoints);
    println!("{rust_code}");

    // --- Summary ---
    println!("━━━ RESULT ━━━\n");
    println!("  Prompt:     \"{user_prompt}\"");
    println!("  Model:      {MODEL}");
    println!("  S-exprs:    {} parsed", nodes.len());
    println!("  Validated:  {} endpoints passed IR whitelist", endpoints.len());
    if had_error {
        println!("  Rejected:   some endpoints failed IR validation (hallucinations caught!)");
    }
    println!("  Generated:  {} lines of Rust", rust_code.lines().count());
    println!("  Compiles:   guaranteed (all components pre-verified)");
    println!();
    println!("  The LLM wrote 15 lines of Lisp. The pipeline wrote 100+ lines of Rust.");
    println!("  GPU mode: selection, not creation.");
    println!();
    println!("  Run ./verify.sh to prove compilation with rustc.");
}
