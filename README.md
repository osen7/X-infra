# `xctl` - 极简主义异构 AI 算力集群管控底座

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

`xctl` 是一款专为 AI 基础设施（GPU/NPU/网络/存储）打造的底层观测与管控工具。采用事件驱动架构，提供实时因果图分析和 AI 诊断能力。

## ✨ 特性

- 🚀 **事件驱动内核**：基于事件流的实时状态图，零轮询开销
- 🔌 **可插拔探针**：支持 GPU（NVML）、网络（eBPF/proc）、存储等多种探针
- 🧠 **AI 诊断**：集成大模型（OpenAI/Claude），自动生成修复建议
- 🔍 **因果分析**：自动推导进程-资源-错误的因果关系
- 💻 **极简 CLI**：类似 Docker 的 C/S 架构，轻量级客户端
- 🛡️ **生产级**：内存安全、错误处理完善、OOM 防护

## 🚀 快速开始

```bash
# 1. 克隆仓库
git clone https://github.com/osen7/X-infra.git
cd X-infra

# 2. 构建项目
cargo build --release

# 3. 启动 daemon（使用 GPU 探针）
cargo run --release -- run --probe examples/xctl-probe-nvml.py

# 4. 在另一个终端查询
cargo run --release -- ps
cargo run --release -- why <PID>
cargo run --release -- diag <PID>  # AI 诊断
```

详细使用指南请查看 [README_USAGE.md](README_USAGE.md) 和 [QUICKSTART.md](QUICKSTART.md)。

## 📖 文档

- [使用指南](README_USAGE.md) - 完整的功能说明和使用示例
- [快速开始](QUICKSTART.md) - 5 分钟上手指南
- [项目路线图](docs/ROADMAP.md) - 开发计划和里程碑
- [规则引擎](docs/RULES_ENGINE.md) - 声明式规则系统
- [eBPF 网络探针](docs/EBPF_NETWORK_PROBE.md) - 内核级网络监控
- [探针开发](examples/README.md) - 如何开发自定义探针

## 🏗️ 架构设计

### 核心原则

- **事件引擎为核心**：所有底层信号转化为追加写入的事件流
- **KISS 原则**：单机可运行，拒绝过度设计
- **探针彻底解耦**：核心不包含硬件 SDK，探针通过 stdout 输出 JSONL
- **内存极其克制**：使用 Ring Buffer 和无锁通道处理高频事件

### 数据模型

- **8 大原子事件**：计算、传输、存储、进程、错误、拓扑、意图、动作
- **3 大推导边**：Consumes（消耗）、WaitsOn（等待）、BlockedBy（阻塞）


## 📦 项目结构

```
x-infra/
├── src/
│   ├── main.rs          # CLI 入口
│   ├── event.rs         # 事件定义和事件总线
│   ├── graph.rs         # 状态图引擎
│   ├── ipc.rs           # IPC 服务（TCP 9090）
│   ├── diag.rs          # AI 诊断模块
│   ├── plugin/          # 探针系统
│   └── exec/            # 执行器
├── examples/
│   ├── xctl-probe-nvml.py      # NVIDIA GPU 探针
│   ├── xctl-probe-network.py    # 网络探针
│   └── xctl-probe-dummy.py      # 模拟探针
└── docs/                # 文档
```

## 🔧 开发

### 前置要求

- Rust 1.70+
- Python 3.7+（用于探针脚本）
- Linux（网络探针需要 `/proc/net`）

### 构建

```bash
cargo build --release
```

### 测试

```bash
# 运行内置探针测试
cargo run --release -- run

# 测试 GPU 探针（需要 NVIDIA GPU）
pip install pynvml
cargo run --release -- run --probe examples/xctl-probe-nvml.py
```

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

## 📄 许可证

MIT License

## 🙏 致谢

本项目遵循极简主义设计哲学，致力于为 AI 基础设施提供轻量、高效的监控和诊断能力。

## 📊 核心数据模型

### 8 大原子事件

- **计算域**: `compute.util` (算力利用率), `compute.mem` (显存/内存使用率)
- **传输域**: `transport.bw` (网络吞吐), `transport.drop` (丢包/重传)
- **存储域**: `storage.iops` (存储 IO), `storage.qdepth` (队列深度)
- **进程域**: `process.state` (进程状态)
- **错误域**: `error.hw` (硬件级报错), `error.net` (网络阻塞报错)
- **拓扑域**: `topo.link_down` (NVLink/PCIe 降级)
- **意图域**: `intent.run` (调度器元数据)
- **动作域**: `action.exec` (系统干预动作)

### 3 大推导边

在状态图中，事件转化为 DAG（有向无环图），边只有三种：

1. **Consumes** (消耗)：进程 PID 消耗某物理资源
2. **WaitsOn** (等待)：进程 PID 正在等待某网络/存储资源完成
3. **BlockedBy** (阻塞于)：资源/进程被某个 Error 彻底阻塞（根因）

## 🔗 相关链接

- [GitHub 仓库](https://github.com/osen7/X-infra)
- [问题反馈](https://github.com/osen7/X-infra/issues)
- [功能建议](https://github.com/osen7/X-infra/issues/new)