# 方案 B：工程实现

## 核心原则

**方案 B 的关键不在"转成 Rust"，而在"只生成安全且可控的 Rust"。**

避免两种极端：

1. **让 AI 输出 Rust** → 语法/依赖/unsafe/注入全是坑
2. **让 AI 输出能任意扩展的 Lisp** → 宏太强，AI 一旦发散会反噬

方案 B 的秘诀：

1. AI 只输出"数据结构化的 AST 序列化"（S-expr）
2. 后端把它解析成 **强类型 IR**（你自己的 enum/struct）
3. 所有宏展开/默认值填充/资源解析/优化都在 IR 上做
4. Rust 代码生成只是一层"打印机"，而不是"编译器核心"

## 系统分层

把整个链路固定为可观测的 pipeline：

```
Natural Language
   ↓ (LLM, constrained output)
S-Expr (AI-Lisp Layer; untrusted text)
   ↓ parse
Raw AST (lexpr::Value)
   ↓ convert + typecheck
Typed IR (trusted, your structs/enums)
   ↓ lowering / macro expansion / resource resolution
Core IR (更接近执行层、无语法糖、无歧义)
   ↓ codegen (Rust)
Rust source (generated.rs + template crate)
   ↓ build (cargo/rustc)
Binary artifact
```

**每一层都要能 dump、能复现、能写 golden test。**

这是后面 Debug 与"自动递归修复"的地基。

## 模板 Crate 设计

### 问题

不要每次生成一个完整工程，也不要每次把业务逻辑都重新生成一遍。

### 解决方案

维护一个稳定的 Rust 工程（比如 `xjp_app_template/`）：

```
xjp_app_template/
├── Cargo.toml
├── src/
│   ├── main.rs          # 固定不变，调用 generated::run()
│   ├── lib.rs           # 固定不变
│   └── generated.rs     # 每次生成的极薄代码
└── runtime/
    └── xjp_runtime/     # 真正的业务实现
        ├── extractors/  # MultipartUpload 等
        ├── middleware/  # RequireAuth 等
        └── handlers/    # 通用 handler 模板
```

**好处**：

- Rust 编译器的增量编译能最大化命中
- runtime 只编一次，每次只编译小小的 generated.rs
- 编译速度从分钟级降到秒级

## S-expression 解析

### 原则

把 S-expr 当"序列化格式"，不要当"语言"。

### 实现

用成熟的 S-exp 解析库（如 `lexpr`）把字符串解析成 `Value`，然后做严格的"Value → Typed IR"转换。

```rust
use lexpr::Value;

fn parse_api_spec(value: &Value) -> Result<ApiSpec, ParseError> {
    let list = value.as_list().ok_or(ParseError::ExpectedList)?;

    // 第一个元素必须是 'api'
    let op = list[0].as_symbol().ok_or(ParseError::ExpectedSymbol)?;
    if op != "api" {
        return Err(ParseError::UnknownOp(op.to_string()));
    }

    // 解析关键字参数
    let method = get_keyword(list, ":method")?
        .parse::<HttpMethod>()?;
    let path = get_keyword(list, ":path")?;
    // ...

    Ok(ApiSpec { method, path, ... })
}
```

### 校验规则

这个转换阶段要做到：

- **不认识的 form/keyword 直接报错**（不要默认忽略）
- **参数类型不对直接报错**（不要"尽量转换"）
- **强制必须字段、默认字段、互斥字段都在这里处理**

这一步就是"AI 不是写程序，是填表"。真正的"表结构"就在 Typed IR 上，而不是 prompt 上。

## IR 设计

### 强类型 + 可定位错误

```rust
pub enum Step {
    Source { path: String },
    Slice { start: f64, end: f64 },
    FilterLut { name: String, lut_path: PathBuf, strength: f32 },
    Export { format: String, out_path: PathBuf },
}

pub struct Pipeline {
    pub steps: Vec<Step>,
}

// 错误类型，带上出错位置
pub struct ParseError {
    pub path: String,           // e.g. "pipeline.steps[2].filter.name"
    pub expected: String,       // e.g. "string"
    pub actual: String,         // e.g. "number"
    pub suggestion: Option<String>, // 给 LLM 的修复建议
}
```

这会直接决定后面自动纠错循环的成功率。

## Lowering Passes

把高层语法糖变成"核心 IR"。做成多 pass 的形式：

### 1. Normalize Pass

补默认值、规范化参数名、把多种同义写法变成一种。

```
输入: (filter :name "Vintage")
输出: (filter :name "Vintage" :strength 0.8)  // 补上默认 strength
```

### 2. Resolve Pass

查数据库/文件系统，把名字解析成路径/ID。

```
输入: (filter :name "Vintage")
输出: FilterLut {
    name: "Vintage",
    lut_path: "/assets/lut/vintage.cube",  // 解析出实际路径
    strength: 0.8
}
```

### 3. Validate Pass

跨 step 的一致性校验。

```
- source 必须在第一步
- export 必须最后
- slice 必须在 source 之后
- filter 不能应用于空 pipeline
```

### 4. Optimize Pass（可选）

合并相邻切片、合并滤镜、推导输出格式等。

**每个 pass 输入输出都是 IR，且可独立测试。**

## Rust 代码生成

### 两种路线

**路线 1（更稳）：生成 Rust AST → pretty print**

```rust
use quote::quote;

fn codegen_step(step: &Step) -> TokenStream {
    match step {
        Step::FilterLut { name, lut_path, strength } => {
            quote! {
                pipeline.add_op(xjp_runtime::ops::FilterLut::new(
                    #name,
                    include_bytes!(#lut_path),
                    #strength,
                ));
            }
        }
        // ...
    }
}
```

- 优点：不会产生括号/逗号/转义错误
- 缺点：要写 AST 构造代码

**路线 2（更快上手）：模板 + 严格转义**

- main.rs 固定模板
- 生成部分只填几个结构体字面量
- 对所有字符串做 Rust 字符串字面量转义

### 推荐：生成配置文件

最安全的方式：生成 JSON/RON 配置，Rust 代码只负责加载和执行。

```rust
// generated.rs
pub const PIPELINE_CONFIG: &str = include_str!("pipeline.json");

pub fn run() -> Result<()> {
    let pipeline: Pipeline = serde_json::from_str(PIPELINE_CONFIG)?;
    xjp_runtime::execute(pipeline)
}
```

这样：

- 安全性大幅提升（没有代码注入风险）
- Rust 编译更稳定（代码结构不变，变化的是配置）
- 仍然得到独立二进制

## 构建与交付

### 构建命令

推荐使用 cargo：

```bash
cargo build --release --locked
```

关键优化：

- 固定 toolchain 版本（rust-toolchain.toml）
- 使用共享 target/ 目录 + 增量编译
- 启用 sccache
- 限流/队列（编译是 CPU + IO 重活）

### 交付形态

考虑因素：

- 目标平台：Linux/macOS/Windows？
- 静态/动态链接：多媒体依赖通常很大
- 许可证合规：如 ffmpeg 的分发限制

## 主要阻滞点与对策

### 阻滞 1：AI 输出"看似合法但语义错"的 S-expr

**症状**：语法没错、括号也对，但参数缺失/错位/字段名拼错

**对策**：
- S-expr 只是表面，真正的约束在 IR 构造器里
- 错误要结构化：`expected: number, got: "ten"`
- 对 LLM 的纠错循环：把错误信息变成"最小修复提示"

### 阻滞 2：Rust 编译速度

**症状**：每次几十秒甚至几分钟

**对策**：
- 模板 crate + generated.rs 小改动（增量编译）
- runtime 逻辑放库 crate，减少变化面
- sccache
- 限流/队列

### 阻滞 3：依赖与环境

**症状**：你机器能编，用户机器不能跑

**对策**：
- 明确是否"真正独立单文件"
- 容器化构建环境
- 对外分发：打包动态库/资源

### 阻滞 4：安全

**症状**：有人构造输入让你生成恶意代码

**对策**：
- AI 输出只是不可信文本：必须走白名单 IR
- 生成 Rust 时禁止任何"原样拼接可执行代码"
- 编译环境沙箱化

### 阻滞 5：可调试性

**症状**：用户说"结果不对"，难以定位

**对策**：
永远保留并可导出这些工件：
- 原始 NL
- AI 生成 S-expr
- Typed IR（JSON dump）
- Core IR
- 生成的 Rust 源码
- 构建日志

### 阻滞 6：DSL 演进

**症状**：加了新字段，旧 prompt/旧模型输出全坏

**对策**：
- DSL 版本号：`(pipeline :dsl-version 3 ...)`
- IR 层做向后兼容
- 模型侧也版本化 prompt

## 落地 Checklist

如果明天要开始写，按这个顺序做：

1. [ ] **定义 DSL + Typed IR**：把允许的 form 列成白名单
2. [ ] **写 Parser**：S-expr → Raw AST → Typed IR（严格类型检查）
3. [ ] **写 Lowering Pass**：把高层 pipeline 变成 core steps
4. [ ] **写 Runtime Crate**：`execute(core_ir) -> Result<()>`
5. [ ] **写 Codegen**：生成极薄的 generated.rs
6. [ ] **写 Builder**：管理 temp workspace、调用 cargo
7. [ ] **加入 LLM 纠错循环**：只把结构化错误喂回模型
8. [ ] **加可观测性**：dump 每层 IR + build log

做到第 5 步就已经"make it happen"了。

第 6-8 步决定能不能上线、能不能抗并发、能不能长期维护。
