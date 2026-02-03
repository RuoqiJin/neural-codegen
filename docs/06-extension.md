# 如何扩展 DSL

在"编译器/DSL"架构下，增加新功能涉及"全链路"的修改。

但好消息是，这种架构极其 **解耦**。只要按 **"自底向上（Bottom-Up）"** 的顺序来开发，流程是非常清晰且安全的。

## 示例：添加"视频变速"功能

### 第一阶段：Runtime（底座层）

在 AI 知道这个功能之前，Rust 后端必须先支持它。

```rust
// runtime/xjp_runtime/src/ops/speed.rs

pub struct Speed {
    pub factor: f32,
    pub keep_pitch: bool,
}

impl Speed {
    pub fn new(factor: f32, keep_pitch: bool) -> Self {
        Self { factor, keep_pitch }
    }

    pub fn execute(&self, input: &VideoFrame) -> Result<VideoFrame> {
        // 调用 ffmpeg 的 setpts 滤镜
        // ...
    }
}
```

**关键点**：这一步完全不涉及 AI 或编译器，纯粹是 Rust 开发。

### 第二阶段：Typed IR（接口层）

定义"中间层"如何表达这个操作。

```rust
// crates/nc-ir/src/step.rs

pub enum Step {
    Source { path: String },
    Filter { name: String },
    // ... 旧的 ...

    // [新增] 变速节点
    Speed { factor: f32, keep_pitch: bool },
}
```

### 第三阶段：Parser（S-expr 解析）

告诉 Parser 如何把 Lisp 里的 `(speed ...)` 映射到 IR。

```rust
// crates/nc-parser/src/parse.rs

fn parse_step(value: &Value) -> Result<Step, ParseError> {
    let op = get_symbol(value, 0)?;

    match op {
        "speed" => Ok(Step::Speed {
            factor: get_keyword_f32(value, ":factor")?,
            keep_pitch: get_keyword_bool(value, ":keep-pitch")
                .unwrap_or(true), // 默认保持音调
        }),
        // ...
    }
}
```

### 第四阶段：Codegen（代码生成）

把 IR 转换成 Runtime 的调用代码。

```rust
// crates/nc-codegen/src/gen.rs

fn codegen_step(step: &Step) -> TokenStream {
    match step {
        Step::Speed { factor, keep_pitch } => {
            quote! {
                pipeline.add_op(xjp_runtime::ops::Speed::new(
                    #factor,
                    #keep_pitch
                ));
            }
        }
        // ...
    }
}
```

### 第五阶段：AI Prompt（交互层）

最后一步才是教 AI。

**更新 System Prompt / Schema 文档：**

```markdown
## Available Commands

...

* `(speed :factor <float> :keep-pitch <bool>)`: Changes video playback speed.
  - factor > 1.0 is fast forward
  - factor < 1.0 is slow motion
  - Default keep-pitch is true
```

**更新 Few-Shot Examples：**

```
User: "快进两倍播放"
AI: (speed :factor 2.0)

User: "慢放到 0.5 倍，不要变声"
AI: (speed :factor 0.5 :keep-pitch true)
```

## 两种变更类型

### 类型 A：增加"原语"（New Primitive）

**例子**：变速、绿幕抠图、人脸检测

**流程**：必须走完 Runtime → IR → Parser → Codegen → Prompt 全流程

**频率**：低。只有核心引擎升级时才做。

**成本**：高

### 类型 B：增加"预设/组合"（New Preset/Macro）

**例子**：新的滤镜风格（Cyberpunk）、TikTok 风格剪辑

**流程**：
1. Runtime：把新的 LUT 文件放进 assets/
2. Prompt：告诉 AI "现在有一个叫 'Cyberpunk' 的滤镜"
3. 结束

**不需要动 Rust 代码，不需要改 IR，不需要重新发布编译器。**

**频率**：高

**成本**：极低

## 最佳实践

### 1. 通用参数（Generic Args）

在 IR 里留一个通用的 CustomOp：

```rust
pub enum Step {
    // ... 正式的 steps ...

    // 实验性功能入口
    CustomOp {
        name: String,
        args: HashMap<String, Value>,
    },
}
```

这样如果临时想加个实验性功能：
- 不需要改 Enum 结构
- 在 Runtime 里通过字符串匹配 name 就能跑通
- 验证成熟了再通过"正规流程"转正

### 2. 版本控制

编译器（Builder）和 Runtime（Engine）版本号对齐。

如果 AI 生成了包含 `(speed)` 的 DSL，但用户的 Runtime 是旧版本：

```
Error: 请升级您的播放器内核以支持变速功能
Required: runtime >= 1.2.0
Current: runtime 1.1.0
```

### 3. 向后兼容

IR 层做向后兼容：

```rust
fn migrate_ir(ir: RawIR, version: u32) -> CurrentIR {
    match version {
        1 => migrate_v1_to_v2(ir).and_then(migrate_v2_to_current),
        2 => migrate_v2_to_current(ir),
        3 => ir, // 当前版本
        _ => Err(UnsupportedVersion),
    }
}
```

旧 DSL 可以被自动迁移到新格式。

## Checklist

每当问"怎么加功能"时，按这个清单打钩：

- [ ] **Runtime**：Rust 库里实现了吗？有单元测试吗？
- [ ] **IR**：Enum 里加了吗？
- [ ] **Parser**：能从 Lisp 字符串读出来吗？
- [ ] **Codegen**：能生成对应的 Rust 调用代码吗？
- [ ] **Prompt**：AI 知道怎么用吗？

只要按这个顺序，架构就是稳如泰山的：

- AI 永远无法调用一个你没实现的功能（因为通不过 Parser）
- 你永远不会因为 AI 的胡言乱语导致底层崩溃（因为有 IR 这一层强类型屏障）

## 流程图

```
                    ┌─────────────────┐
                    │   用户需求       │
                    │ "我要加变速功能" │
                    └────────┬────────┘
                             │
              ┌──────────────┴──────────────┐
              │                             │
              ▼                             ▼
     ┌────────────────┐           ┌────────────────┐
     │  类型 A: 原语   │           │ 类型 B: 预设   │
     │  (新功能)       │           │ (新组合)       │
     └────────┬───────┘           └────────┬───────┘
              │                             │
              ▼                             ▼
     ┌────────────────┐           ┌────────────────┐
     │ 1. Runtime     │           │ 1. 放资源文件  │
     │ 2. IR          │           │ 2. 更新 Prompt │
     │ 3. Parser      │           │                │
     │ 4. Codegen     │           │ 结束           │
     │ 5. Prompt      │           └────────────────┘
     └────────┬───────┘
              │
              ▼
     ┌────────────────┐
     │   测试 & 发布   │
     └────────────────┘
```
