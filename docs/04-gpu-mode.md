# GPU 模式：AI 选择，不创作

## CPU 模式 vs GPU 模式

### CPU 模式（当前 AI 编程的低效模式）

```
你："实现用户头像上传"
    ↓
AI 开始"创作" Rust 代码
    ↓
猜借用关系（可能猜错）
猜类型组合（可能猜错）
猜错误处理（可能猜错）
猜项目约定（可能猜错）
    ↓
编译失败
    ↓
AI 看报错，尝试修复
    ↓
编译失败
    ↓
循环 5-10 次
    ↓
终于跑通（或放弃）
```

**问题**：AI 必须同时猜对所有维度，概率很低。

### GPU 模式（Neural Codegen 的目标模式）

```
你："实现用户头像上传"
    ↓
AI 输出结构化 DSL（只选择，不发挥）
    ↓
(api :method POST :path "/users/me/avatar"
     :input (file :max-size "5MB" :types ["image/*"])
     :output (json :schema UserAvatar)
     :auth required)
    ↓
校验器检查（白名单约束）
    ↓
生成器查表拼装（预验证组件）
    ↓
100% 能编译的 Rust 代码
```

**核心转变**：从"创作"到"选择"。

## 为什么叫"GPU 模式"？

GPU 快不是因为它聪明，而是因为它 **只执行预定义的着色器指令**。

| CPU | GPU |
|-----|-----|
| 通用计算 | 专用计算 |
| 可以执行任意指令序列 | 只能执行预定义的着色器 |
| 灵活但低效 | 受限但高效 |
| 每个像素的处理逻辑可以不同 | 每个像素执行相同的函数 |

**GPU 着色器的本质**：
```glsl
vec4 fragmentShader(vec2 uv) {
    // 输入：像素坐标
    // 输出：颜色
    // 没有循环，没有状态，只有纯函数变换
    return texture(sampler, uv) * color;
}
```

**Neural Codegen 的 DSL 也是如此**：
```lisp
(api :method POST :path "/users/me/avatar" ...)
;; 输入：结构化描述
;; 输出：Rust 代码
;; 没有自由发挥，只有预定义的组合
```

## AI 的本质特性

### 擅长的

- 模式识别（从自然语言提取意图）
- 分类（选择正确的选项）
- 生成结构化输出（JSON、S-表达式）
- 遵循模板

### 不擅长的

- 精确计算（数学运算容易出错）
- 状态跟踪（容易忘记上下文）
- 类型推断（Rust 的借用检查太复杂）
- 一致性（同一个问题可能给出不同答案）

### 结论

**让 AI 做它擅长的事**：从自然语言到结构化选择

**让编译器做它擅长的事**：从结构化选择到正确代码

## 白名单约束的威力

### 没有白名单（任意代码生成）

AI 的输出空间是无限的：
```
可能的 Rust 代码 = ∞
正确的 Rust 代码 = 极小子集
AI 命中正确代码的概率 ≈ 0
```

### 有白名单（DSL 约束）

AI 的输出空间是有限的：
```
可能的 DSL 组合 = N（有限）
正确的 DSL 组合 = N（全部都正确，因为预验证）
AI 命中正确代码的概率 = 100%
```

### 白名单的实现

```rust
// 只允许这些 HTTP 方法
enum HttpMethod { GET, POST, PUT, DELETE }

// 只允许这些输入类型
enum InputSpec {
    Json { schema: String },
    Form { fields: Vec<Field> },
    File { max_size: String, types: Vec<String> },
    Query { params: Vec<Param> },
}

// 只允许这些认证方式
enum AuthRequirement { Required, Optional, None }
```

AI 不能发明新的 HTTP 方法，不能发明新的输入类型。它只能从预定义的选项中选择。

## 查表而非创作

### 传统方式（AI 创作代码）

```
AI 需要知道：
- Axum 的 extractor 语法
- multipart form 的处理方式
- 错误类型的定义
- 中间件的应用顺序
- Response 的序列化方式
- ...

任何一个环节出错 = 编译失败
```

### GPU 模式（AI 选择，生成器查表）

```
AI 只需要选择：
- method: POST ✓
- input: file ✓
- auth: required ✓

生成器查表：
- POST → axum::routing::post
- file → Form<MultipartUpload>（你预先写好的 extractor）
- auth required → #[middleware(RequireAuth)]（你预先写好的中间件）

输出：保证能编译，因为每个组件都是预验证的
```

| DSL 片段 | 生成的 Rust（预验证模板） |
|----------|--------------------------|
| `:method POST` | `axum::routing::post(handler)` |
| `:input (file ...)` | `Form<MultipartUpload>` |
| `:auth required` | `#[middleware(RequireAuth)]` |
| `:rate-limit "10/min"` | `#[rate_limit(10, Duration::MINUTE)]` |
| `:output (json :schema X)` | `Json<X>` |

## 纠错循环

即使 AI 选择了无效的组合，也可以自动纠正：

```
AI 输出 DSL
    ↓
Parser 校验失败
    ↓
结构化错误信息：
  "error: field 'max-size' expects format like '5MB', got '5'"
    ↓
喂回 AI
    ↓
AI 修正：:max-size "5MB"
    ↓
重新校验
    ↓
通过 → 生成代码
```

这个循环是 **确定性的**：

- 错误信息是结构化的（不是 Rust 编译器的长篇报错）
- 修正方式是明确的（改一个字段值）
- AI 几乎不可能连续犯同样的错误

## 与形式化验证的关系

形式化验证的核心是 **Curry-Howard 对应**：

> 程序 = 证明
> 类型 = 命题

在 Neural Codegen 中：

> DSL = 命题（"我要一个 POST API"）
> 生成的代码 = 证明（"这是一个正确实现的 POST API"）

如果 DSL 通过了校验器，那么生成的代码 **必然** 正确。这不是概率问题，是逻辑蕴含。

## 总结

| | CPU 模式 | GPU 模式 |
|---|---------|---------|
| AI 的角色 | 创作者 | 选择者 |
| 输出空间 | 无限 | 有限（白名单） |
| 正确率 | 低（需要迭代） | 高（预验证） |
| 纠错方式 | 看编译报错猜 | 结构化反馈 |
| 类比 | 通用计算 | 着色器执行 |
