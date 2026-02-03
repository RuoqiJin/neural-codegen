# Neural Codegen

**让 AI 像 GPU 一样生成代码：一次成型，而非反复斟酌**

## 问题

当前 AI 写 Rust 的模式是"CPU 式"的：

```
Prompt: "实现用户头像上传"
    ↓
AI 写 Rust（猜借用关系、猜类型、猜错误处理）
    ↓
编译失败 → 修 → 编译失败 → 修 → 编译失败 → ...
    ↓
终于跑通（5-10 轮迭代）
```

为什么效率低？因为 Rust 的"正确性"分散在太多地方：

- **类型系统** — 几百种组合可能
- **借用检查** — 生命周期推断
- **错误处理** — Result/Option 链
- **异步** — Pin、Send、Sync trait bounds
- **项目约定** — error 类型、db 抽象、中间件组合

AI 必须同时猜对所有这些，概率很低。每一轮编译失败都是信息损耗。

## 解决方案

**GPU 模式**：AI 只做"选择"，不做"创作"

```
Prompt: "实现用户头像上传"
    ↓
AI 输出结构化 DSL（只选择，不发挥）
    ↓
校验器检查（白名单约束）
    ↓
生成器查表拼装（预验证组件）
    ↓
100% 能编译的 Rust 代码
```

GPU 快不是因为它聪明，而是因为它只执行预定义的着色器指令。

同理，我们的 DSL 让 AI 从"创作 Rust 代码"降级为"填写结构化表单"。

## 核心原理

### 为什么用 S-表达式（Lisp 风格）？

```lisp
;; AI 输出这个
(api :method POST :path "/users/me/avatar"
     :input (file :max-size "5MB" :types ["image/*"])
     :output (json :schema UserAvatar)
     :auth required
     :rate-limit "10/min")
```

**不是因为 Lisp "逻辑严密"，而是因为：**

1. **语法极简** — 只有括号和原子，解析器 50 行代码
2. **AST 显式** — 代码结构就是数据结构，没有语法歧义
3. **AI 几乎不可能写错语法** — 唯一规则是括号匹配

### 真正的严密性来自哪里？

来自你定义的 **白名单 IR**：

```rust
enum ApiSpec {
    Endpoint {
        method: HttpMethod,      // 只能是 GET/POST/PUT/DELETE
        path: String,            // 会校验格式
        input: InputSpec,        // 枚举，不是任意类型
        output: OutputSpec,      // 枚举，不是任意类型
        auth: AuthRequirement,   // Required/Optional/None
        rate_limit: Option<RateLimit>,
    }
}
```

AI 输出的 S-expr 必须能转换成这个 IR。转换失败 = 拒绝，不会生成错误代码。

### 生成器怎么保证正确？

**查表，不创作**：

| DSL 片段 | 生成的 Rust（预验证模板） |
|----------|--------------------------|
| `:auth required` | `#[middleware(RequireAuth)]` |
| `:input (file ...)` | `Form<MultipartUpload>` extractor |
| `:rate-limit "10/min"` | `#[rate_limit(10, Duration::MINUTE)]` |
| `:output (json :schema X)` | `Json<X>` + 自动 derive Serialize |

每个组件都是你预先写好、测试过的。生成器只是把它们拼起来。

## 架构

```
┌─────────────────────────────────────────────────────────────┐
│                    Natural Language                         │
│              "实现用户头像上传，限制 5MB"                      │
└─────────────────────────┬───────────────────────────────────┘
                          │ LLM (constrained output)
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    S-Expression (DSL)                        │
│  (api :method POST :path "/users/me/avatar" ...)            │
│                    [不可信文本]                               │
└─────────────────────────┬───────────────────────────────────┘
                          │ parse + validate
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    Typed IR (Rust enum)                      │
│  ApiSpec::Endpoint { method: POST, path: "...", ... }       │
│                    [可信结构]                                 │
└─────────────────────────┬───────────────────────────────────┘
                          │ lower + expand
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    Core IR (无语法糖)                         │
│  具体的 middleware 组合、extractor 类型、handler 签名         │
└─────────────────────────┬───────────────────────────────────┘
                          │ codegen (template + quote!)
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    Generated Rust                            │
│  pub async fn upload_avatar(...) -> Result<Json<...>>       │
│                    [保证编译通过]                             │
└─────────────────────────┬───────────────────────────────────┘
                          │ rustc
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    Binary                                    │
└─────────────────────────────────────────────────────────────┘
```

## 与传统编译器的区别

| 传统编译器 | Neural Codegen |
|-----------|----------------|
| 输入：人写的代码 | 输入：AI 写的 DSL |
| 目标：翻译 | 目标：约束 + 翻译 |
| 信任输入 | 不信任输入（AI 会幻觉） |
| 报错让人改 | 报错让 AI 重写（自动循环） |

## 适用场景

**适合：**
- API endpoint 生成（CRUD、文件上传、认证流程）
- 数据库 migration 生成
- 配置驱动的工作流
- 声明式、可枚举的任务

**不适合：**
- 复杂业务逻辑（状态机、并发控制）
- 性能敏感的算法实现
- 需要人类创意的代码

## 实现路径

### Phase 1: 最小验证

选一个最常写的模式（比如 REST endpoint），实现：
1. 定义 DSL schema（S-expr 语法）
2. 写 Parser（S-expr → Typed IR）
3. 写 Codegen（IR → Rust 源码）
4. 集成到 Claude Code workflow

**成功标准**：1 prompt → 1 endpoint，编译通过率 > 95%

### Phase 2: 扩展覆盖

- 更多 API 模式（GraphQL、WebSocket）
- 数据层生成（migration、repository）
- 测试生成（单元测试骨架）

### Phase 3: 自动纠错循环

```
AI 生成 DSL
    ↓
Parser 校验失败
    ↓
结构化错误信息 → 喂回 AI
    ↓
AI 修正 → 重新校验
    ↓
循环直到通过（或超过 N 次放弃）
```

## 项目结构

```
neural-codegen/
├── crates/
│   ├── nc-parser/       # S-expr 解析 → Raw AST
│   ├── nc-ir/           # Typed IR 定义 + 校验
│   ├── nc-lower/        # IR 降级 + 宏展开
│   ├── nc-codegen/      # Rust 代码生成
│   └── nc-cli/          # CLI 工具
├── runtime/             # 预验证的 Rust 组件库
│   ├── nc-axum/         # Axum middleware/extractor
│   ├── nc-sqlx/         # SQLx 抽象
│   └── nc-common/       # 通用类型
├── schemas/             # DSL schema 定义
│   ├── api.schema       # API endpoint DSL
│   └── migration.schema # 数据库迁移 DSL
└── examples/
    └── api-gen/         # 示例：生成 API endpoint
```

## 长期愿景

**1 Prompt → 1 App**

不是让 AI 写任意代码，而是：
- 预先定义好所有"积木"（runtime 库）
- AI 只负责"选择和组合"（DSL）
- 生成器保证组合后的正确性

这是从"AI 创作"到"AI 编排"的范式转换。

## 理论文档

本项目的理论基础来自一场深度对话，从"编程为什么要 Debug"出发，逐步深入到计算科学本质。

| 文档 | 内容 |
|------|------|
| [00-origin.md](./docs/00-origin.md) | 起源：从 Debug 到 Neural Codegen |
| [01-why-debug.md](./docs/01-why-debug.md) | 为什么编程需要 Debug |
| [02-lambda-calculus.md](./docs/02-lambda-calculus.md) | Lambda 演算：计算的数学本质 |
| [03-turing-vs-lambda.md](./docs/03-turing-vs-lambda.md) | 图灵机 vs Lambda：物理的选择 |
| [04-gpu-mode.md](./docs/04-gpu-mode.md) | GPU 模式：AI 选择，不创作 |
| [05-implementation.md](./docs/05-implementation.md) | 方案 B：工程实现 |
| [06-extension.md](./docs/06-extension.md) | 如何扩展 DSL |
| [07-2026-context.md](./docs/07-2026-context.md) | 2026 年的现实背景 |

## License

MIT
