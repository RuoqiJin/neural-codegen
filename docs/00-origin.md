# 起源：从 Debug 到 Neural Codegen

本项目的理论基础来自一场深度对话，从一个简单的问题开始：

> **"编程为什么要 Debug？理论上它不是应该像数学公式一样运转吗？"**

这个问题触及了计算的本质，最终引出了 Neural Codegen 的核心设计理念。

## 对话脉络

```
编程为什么要 Debug？
    ↓
"我能底层重写吗？"（追求数学纯净性）
    ↓
形式化验证 & 函数式编程（Haskell）
    ↓
Lambda 演算（计算的数学本质）
    ↓
为什么硬件选择了图灵机而非 Lambda？
    ↓
物理定律的约束（修改比复制便宜）
    ↓
现代趋势：硬件正在 "Lambda 化"（GPU、TPU、FPGA）
    ↓
AI 时代的新可能：自然语言 → DSL → 二进制
    ↓
Neural Codegen 的诞生
```

## 核心洞察

### 1. 理想与现实的鸿沟

**数学世界**：纯净、无状态、永恒
**物理世界**：能量有限、时间流逝、空间受限

Debug 的本质是弥合这两个世界的鸿沟。

### 2. 两种计算范式

| Lambda 演算 | 图灵机 |
|------------|--------|
| 变换系统 | 状态机系统 |
| 像不断变形的云 | 像老会计查账 |
| 需要无限复制空间 | 只需一条纸带 |
| 物理上昂贵 | 物理上便宜 |

现代计算机选择了图灵机，因为**物理定律喜欢它**。

### 3. 历史的反转

虽然图灵机赢了 40 年，但现在它撞墙了：
- 冯诺依曼瓶颈
- 摩尔定律失效

现代硬件正在悄悄 "Lambda 化"：
- GPU/TPU：数据流，而非指令流
- FPGA：电路即逻辑
- Groq LPU：硬件化的 Lambda

### 4. AI 的机会

AI（LLM）的特点：
- 擅长模式匹配和生成
- 不擅长精确控制和状态管理
- 容易"幻觉"

**结论**：让 AI 写"声明式 DSL"，而非"命令式代码"。

这就是 Neural Codegen 的理论基础：
- AI 只做"选择"（从预定义选项中挑选）
- 不做"创作"（不生成任意代码）
- 像 GPU 着色器一样：输入确定，输出确定

## 文档索引

| 文档 | 内容 |
|------|------|
| [01-why-debug.md](./01-why-debug.md) | 为什么编程需要 Debug |
| [02-lambda-calculus.md](./02-lambda-calculus.md) | Lambda 演算的本质 |
| [03-turing-vs-lambda.md](./03-turing-vs-lambda.md) | 图灵机 vs Lambda：物理的选择 |
| [04-gpu-mode.md](./04-gpu-mode.md) | GPU 模式的理论基础 |
| [05-implementation.md](./05-implementation.md) | 方案 B 的工程实现 |
| [06-extension.md](./06-extension.md) | 如何扩展 DSL |
| [08-parallelism-confluence.md](./08-parallelism-confluence.md) | 并行性与合流性：硬件 Lambda 的启示 |
| [09-instruction-set-evolution.md](./09-instruction-set-evolution.md) | 指令集演进：让 DSL 不断变聪明 |
