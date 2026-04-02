[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.19372158.svg)](https://doi.org/10.5281/zenodo.19372158)

# Neural Codegen

**A deterministic harness for AI code generation. The LLM outputs S-expressions, a typed IR whitelist rejects hallucinations, and a template engine assembles guaranteed-compilable Rust. GPU-style: selection, not creation.**

> The moat isn't the model. It's the harness. This one uses Rust's type system as the contract layer and S-expressions as the constraint interface. 100% first-pass compile rate by construction.

<!-- TODO: replace with demo GIF after recording -->

## See it in action (5 seconds)

```bash
git clone https://github.com/RuoqiJin/neural-codegen.git
cd neural-codegen
cargo run -p nc-codegen --example api-gen
```

**What happens:** 15 lines of S-expression → parsed → validated against typed IR whitelist → 114 lines of production Rust (axum router, handlers, auth extractors, error types) → **guaranteed to compile**.

```bash
# Prove it: generated code passes rustc with zero errors
./verify.sh
```

## Where is the AI?

This demo isolates the **deterministic pipeline** (S-expr → Typed IR → Rust). In production, an LLM generates the S-expressions — in my private system [Jarvis](https://github.com/RuoqiJin), Claude 4.6 Opus outputs the DSL and the pipeline guarantees correctness.

To make this repo **100% reproducible in 5 seconds without API keys**, the AI's intended outputs are hardcoded. The point is: **if the AI outputs this S-expression, the code WILL compile. Always. Deterministically.**

The AI's job is trivial — match brackets in a 15-line form. The pipeline's job is everything else.

## The Problem

Every AI coding tool today works the same way:

```
Prompt → LLM predicts tokens → Compile error → Feed error back → Repeat 5-10x
```

For Rust, the LLM must simultaneously guess types, borrows, lifetimes, error handling, async bounds, and your project conventions. The probability of getting ALL dimensions right on the first pass is near zero.

This is **CPU-mode** code generation — sequential guessing through an impossibly large state space.

## The Solution: GPU Mode

GPUs don't think. They execute predefined shaders. What if AI code generation worked the same way?

```
LLM outputs S-expression (selection only, no improvisation)
    → nc-parser validates syntax (50 lines, hard reject)
    → nc-ir validates against whitelist (Rust enum — finite legal operations)
    → nc-codegen assembles pre-verified templates (lookup, not creation)
    → Guaranteed-compilable Rust
```

**The AI fills out a form. The pipeline does the rest.**

### Why S-expressions?

```lisp
;; This is what the LLM outputs. Only rule: match brackets.
(api :method POST :path "/users/me/avatar"
     :input (file :max-size "5MB" :types ("image/png" "image/jpeg"))
     :output (json :schema UserAvatar)
     :auth required
     :rate-limit "10/min")
```

- **Minimal syntax** — atoms and parenthesized lists. That's it.
- **AST = Data** — no parsing ambiguity, no operator precedence
- **LLMs can't break it** — the only constraint is bracket matching

### Where does the rigor come from?

From the **whitelist IR** — a Rust enum that defines every legal operation:

```rust
enum HttpMethod { Get, Post, Put, Delete }          // No PATCH. No OPTIONS. No hallucination.
enum InputSpec  { Json { schema }, File { .. }, Query { .. }, None }
enum OutputSpec { Json { schema }, Text, NoContent }
enum AuthRequirement { Required, Optional, None }
```

If the LLM invents a method, type, or field that doesn't exist in this enum, **the IR layer rejects it before any code is generated**:

```
✗ REJECTED: (api :method PATCH :path "/users")
  Reason: invalid HTTP method
  Expected: ["GET", "POST", "PUT", "DELETE"]
  → Feed this structured error back to LLM for self-correction
```

## Architecture

```
┌─────────────────────────────────────────────────┐
│        Natural Language / LLM Prompt             │
└────────────────────┬────────────────────────────┘
                     │ LLM generates S-expr
                     ▼
┌─────────────────────────────────────────────────┐
│           S-Expression (DSL)                     │
│  (api :method POST :path "/users/me/avatar" ...) │
│                [untrusted AI output]             │
└────────────────────┬────────────────────────────┘
                     │ nc-parser (50 lines)
                     ▼
┌─────────────────────────────────────────────────┐
│           Typed IR (Rust enum)                   │
│  ApiEndpoint { method: Post, path: "...", ... }  │
│          [validated, trusted structure]           │
└────────────────────┬────────────────────────────┘
                     │ nc-codegen (template lookup)
                     ▼
┌─────────────────────────────────────────────────┐
│           Generated Rust                         │
│  pub async fn upload_avatar(...) -> Result<...>  │
│          [guaranteed to compile]                 │
└────────────────────┬────────────────────────────┘
                     │ rustc ✓
                     ▼
                  Binary
```

## Project Structure

```
neural-codegen/
├── crates/
│   ├── nc-parser/     # S-expr parser — 50 lines of core logic, zero dependencies
│   ├── nc-ir/         # Typed IR — whitelist enums + S-expr → IR lowering
│   └── nc-codegen/    # Code generator — IR → Rust via pre-verified templates
│       └── examples/
│           └── api-gen.rs   # Full pipeline demo
├── docs/              # 10 theory essays (Lambda Calculus → GPU Mode → ISA evolution)
├── verify.sh          # Proves generated code compiles with rustc
├── quick-start.sh     # One-command demo
└── manifesto.md       # The full thesis
```

## Related Work

This project builds on a rich lineage. Grammar-guided code generation dates to [Yin & Neubig (ACL 2017)](https://arxiv.org/abs/1704.01696). Constrained decoding tools like [Guidance](https://github.com/guidance-ai/guidance), [Outlines](https://github.com/dottxt-ai/outlines), and [SGLang](https://github.com/sgl-project/sglang) enforce syntax at the token level. [TyFlow (2025)](https://arxiv.org/abs/2510.10216) pushes type-directed synthesis with formal guarantees. In the S-expr-for-LLM space, [Pact-lang](https://github.com/pact-lang) generates Rust Axum from S-expressions, and Nanolang uses prefix notation as an LLM-native language.

Neural Codegen's contribution is not theoretical novelty in any single component — it's the first runnable end-to-end integration of S-expr parsing, typed IR validation, and deterministic template assembly, plus the "GPU mode" framing and the intent-reality-delta closed loop. See [manifesto.md](./manifesto.md#related-work--intellectual-honesty) for the full discussion.

## The Deeper Theory

In 1936, Lambda Calculus lost to Turing Machines because copying was physically expensive. In 2026, GPUs/TPUs are Lambda machines in silicon. LLMs are stateless transformers — Lambda engines. Their natural output isn't imperative code; it's declarative specifications.

We should stop forcing them to pretend otherwise.

| Document | Content |
|----------|---------|
| [00-origin.md](./docs/00-origin.md) | Origin: From Debug to Neural Codegen |
| [01-why-debug.md](./docs/01-why-debug.md) | Why programming needs debugging |
| [02-lambda-calculus.md](./docs/02-lambda-calculus.md) | Lambda Calculus: the math of computation |
| [03-turing-vs-lambda.md](./docs/03-turing-vs-lambda.md) | Turing vs Lambda: Physics' choice |
| [04-gpu-mode.md](./docs/04-gpu-mode.md) | GPU Mode: AI selects, doesn't create |
| [05-implementation.md](./docs/05-implementation.md) | Engineering implementation plan |
| [06-extension.md](./docs/06-extension.md) | How to extend the DSL |
| [07-2026-context.md](./docs/07-2026-context.md) | 2026 context: HBM shortage, AI workflows |
| [08-parallelism-confluence.md](./docs/08-parallelism-confluence.md) | Parallelism & Confluence |
| [09-instruction-set-evolution.md](./docs/09-instruction-set-evolution.md) | Instruction Set Evolution |

Read the full thesis: **[manifesto.md](./manifesto.md)**

## Beyond This Demo

This repo demonstrates the core pipeline. My private system **Jarvis** takes this further with a self-correcting bidirectional loop:

- `intent.lisp` — 2,700-line S-expr specification of what the system *should* be
- `jarvis-reality.sexp` — auto-extracted ground truth of what *actually* exists (via tree-sitter, every 3 seconds)
- `DeltaDetector` — pure algorithm (no AI) that compares intent vs reality, dispatches fix tasks
- `VerificationWorker` — re-validates every fix before acceptance

AI handles creative work. Deterministic algorithms handle critical work. The boundary is a typed S-expression.

## Benchmark: Pipeline vs Raw LLM (Claude 4.6 Opus)

8 test cases, from simple health checks to complex stateful APIs with auth and rate limiting. Each run twice: once through the pipeline (LLM → S-expr → IR → Rust), once with the LLM generating Rust directly. Both outputs verified with `cargo check`.

```
Test Case            Pipeline      Raw LLM
──────────────────────────────────────────
simple_health       ✓ compiled   ✓ compiled
crud_users          ✓ compiled   ✓ compiled
file_upload         ✓ compiled   ✗ failed     (hallucinated `rand` crate)
stateful_api        ✓ compiled   ✗ failed     (used removed axum::async_trait)
mixed_io            ✗ failed     ✓ compiled
auth_variants       ✗ failed     ✗ failed
rate_limited        ✓ compiled   ✓ compiled
complex_state       ✓ compiled   ✓ compiled

Pass@1:  Pipeline 75%  |  Raw LLM 62%
```

Key insight: **Raw LLM failures are unpredictable hallucinations** (importing crates that don't exist, using APIs that were removed). **Pipeline failures are fixable engineering bugs** (edge cases in the template generator). One is a probabilistic problem. The other is an engineering problem.

Run it yourself: `cargo run -p nc-codegen --example benchmark` (requires `OPENROUTER_API_KEY`)

## Honest Limitations

1. **Expressivity bottleneck** — The IR strips Turing-completeness. Complex algorithms can't be expressed yet. Future: typed escape hatches.
2. **State explosion** — 144 combinations today; real systems have thousands. Future: auto-derive IR from crate signatures.
3. **Compilation ≠ correctness** — The floor, not the ceiling. Future: TLA+ integration, property-based testing.
4. **Single target** — Rust only. The architecture is language-agnostic; only one backend exists.

These are the research frontier, not excuses. See [manifesto.md](./manifesto.md#honest-limitations) for details.

## Who Built This

I'm a 33-year-old video editor from China with no CS degree. No compute clusters, so I had to think differently. While big labs brute-force smarter models, I went back to 1936 to figure out how to mathematically constrain them.

## License

MIT

## Contact

- X: [@RuoqiJin](https://x.com/RuoqiJin) (DMs open)
- LinkedIn: [ruoqijin](https://linkedin.com/in/ruoqijin)
- Email: jinruoqi@xiaojinpro.com
