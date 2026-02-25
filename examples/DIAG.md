# xctl diag - AI 诊断功能使用指南

## 概述

`xctl diag` 是 xctl 的 AI 诊断功能，它能够：

1. **自动收集诊断信息**：从状态图中提取进程阻塞根因和相关上下文
2. **智能分析**：使用大模型（OpenAI/Claude）分析问题
3. **提供修复建议**：生成人话版的 SRE 诊断报告和修复建议

## 快速开始

### 1. 设置 API Key

```bash
# OpenAI
export OPENAI_API_KEY=sk-...

# 或 Claude
export ANTHROPIC_API_KEY=sk-ant-...
```

### 2. 启动 Daemon

```bash
cargo run --release -- run --probe examples/xctl-probe-nvml.py
```

### 3. 运行诊断

```bash
cargo run --release -- diag <PID>
```

## 使用示例

### 示例 1: 诊断 GPU 进程阻塞

```bash
$ cargo run --release -- diag 12345

[xctl] 正在诊断进程 12345...
[xctl] 收集诊断信息...

======================================================================
AI 诊断报告
======================================================================

阻塞根因:
  1. 等待资源: eth0
  2. error-gpu-00: ECC_UNCORRECTED:5

AI 诊断建议:
----------------------------------------------------------------------
## 问题根因分析

进程 12345 同时面临两个阻塞问题：
1. 网络 I/O 阻塞（等待 eth0 接口）
2. GPU 硬件错误（ECC 未纠正错误）

## 修复建议

1. **立即处理 GPU 错误**
   - ECC 未纠正错误可能导致数据损坏
   - 建议重启 GPU 或迁移任务到其他 GPU
   - 检查 GPU 硬件状态：nvidia-smi

2. **排查网络阻塞**
   - 检查网络接口状态：ip addr show eth0
   - 查看网络丢包：cat /proc/net/dev
   - 检查防火墙规则

3. **监控资源使用**
   - 使用 xctl ps 持续监控进程状态
   - 设置告警阈值

置信度: 80%
```

## 支持的大模型提供商

### OpenAI

```bash
export OPENAI_API_KEY=sk-...
cargo run --release -- diag <PID> --provider openai
```

**模型**: `gpt-4o-mini`（成本优化）

### Claude (Anthropic)

```bash
export ANTHROPIC_API_KEY=sk-ant-...
cargo run --release -- diag <PID> --provider claude
```

**模型**: `claude-3-haiku-20240307`（成本优化）

### 本地模型（未来支持）

```bash
# 需要运行 Ollama 或类似服务
cargo run --release -- diag <PID> --provider local
```

## 诊断信息收集

`xctl diag` 会自动收集以下信息：

1. **阻塞根因**：通过 `xctl why` 获取
2. **进程状态**：进程列表和资源使用
3. **图状态**：WaitsOn、BlockedBy 等关系

这些信息会被组装成结构化的 Prompt 发送给大模型。

## Prompt 示例

发送给大模型的 Prompt 格式：

```
## 问题描述

进程 PID 12345 出现性能问题。

## 阻塞根因分析

1. 等待资源: eth0
2. error-gpu-00: ECC_UNCORRECTED:5

## 相关进程信息

- PID 12345: 状态=running, 资源=[gpu-00, eth0]
- PID 12346: 状态=running, 资源=[gpu-01]

## 请提供诊断建议

请基于以上信息，提供：
1. 问题根因分析（用一句话概括）
2. 具体的修复建议（3-5 条）
3. 预防措施（可选）

请用简洁、专业的中文回答，避免技术术语过多。
```

## 成本优化

默认使用成本较低的模型：
- OpenAI: `gpt-4o-mini`（而非 `gpt-4`）
- Claude: `claude-3-haiku`（而非 `claude-3-opus`）

每次诊断约消耗：
- OpenAI: ~$0.001-0.01（取决于问题复杂度）
- Claude: ~$0.0005-0.005

## 故障排查

### 问题: "未设置 OPENAI_API_KEY 环境变量"

**解决**:
```bash
export OPENAI_API_KEY=your_key
# 或
export ANTHROPIC_API_KEY=your_key
```

### 问题: "API 请求失败"

**可能原因**:
1. 网络连接问题
2. API Key 无效
3. 账户余额不足

**解决**:
1. 检查网络连接
2. 验证 API Key
3. 检查账户余额

### 问题: "无法连接到 daemon"

**解决**:
```bash
# 确保 daemon 正在运行
cargo run --release -- run
```

## 最佳实践

1. **定期诊断**：在发现性能问题时立即运行诊断
2. **保存报告**：将诊断结果保存到文件
   ```bash
   cargo run --release -- diag <PID> > diagnosis.txt
   ```
3. **结合监控**：配合 `xctl ps` 和 `xctl why` 使用
4. **成本控制**：使用环境变量控制模型选择

## 未来增强

- [ ] 支持更多大模型提供商（本地模型、开源模型）
- [ ] 结构化输出（JSON 格式）
- [ ] 诊断历史记录
- [ ] 自动修复建议（集成到 `xctl zap`）
- [ ] 批量诊断（多个进程）
