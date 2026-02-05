# Instruction Set Evolution: Growing a Smarter DSL

## The Tension

Neural Codegen constrains AI to a finite set of operations. But Rust is infinitely expressive, and that expressiveness is valuable — elegant error handling, zero-cost abstractions, clever trait compositions.

How do we keep the safety of a constrained DSL while capturing the full beauty of Rust?

**The same way CPUs solved it: let the instruction set grow.**

## How CPU Instruction Sets Evolve

```
1978  x86 base          ADD, MOV, JMP
1997  MMX multimedia    PADDB, PMULLW         ← multimedia kept repeating same patterns
2001  SSE vectors       ADDPS, MULPS          ← compress 4 ADDs into 1 instruction
2013  AVX2 wider        VADDPS (256bit)        ← same operation, larger scale
2020  AMX matrices      TDPBF16PS             ← AI workloads became dominant
```

Each generation follows the same cycle:

```
Discover repeated pattern → Encapsulate as instruction → Verify → Add to ISA
```

The base instructions (ADD, MOV) never change. New instructions build on top.

## Neural Codegen's Growth Cycle

The same cycle, applied to code generation:

```
You write an elegant Rust implementation
    ↓
You realize this pattern will recur
    ↓
Encapsulate as a runtime component (pre-verified)
    ↓
Define the corresponding DSL operation
    ↓
Write tests to validate it
    ↓
AI can now invoke your craftsmanship with one line of DSL
    ↓
Your Rust taste becomes accumulated instruction set
```

**Every line of elegant Rust you write makes the instruction set smarter.**

## Three-Layer Instruction Set

### Layer 1 — Primitives (atomic operations, rarely change)

```lisp
(handler :fn-name "get_user" :input (path :param "id") :output (json :schema User))
(query :sql "SELECT * FROM users WHERE id = ?" :params [id])
(type :name "User" :fields [(:id i64) (:name String) (:email String)])
```

These are the ADD, MOV, JMP of your system. Boring but foundational.

### Layer 2 — Structural (common patterns, occasionally new)

```lisp
(api :method POST :path "/users" :input (json :schema CreateUser) :auth required)
(migration :table "users" :add-column (:avatar_url String :nullable true))
(middleware :chain [auth rate-limit cors])
```

These combine Layer 1 primitives into common architectural patterns.

### Layer 3 — Domain (your craftsmanship, continuously growing)

```lisp
(oauth2-flow :provider github)
(rate-limiter :algo sliding-window :limit 100 :window "1m")
(resilient-http :retry 3 :backoff exponential :circuit-breaker true)
(realtime-channel :protocol ws :auth jwt :heartbeat "30s")
(credit-transaction :user $uid :amount -10 :idempotency-key $key)
```

**One line of Layer 3 DSL = 100-500 lines of carefully crafted Rust.**

Each Layer 3 instruction is a crystallization of your deep understanding of a specific domain problem.

## Instruction Composition: Macro Expansion

Layer 3 instructions expand into Layer 2, which expand into Layer 1:

```lisp
;; Define a new domain instruction
(define-instruction oauth2-flow
  :version 1
  :params (
    :provider (enum [github google apple wechat])
    :scopes   (list string :default ["openid" "profile" "email"])
  )
  :expands-to (
    ;; Layer 2: three API endpoints
    (api :method GET :path "/oauth2/authorize"
         :input (query :params [provider scopes redirect_uri state])
         :output (redirect :to (oauth2-authorize-url $provider $scopes))
         :auth none)

    (api :method GET :path "/oauth2/callback"
         :input (query :params [code state])
         :handler (oauth2-exchange $provider)
         :auth none)

    (api :method POST :path "/oauth2/token"
         :input (json :schema TokenRequest)
         :output (json :schema TokenResponse)
         :handler (oauth2-token-issue)
         :auth none)

    ;; Layer 2: database
    (migration :table "oauth_connections"
               :columns [(:user_id i64)
                         (:provider String)
                         (:provider_uid String)
                         (:access_token String :encrypted true)])
  ))
```

The lowering pipeline:

```
Layer 3:  (oauth2-flow :provider github)
    ↓ expand
Layer 2:  3 API endpoints + 1 migration
    ↓ expand
Layer 1:  handlers, queries, types, middleware chains
    ↓ codegen
Rust:     Complete OAuth2 implementation, guaranteed to compile
```

## The Role of Your Rust Passion

In this architecture, you are the **instruction set architect**:

| What you do | What the system gains |
|-------------|----------------------|
| Write an elegant retry mechanism with exponential backoff | A new `resilient-http` runtime component |
| Design a clever credit transaction with idempotency | A new `credit-transaction` instruction |
| Craft a WebSocket handler with heartbeat and reconnection | A new `realtime-channel` instruction |
| Implement a sliding window rate limiter | A new `rate-limiter` instruction |
| **Each piece of Rust craftsmanship** | **A permanent addition to the ISA** |

You're not giving up Rust. You're **distilling** Rust.

The difference between a regular programmer and an ISA architect:
- Regular programmer writes code that runs once
- ISA architect writes instructions that generate correct code forever

## Instruction Lifecycle

### Stage 1: Inline Rust (prototype)

You write Rust directly, iterating until it's perfect:

```rust
// You spend 2 days getting this exactly right
pub async fn resilient_request(url: &str, config: RetryConfig) -> Result<Response> {
    let mut attempts = 0;
    let mut delay = config.initial_delay;
    loop {
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => return Ok(resp),
            Ok(resp) if resp.status().is_server_error() && attempts < config.max_retries => {
                attempts += 1;
                tokio::time::sleep(delay).await;
                delay = std::cmp::min(delay * 2, config.max_delay);
            }
            Ok(resp) => return Err(Error::HttpStatus(resp.status())),
            Err(e) if attempts < config.max_retries => {
                attempts += 1;
                tokio::time::sleep(delay).await;
                delay = std::cmp::min(delay * 2, config.max_delay);
            }
            Err(e) => return Err(e.into()),
        }
    }
}
```

### Stage 2: Runtime Component (encapsulate)

Extract it into the runtime library with a clean interface:

```rust
// runtime/nc-http/src/resilient.rs
pub struct ResilientHttp {
    max_retries: u32,
    backoff: BackoffStrategy,
    circuit_breaker: Option<CircuitBreaker>,
}

impl ResilientHttp {
    pub fn from_spec(spec: &ResilientHttpSpec) -> Self { ... }
    pub async fn execute(&self, req: Request) -> Result<Response> { ... }
}
```

Test exhaustively. This component is now **pre-verified**.

### Stage 3: DSL Operation (expose to AI)

Define the DSL surface:

```lisp
(resilient-http
  :retry 3
  :backoff exponential
  :circuit-breaker true
  :timeout "30s")
```

Add IR types:

```rust
pub struct ResilientHttpSpec {
    pub max_retries: u32,
    pub backoff: BackoffStrategy,
    pub circuit_breaker: bool,
    pub timeout: Duration,
}
```

Add parser rules. Add codegen rules. Update AI prompt.

### Stage 4: AI Uses It (the payoff)

From now on, AI generates this one line instead of 50 lines of Rust.

```
User: "Add an HTTP call to the payment API with retry and circuit breaker"
AI: (resilient-http :retry 3 :backoff exponential :circuit-breaker true)
Generator: 50 lines of your battle-tested Rust
```

**Your 2 days of craftsmanship now saves 30 minutes every time this pattern recurs.**

## Instruction Set Growth Strategy

### What to promote to an instruction

Not every piece of code should become an instruction. Criteria:

| Signal | Promote? | Example |
|--------|----------|---------|
| Used 3+ times across projects | Yes | Auth middleware, CRUD patterns |
| Complex but well-understood | Yes | OAuth2 flow, rate limiting |
| Performance-critical with tricky edge cases | Yes | Connection pooling, retry logic |
| One-off business logic | No | "Calculate user's birthday discount" |
| Rapidly changing requirements | No | Experimental features |
| Simple enough AI gets it right every time | No | Basic string formatting |

### Instruction set size management

Like RISC vs CISC, there's a trade-off:

```
Too few instructions:
  → AI has to compose many Layer 1 ops → more room for error
  → You're not leveraging your Rust expertise

Too many instructions:
  → AI has too many choices → selection accuracy drops
  → Maintenance burden grows
  → Instructions overlap → ambiguity

Sweet spot:
  → Layer 1: ~10-15 primitives (stable)
  → Layer 2: ~20-30 structural patterns (slow growth)
  → Layer 3: ~50-100 domain instructions (active growth)
  → Total: under 150 instructions for a full-stack web application
```

### Versioning & deprecation

```lisp
;; Instruction versioning
(define-instruction resilient-http
  :version 2                          ; bumped from v1
  :deprecated-params [:retry-count]   ; old name, still accepted
  :params (
    :retry     (int :default 3)       ; new canonical name
    :backoff   (enum [linear exponential] :default exponential)
    :circuit-breaker (bool :default false)
    :timeout   (duration :default "30s")  ; new in v2
  ))
```

Old DSL using `:retry-count` still works — the normalize pass maps it to `:retry`.

## Connection to CPU ISA Design Principles

| CPU ISA Principle | Neural Codegen Application |
|-------------------|---------------------------|
| **Orthogonality**: instructions don't overlap | Each DSL operation has a unique purpose |
| **Regularity**: consistent encoding format | All operations follow `(op :key value)` syntax |
| **Backward compatibility**: old code still runs | Old DSL still parses via normalize pass |
| **Extension mechanism**: reserved opcodes | `CustomOp` for experimental instructions |
| **Privilege levels**: user/kernel mode | Layer separation (AI generates Layer 2-3, only you define new instructions) |

The most important parallel:

> **In CPU design, the ISA is the contract between hardware and software.**
> **In Neural Codegen, the DSL is the contract between AI and the code generator.**

Both must be stable, well-defined, and extensible without breaking existing users.

## Summary

Your love for Rust doesn't conflict with the DSL approach — it **powers** it.

```
Rust Craftsman                    ISA Architect
─────────────────                 ─────────────────
Writes beautiful code             Designs powerful instructions
Code runs once                    Instructions generate code forever
Expertise is in the code          Expertise is in the instruction set
Impact: one project               Impact: every future project
```

The instruction set is your **compound interest**. Every elegant pattern you extract today accelerates every project tomorrow.
