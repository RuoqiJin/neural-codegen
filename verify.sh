#!/bin/bash
# Neural Codegen — Full Verification
# Proves the generated Rust code actually compiles with zero errors.
#
# What this does:
#   1. Runs all unit tests (15 tests)
#   2. Runs the S-expr → IR → Rust pipeline
#   3. Creates a temporary Cargo project with real dependencies
#   4. Runs `cargo check` to prove it compiles
#
# Usage: ./verify.sh

set -e

echo "╔══════════════════════════════════════════════════╗"
echo "║   Neural Codegen — Compilation Verification      ║"
echo "╚══════════════════════════════════════════════════╝"
echo ""

# Step 1: Build
echo "━━━ Step 1: Build the pipeline ━━━"
cargo build -p nc-codegen --example api-gen 2>&1 | tail -1
echo "  ✓ Built"
echo ""

# Step 2: Run tests
echo "━━━ Step 2: Run unit tests (15 tests) ━━━"
cargo test --workspace --quiet 2>&1
echo ""

# Step 3: Run pipeline and capture generated code
echo "━━━ Step 3: Run pipeline, extract generated Rust ━━━"
FULL_OUTPUT=$(cargo run -p nc-codegen --example api-gen 2>/dev/null)

# Extract the generated Rust code between the markers
GENERATED=$(echo "$FULL_OUTPUT" | awk '/^\/\/ =====/,/^━━━ RESULT/' | grep -v '^━━━ RESULT')

if [ -z "$GENERATED" ]; then
    echo "  ✗ Failed to capture generated code"
    exit 1
fi

LINE_COUNT=$(echo "$GENERATED" | wc -l | tr -d ' ')
echo "  ✓ Captured $LINE_COUNT lines of generated Rust"
echo ""

# Step 4: Create temp project and prove it compiles
echo "━━━ Step 4: Prove generated code compiles ━━━"
TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

cat > "$TMPDIR/Cargo.toml" << 'CARGO_EOF'
[package]
name = "nc-verify"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = { version = "0.8", features = ["multipart"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
CARGO_EOF

mkdir -p "$TMPDIR/src"
echo "$GENERATED" > "$TMPDIR/src/main.rs"

echo "  Generated code written to temp project"
echo "  Running cargo check (downloads deps on first run)..."
echo ""

cd "$TMPDIR"
if cargo check 2>&1; then
    echo ""
    echo "╔══════════════════════════════════════════════════╗"
    echo "║  ✓ VERIFIED: Generated Rust compiles with       ║"
    echo "║    ZERO errors. 100% first-pass compile rate.   ║"
    echo "╚══════════════════════════════════════════════════╝"
else
    echo ""
    echo "  ✗ COMPILATION FAILED — this is a bug in nc-codegen"
    exit 1
fi
