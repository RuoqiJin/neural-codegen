# Parallelism & Confluence: Lessons from Hardware Lambda

This document extracts insights from the 2026 paper "CPU-less parallel execution of lambda calculus in digital logic" and applies them to Neural Codegen's design.

## The Paper's Core Idea

The paper (arXiv:2601.13040) proposes executing Lambda calculus directly in digital logic, without a CPU:

- Each sub-expression becomes a **hardware node**
- Nodes communicate via **buses** (Expression Bus, Instruction Bus)
- β-reduction happens **in parallel** across multiple nodes
- No central CPU doing fetch-decode-execute

### Key Result: Parallel Reduction

The paper demonstrates that more complex expressions can complete in the same number of clock cycles as simpler ones, because transformations happen simultaneously:

| Expression | Transformations | Clock Cycles |
|------------|-----------------|--------------|
| `(λx.x) y → y` | 1 substitution | 8 |
| `(λx.xx) y → yy` | 2 substitutions | 8 |

The second expression does **twice the work** but takes the **same time** — because the substitutions happen in parallel.

## Interaction Nets: A More Parallel Model

The paper's discussion leads to **Interaction Nets** / **Interaction Combinators**, which have two key properties:

### 1. Locality

Every rewrite rule is **local**: it only involves a fixed number of nodes in the immediate neighborhood. No global state, no distant dependencies.

```
Traditional λ:     Need to check free variables, scoping, etc.
Interaction Nets:  Just match the local pattern, rewrite, done.
```

### 2. Strong Confluence (One-Step Diamond)

If two rewrite rules can both apply to a graph, they can be applied in **either order** (or simultaneously) and reach the same result.

```
     G
    / \
   A   B     (two possible rewrites)
    \ /
     H       (same final result, regardless of order)
```

This means: **no synchronization needed**. Parallel execution is inherently safe.

## How This Applies to Neural Codegen

Neural Codegen doesn't execute Lambda on hardware, but the **design principles** transfer directly.

### Principle 1: Local Steps

Each DSL step should be **self-contained**:

```lisp
;; Good: each step is independent
(pipeline
  (api :method GET :path "/health")           ; step 1
  (api :method POST :path "/users")           ; step 2
  (api :method DELETE :path "/users/:id"))    ; step 3
```

Step 1's validation doesn't depend on step 2's result. Step 2's code generation doesn't need to wait for step 1's code generation.

```lisp
;; Bad: steps have implicit dependencies
(pipeline
  (define :var "base" :value "/api/v1")       ; creates state
  (api :path (concat $base "/users")))        ; depends on that state
```

This introduces ordering constraints and breaks parallelism.

### Principle 2: Deterministic Output (Confluence)

No matter how the system processes the DSL:
- Validate step 1 first, or step 3 first
- Generate code for step 2 before step 1
- Process in parallel or sequentially

**The final output must be identical.**

This is crucial for:
- **Reproducibility**: Same input → same output, always
- **Debugging**: Can re-run any step in isolation
- **Caching**: Intermediate results can be cached and reused

### Principle 3: Parallel Validation

Because steps are local and the system is confluent:

```
Traditional (sequential):
  validate(step1) → validate(step2) → validate(step3) → generate_all

Neural Codegen (parallel):
  ┌─ validate(step1) ─┐
  ├─ validate(step2) ─┼─→ merge errors → generate_all (if no errors)
  └─ validate(step3) ─┘
```

If step 2 has an error, we don't need to wait for step 1 and 3 to finish validating — we can report all errors at once.

### Principle 4: Parallel Code Generation

Similarly, code generation can be parallelized:

```
┌─ codegen(step1) → code_fragment_1 ─┐
├─ codegen(step2) → code_fragment_2 ─┼─→ assemble → final Rust code
└─ codegen(step3) → code_fragment_3 ─┘
```

The assembly step is simple concatenation (with ordering), not complex merging.

## Design Implications

### IR Design

The Typed IR should enforce locality:

```rust
pub enum Step {
    Api(ApiSpec),      // self-contained
    Migration(MigrationSpec),  // self-contained
    // ...
}

// Each step has all the information it needs
pub struct ApiSpec {
    method: HttpMethod,
    path: String,
    input: InputSpec,
    output: OutputSpec,
    auth: AuthRequirement,
    // No references to other steps!
}
```

### No Cross-Step References

Avoid designs like:

```rust
// Bad: step references another step
pub struct ApiSpec {
    depends_on: Option<StepId>,  // creates ordering dependency
}
```

If you need shared configuration, lift it to the pipeline level:

```lisp
(pipeline
  :base-path "/api/v1"           ; shared config at pipeline level
  :default-auth required
  (api :method GET :path "/health")   ; inherits from pipeline
  (api :method POST :path "/users"))  ; inherits from pipeline
```

The lowering pass resolves these into fully self-contained steps.

### Error Aggregation

Because validation is parallel, errors should be collected, not thrown:

```rust
fn validate_pipeline(steps: &[Step]) -> Vec<ValidationError> {
    steps
        .par_iter()  // parallel iteration (rayon)
        .flat_map(|step| validate_step(step))
        .collect()
}
```

Return all errors at once, so AI can fix them in one round.

## Connection to GPU Mode

This reinforces the "GPU Mode" concept from [04-gpu-mode.md](./04-gpu-mode.md):

| GPU Shader | Neural Codegen DSL |
|------------|-------------------|
| Each pixel processed independently | Each step processed independently |
| No cross-pixel dependencies | No cross-step dependencies |
| Massively parallel execution | Parallel validation & generation |
| Deterministic output | Confluent output |

The DSL is like a shader program: you describe **what** each element should become, not **how** to process them in order.

## Further Reading

### The 2026 Paper

- **Title**: CPU-less parallel execution of lambda calculus in digital logic
- **arXiv**: [2601.13040](https://arxiv.org/abs/2601.13040)
- **GitHub**: [LAMB-TARK/CPU-less-parallel-execution-of-Lambda-calculus-in-digital-logic](https://github.com/LAMB-TARK/CPU-less-parallel-execution-of-Lambda-calculus-in-digital-logic)

### Interaction Nets & HVM

- **Interaction Nets**: [Wikipedia](https://en.wikipedia.org/wiki/Interaction_nets)
- **HVM2**: [HigherOrderCO/HVM](https://github.com/HigherOrderCO/HVM) — A massively parallel Interaction Combinator evaluator

### Hardware Graph Reduction (Related Work)

- **Heron**: Modern FPGA graph reduction processor
- **KappaMutor**: Single-cycle reduction with structured combinators
- **Cloaca**: Hardware-level concurrent garbage collection

These projects demonstrate that the "locality + confluence = parallelism" principle works in practice, from software runtimes to FPGA implementations.
