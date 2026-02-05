# Neural Codegen

**Make AI generate code like a GPU: one-shot completion, not iterative refinement**

## The Problem

Current AI-assisted Rust development follows a "CPU-style" pattern:

```
Prompt: "Implement user avatar upload"
    ↓
AI writes Rust (guessing borrow relationships, types, error handling)
    ↓
Compile error → fix → compile error → fix → compile error → ...
    ↓
Finally works (after 5-10 iterations)
```

Why is this inefficient? Because Rust's "correctness" is scattered across too many dimensions:

- **Type system** — hundreds of possible combinations
- **Borrow checker** — lifetime inference
- **Error handling** — Result/Option chains
- **Async** — Pin, Send, Sync trait bounds
- **Project conventions** — error types, db abstractions, middleware composition

AI must guess all of these correctly at once. The probability is low. Each compilation failure is information loss.

## The Solution

**GPU Mode**: AI only "selects", never "creates"

```
Prompt: "Implement user avatar upload"
    ↓
AI outputs structured DSL (selection only, no improvisation)
    ↓
Validator checks (whitelist constraints)
    ↓
Generator assembles via lookup (pre-verified components)
    ↓
100% compilable Rust code
```

GPUs are fast not because they're smart, but because they only execute predefined shader instructions.

Similarly, our DSL downgrades AI from "creating Rust code" to "filling out a structured form".

## Core Principles

### Why S-expressions (Lisp-style)?

```lisp
;; AI outputs this
(api :method POST :path "/users/me/avatar"
     :input (file :max-size "5MB" :types ["image/*"])
     :output (json :schema UserAvatar)
     :auth required
     :rate-limit "10/min")
```

**Not because Lisp is "logically rigorous", but because:**

1. **Minimal syntax** — only parentheses and atoms, parser in 50 lines
2. **Explicit AST** — code structure IS data structure, no syntactic ambiguity
3. **AI almost can't write syntax errors** — the only rule is bracket matching

### Where does the real rigor come from?

From your **whitelist IR**:

```rust
enum ApiSpec {
    Endpoint {
        method: HttpMethod,      // can only be GET/POST/PUT/DELETE
        path: String,            // format validated
        input: InputSpec,        // enum, not arbitrary type
        output: OutputSpec,      // enum, not arbitrary type
        auth: AuthRequirement,   // Required/Optional/None
        rate_limit: Option<RateLimit>,
    }
}
```

AI's S-expr output must convert to this IR. Conversion failure = rejection, no incorrect code generated.

### How does the generator guarantee correctness?

**Lookup, not creation**:

| DSL Fragment | Generated Rust (pre-verified template) |
|--------------|----------------------------------------|
| `:auth required` | `#[middleware(RequireAuth)]` |
| `:input (file ...)` | `Form<MultipartUpload>` extractor |
| `:rate-limit "10/min"` | `#[rate_limit(10, Duration::MINUTE)]` |
| `:output (json :schema X)` | `Json<X>` + auto derive Serialize |

Every component is pre-written and tested by you. The generator just assembles them.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Natural Language                          │
│              "Implement avatar upload, limit 5MB"            │
└─────────────────────────┬───────────────────────────────────┘
                          │ LLM (constrained output)
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    S-Expression (DSL)                        │
│  (api :method POST :path "/users/me/avatar" ...)            │
│                    [untrusted text]                          │
└─────────────────────────┬───────────────────────────────────┘
                          │ parse + validate
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    Typed IR (Rust enum)                      │
│  ApiSpec::Endpoint { method: POST, path: "...", ... }       │
│                    [trusted structure]                       │
└─────────────────────────┬───────────────────────────────────┘
                          │ lower + expand
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    Core IR (no syntax sugar)                 │
│  Concrete middleware composition, extractor types, handler   │
└─────────────────────────┬───────────────────────────────────┘
                          │ codegen (template + quote!)
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    Generated Rust                            │
│  pub async fn upload_avatar(...) -> Result<Json<...>>       │
│                    [guaranteed to compile]                   │
└─────────────────────────┬───────────────────────────────────┘
                          │ rustc
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    Binary                                    │
└─────────────────────────────────────────────────────────────┘
```

## Comparison with Traditional Compilers

| Traditional Compiler | Neural Codegen |
|---------------------|----------------|
| Input: human-written code | Input: AI-written DSL |
| Goal: translate | Goal: constrain + translate |
| Trusts input | Distrusts input (AI hallucinates) |
| Errors for humans to fix | Errors for AI to retry (auto-loop) |

## Use Cases

**Suitable for:**
- API endpoint generation (CRUD, file upload, auth flows)
- Database migration generation
- Configuration-driven workflows
- Declarative, enumerable tasks

**Not suitable for:**
- Complex business logic (state machines, concurrency control)
- Performance-critical algorithm implementation
- Code requiring human creativity

## Implementation Roadmap

### Phase 1: Minimal Validation

Pick your most common pattern (e.g., REST endpoint), implement:
1. Define DSL schema (S-expr syntax)
2. Write Parser (S-expr → Typed IR)
3. Write Codegen (IR → Rust source)
4. Integrate into Claude Code workflow

**Success criteria**: 1 prompt → 1 endpoint, compile success rate > 95%

### Phase 2: Expand Coverage

- More API patterns (GraphQL, WebSocket)
- Data layer generation (migration, repository)
- Test generation (unit test skeletons)

### Phase 3: Auto-correction Loop

```
AI generates DSL
    ↓
Parser validation fails
    ↓
Structured error message → feed back to AI
    ↓
AI corrects → re-validate
    ↓
Loop until pass (or give up after N attempts)
```

## Project Structure

```
neural-codegen/
├── crates/
│   ├── nc-parser/       # S-expr parsing → Raw AST
│   ├── nc-ir/           # Typed IR definition + validation
│   ├── nc-lower/        # IR lowering + macro expansion
│   ├── nc-codegen/      # Rust code generation
│   └── nc-cli/          # CLI tool
├── runtime/             # Pre-verified Rust component library
│   ├── nc-axum/         # Axum middleware/extractor
│   ├── nc-sqlx/         # SQLx abstractions
│   └── nc-common/       # Common types
├── schemas/             # DSL schema definitions
│   ├── api.schema       # API endpoint DSL
│   └── migration.schema # Database migration DSL
└── examples/
    └── api-gen/         # Example: generate API endpoint
```

## Long-term Vision

**1 Prompt → 1 App**

Not letting AI write arbitrary code, but:
- Pre-define all "building blocks" (runtime library)
- AI only "selects and composes" (DSL)
- Generator guarantees correctness of composition

This is a paradigm shift from "AI creates" to "AI orchestrates".

## Theory Documentation

The theoretical foundation of this project comes from an in-depth conversation, starting from "why does programming need debugging" and gradually diving into the essence of computation science.

| Document | Content |
|----------|---------|
| [00-origin.md](./docs/00-origin.md) | Origin: From Debug to Neural Codegen |
| [01-why-debug.md](./docs/01-why-debug.md) | Why programming needs debugging |
| [02-lambda-calculus.md](./docs/02-lambda-calculus.md) | Lambda Calculus: The mathematical essence of computation |
| [03-turing-vs-lambda.md](./docs/03-turing-vs-lambda.md) | Turing vs Lambda: Physics' choice |
| [04-gpu-mode.md](./docs/04-gpu-mode.md) | GPU Mode: AI selects, doesn't create |
| [05-implementation.md](./docs/05-implementation.md) | Plan B: Engineering implementation |
| [06-extension.md](./docs/06-extension.md) | How to extend the DSL |
| [07-2026-context.md](./docs/07-2026-context.md) | 2026 context: HBM shortage, AI workflows |
| [08-parallelism-confluence.md](./docs/08-parallelism-confluence.md) | Parallelism & Confluence: Lessons from hardware Lambda |
| [09-instruction-set-evolution.md](./docs/09-instruction-set-evolution.md) | Instruction Set Evolution: Growing a smarter DSL |

## License

MIT
