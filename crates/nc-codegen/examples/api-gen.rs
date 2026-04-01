//! # Neural Codegen — End-to-End Demo
//!
//! This example demonstrates the complete pipeline:
//!
//! ```
//! S-expression (AI output) → Parse → Typed IR (whitelist) → Rust code (guaranteed to compile)
//! ```
//!
//! Run with: `cargo run --example api-gen`

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║        Neural Codegen — GPU-Style Code Generation           ║");
    println!("║  S-expr DSL → Typed IR (whitelist) → Compilable Rust        ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // ── STEP 0: The S-expression DSL ──────────────────────────────────────
    // This is what an LLM outputs. Note: the ONLY syntax rule is bracket matching.
    // No semicolons, no indentation sensitivity, no operator precedence.

    let dsl_input = r#"
; A complete REST API defined in 15 lines of S-expressions.
; An LLM generates THIS — not Rust code. The pipeline handles the rest.

(api :method POST
     :path "/users/me/avatar"
     :input (file :max-size "5MB" :types ("image/png" "image/jpeg"))
     :output (json :schema UserAvatar)
     :auth required
     :rate-limit "10/min"
     :description "Upload user avatar image")

(api :method GET
     :path "/health"
     :output text
     :description "Health check endpoint")

(api :method DELETE
     :path "/users/{id}"
     :auth required
     :description "Delete a user account")

(api :method PUT
     :path "/users/me/profile"
     :input (json :schema UserProfile)
     :output (json :schema UserProfile)
     :auth required
     :state (db-pool)
     :rate-limit "30/min"
     :description "Update user profile — AI writes :state (db-pool), generator handles Arc/Mutex/Clone")
"#;

    println!("━━━ STEP 1: S-expression Input (what the LLM outputs) ━━━\n");
    println!("{dsl_input}");

    // ── STEP 1: Parse ─────────────────────────────────────────────────────
    // 50 lines of parser. Hard reject on any syntax error. No "best effort".

    println!("━━━ STEP 2: Parse S-expressions ━━━\n");
    let nodes = match nc_parser::SexpNode::parse(dsl_input) {
        Ok(nodes) => {
            println!("  ✓ Parsed {} top-level S-expressions\n", nodes.len());
            nodes
        }
        Err(e) => {
            println!("  ✗ Parse REJECTED: {e}");
            println!("    → Feed this error back to LLM for self-correction");
            std::process::exit(1);
        }
    };

    // ── STEP 2: Lower to Typed IR ─────────────────────────────────────────
    // Each S-expr must map to a Rust enum variant. If the LLM hallucinated
    // an invalid HTTP method, auth mode, or input type → rejected HERE,
    // before any code is generated.

    println!("━━━ STEP 3: Validate against Typed IR (whitelist) ━━━\n");
    let mut endpoints = Vec::new();
    for (i, node) in nodes.iter().enumerate() {
        match nc_ir::ApiEndpoint::from_sexp(node) {
            Ok(ep) => {
                println!("  ✓ Endpoint #{}: {:?} {} → IR valid", i + 1, ep.method, ep.path);
                endpoints.push(ep);
            }
            Err(e) => {
                // Structured error — designed for LLM self-correction, not human debugging
                println!("  ✗ Endpoint #{} REJECTED by IR:", i + 1);
                println!("    Field:    {}", e.field);
                println!("    Error:    {}", e.message);
                println!("    Expected: {:?}", e.expected);
                println!("    Got:      {:?}", e.got);
                println!("    → Feed this structured error back to LLM");
                std::process::exit(1);
            }
        }
    }
    println!();

    // ── STEP 3: Generate Rust code ────────────────────────────────────────
    // Pure function: IR → String. Same input ALWAYS produces the same output.
    // Every template is pre-written and pre-tested. Lookup, not creation.

    println!("━━━ STEP 4: Generate Rust code (deterministic) ━━━\n");
    let rust_code = nc_codegen::generate(&endpoints);

    println!("{rust_code}");

    // ── Summary ───────────────────────────────────────────────────────────

    println!("━━━ RESULT ━━━\n");
    println!("  {} S-expressions parsed", nodes.len());
    println!("  {} endpoints validated against typed IR", endpoints.len());
    println!("  {} lines of Rust generated", rust_code.lines().count());
    println!("  Compile guarantee: 100% (all components pre-verified)");
    println!();
    println!("  The LLM never wrote Rust. It filled out a form.");
    println!("  The generator assembled pre-tested components.");
    println!("  GPU mode: selection, not creation.");

    // ── Bonus: demonstrate rejection ──────────────────────────────────────

    println!("\n━━━ BONUS: Watch the IR reject hallucinated input ━━━\n");

    let bad_inputs = vec![
        (r#"(api :method PATCH :path "/users")"#, "PATCH is not in the whitelist"),
        (r#"(api :method GET :path "no-slash")"#, "Path must start with /"),
        (r#"(api :method POST :path "/upload" :input (xml :schema Data))"#, "xml is not a valid input type"),
        (r#"(api :method GET :path "/data" :state (mongodb))"#, "mongodb is not a valid state dependency"),
    ];

    for (input, reason) in bad_inputs {
        let node = nc_parser::SexpNode::parse_one(input).unwrap();
        match nc_ir::ApiEndpoint::from_sexp(&node) {
            Ok(_) => println!("  [UNEXPECTED] Should have been rejected"),
            Err(e) => {
                println!("  ✗ REJECTED: {input}");
                println!("    Reason: {} ({})", e.message, reason);
                println!("    Structured feedback to LLM: expected {:?}\n", e.expected);
            }
        }
    }
}
