#!/bin/bash
# Neural Codegen — Quick Start
# Run this to see the full S-expr → Typed IR → Rust pipeline in action.
#
# Prerequisites: Rust toolchain (rustup.rs)
# Usage: git clone ... && cd neural-codegen && ./quick-start.sh

set -e

echo "🔧 Building Neural Codegen..."
cargo build --example api-gen 2>&1 | tail -1

echo ""
echo "🚀 Running the pipeline: S-expr → Typed IR → Rust"
echo ""

cargo run -p nc-codegen --example api-gen
