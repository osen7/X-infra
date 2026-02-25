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
- [探针开发](examples/README.md) - 如何开发自定义探针
- [网络探针](examples/NETWORK_PROBE.md) - 网络监控和 WaitsOn 能力
- [AI 诊断](examples/DIAG.md) - 大模型诊断功能

## 🏗️ 架构设计

### 核心原则

- **事件引擎为核心**：所有底层信号转化为追加写入的事件流
- **KISS 原则**：单机可运行，拒绝过度设计
- **探针彻底解耦**：核心不包含硬件 SDK，探针通过 stdout 输出 JSONL
- **内存极其克制**：使用 Ring Buffer 和无锁通道处理高频事件

### 数据模型

- **8 大原子事件**：计算、传输、存储、进程、错误、拓扑、意图、动作
- **3 大推导边**：Consumes（消耗）、WaitsOn（等待）、BlockedBy（阻塞）

详见 [README.md](README.md#2-核心数据模型-mvem---最小可行事件模型)。

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

---

# `xctl` 极简主义异构 AI 算力集群管控底座 - 开发指南

## 1. 项目愿景与设计哲学

`xctl` 是一款专为 AI 基础设施（GPU/NPU/网络/存储）打造的底层观测与管控工具。 本项目的代码生成必须严格遵循以下原则，**请 AI 助手在开发全程将其作为最高系统指令（System Prompt）**：

- **事件引擎为核心 (Event-Sourcing Core)**：摒弃状态轮询，所有底层信号全部转化为追加写入（Append-only）的事件流。
- **绝对的 KISS 原则 (Keep It Simple, Stupid)**：单机必须可运行。拒绝复杂的 RPC 框架，拒绝过度设计。
- **探针彻底解耦 (Pluggable Probes)**：核心引擎（xctl-core）不包含任何硬件厂商 SDK（如 `nvml`、`cann`）。外围探针只通过标准输出（stdout）向核心吐标准 JSONL 文本。
- **内存极其克制**：作为 Daemon 驻留节点，内存占用须控制在极小范围，使用 Ring Buffer 和无锁通道（Channel）处理高频事件。

------

## 2. 核心数据模型 (MVEM - 最小可行事件模型)

在实现代码前，必须先确立坚如磐石的领域模型。

### 2.1 八大原子事件 (Atom Events)

Rust

```
// 核心事件类型枚举
pub enum EventType {
    // 1. 计算域
    ComputeUtil,    // 算力利用率 (如: gpu.util)
    ComputeMem,     // 显存/内存使用率 (如: gpu.mem)
    // 2. 传输域
    TransportBw,    // 网络吞吐 (如: rdma.bw)
    TransportDrop,  // 丢包/重传 (如: rdma.drop)
    // 3. 存储域
    StorageIops,    // 存储 IO (如: nvme.iops)
    StorageQDepth,  // 队列深度 (如: nvme.qdepth)
    // 4. 进程域
    ProcessState,   // 进程状态 (start/exit/zombie)
    // 5. 错误域
    ErrorHw,        // 硬件级报错 (如 XID/ECC)
    ErrorNet,       // 网络阻塞报错 (如 PFC Storm)
    // 6. 拓扑域
    TopoLinkDown,   // NVLink/PCIe 降级或断开
    // 7. 意图域
    IntentRun,      // 调度器元数据 (如 Job 分配)
    // 8. 动作域
    ActionExec,     // 系统干预动作 (如 kill/reset)
}

// 统一的事件载体
pub struct Event {
    pub ts: u64,                  // 毫秒级时间戳
    pub event_type: EventType,    // 事件类型
    pub entity_id: String,        // 物理资源抽象ID (如 "gpu-03", "mlx5_0")
    pub job_id: Option<String>,   // 关联的任务ID (如果有)
    pub pid: Option<u32>,         // 关联的进程PID (如果有)
    pub value: String,            // 具体载荷 (如 "85", "XID_79")
}
```

### 2.2 三大推导边 (Causal Edges)

在 `graph.rs` 中，事件将转化为 DAG（有向无环图），边只有三种：

1. `Consumes` (消耗)：进程 PID 消耗 某物理资源。
2. `WaitsOn` (等待)：进程 PID 正在等待 某网络/存储资源完成。
3. `BlockedBy` (阻塞于)：资源/进程 被某个 Error 彻底阻塞（根因）。

------

## 3. Rust 工程结构树 (xctl-core)

请按以下目录结构初始化 Rust 项目：

Plaintext

```
xctl/
├── Cargo.toml
└── src/
    ├── main.rs          // CLI 路由层 (基于 clap)
    ├── event.rs         // Event 定义与有界通道 (Bounded Channel) 事件总线
    ├── graph.rs         // 基于事件流构建的实时因果图 (状态树)
    ├── query.rs         // 对图的查询接口 (实现 xctl why / ps)
    ├── plugin/
    │   ├── mod.rs       // 探针生命周期管理 (拉起子进程读取 stdout)
    │   └── trait.rs     // EventSource 和 Actuator Trait 定义
    └── exec/
        └── mod.rs       // 干预动作执行器 (调用 kill -9 等底层命令)
```

### 3.1 核心防污染边界 (trait.rs)

Rust

```
use tokio::sync::mpsc;
use async_trait::async_trait;

#[async_trait]
pub trait EventSource: Send + Sync {
    fn name(&self) -> &str;
    // 启动探针，将产生的事件推入发送端
    async fn start_stream(&self, tx: mpsc::Sender<Event>);
}

#[async_trait]
pub trait Actuator: Send + Sync {
    fn name(&self) -> &str;
    async fn execute(&self, target_pid: u32, action: &str) -> Result<(), String>;
}
```

------

## 4. AI 辅助开发计划 (分阶段 Milestone)

**致 AI 开发助手：** 请不要一次性写完所有代码。请严格按照以下 Phase 逐个实现，并在每个 Phase 结束后请求用户测试。

### 🚩 Phase 1: 骨架与内存安全事件总线

**目标**：实现事件数据结构，构建无阻塞的事件消费循环。

1. 初始化 `Cargo.toml`，引入依赖：`tokio` (异步 runtime), `serde`, `serde_json` (序列化), `clap` (CLI)。
2. 在 `src/event.rs` 中定义 `Event` 和 `EventType`。
3. 实现 `EventBus` 结构体：包含一个 `mpsc::channel`。
4. 编写一个模拟的 `dummy_probe`，每秒向 Bus 随机发送几条 `gpu.util` 和 `process.start` 事件，并能在控制台打印。

### 🚩 Phase 2: 时序因果图 (Workload Graph Engine)

**目标**：将杂乱的事件流聚合成具有逻辑关系的拓扑图。

1. 在 `src/graph.rs` 中设计 `StateGraph`。**注意：不要保存无限历史，只保留当前活跃的 PID 和近 5 分钟的 Error 窗口。**
2. 实现事件消费逻辑：
   - 收到 `process.start` -> 在图中创建 PID 节点。
   - 收到 `gpu.util` 带 PID -> 建立 PID `Consumes` GPU 的边，更新利用率状态。
   - 收到 `rdma.stall` 带 PID -> 建立 PID `WaitsOn` RDMA 的边。
   - 收到 `error.hw` (掉卡) -> 建立所有使用该卡的 PID `BlockedBy` 该卡的边。

### 🚩 Phase 3: 探针协议层 (Plugin Lifecycle)

**目标**：真正解耦硬件厂商探针。

1. 在 `src/plugin/mod.rs` 中实现一个 `SubprocessProbe`。
2. 逻辑：通过 `tokio::process::Command` 启动一个外部脚本（如 `xctl-probe-dummy.sh`）。
3. 使用异步流按行读取该脚本的 stdout (`stdout.lines()`)。
4. 将每一行 JSON 字符串解析为 `Event` 结构体，推入 `EventBus`。
5. *鲁棒性要求*：如果子进程崩溃或卡死，不得阻塞 xctl 主循环，仅记录一条 `Error` 事件。

### 🚩 Phase 4: 极简 CLI 视图层 (The Linux Way)

**目标**：让用户感受到极速的命令行体验。

1. 在 `src/main.rs` 中使用 `clap` 设置子命令：
   - `xctl run`：启动后台 Daemon 模式（运行上述 EventBus 和 Probe）。
   - `xctl ps`：查询当前状态图，打印成带对齐格式的文本表格（PID | 资源 | 状态）。
   - `xctl why <pid>`：从该 PID 节点在图中发起**逆向深度优先搜索 (DFS)**。
     - *如果是 `WaitsOn` -> 打印 "等待 IO/网络"。*
     - *如果追溯到了 `BlockedBy` -> 打印致命根因 (如 "根因: 硬件 XID 错误")。*

### 🚩 Phase 5: 处决执行器 (The Actuator)

**目标**：实现 `xctl zap <pid>` 的系统级清理功能。

1. 在 `src/exec/mod.rs` 中实现清理逻辑。
2. 不仅仅是发送 `kill -9`，需要通过读取 `/proc/{pid}/task/` 或进程组（PGID）确保彻底干掉整个僵尸进程树。

------

## 5. 开发约束规范 (System Rules for AI)

1. **并发模型**：统一使用 `tokio`。在共享图状态时，优先考虑读写锁 `tokio::sync::RwLock`，因为读多（查询）写少（事件合并）。
2. **错误处理**：凡是涉及外部系统调用（读 `/proc`，拉起子进程），必须有完善的 `Result` 返回机制。绝对禁止使用 `unwrap()` 或 `expect()` 导致主进程 Panic。
3. **零中间件**：不允许在初期引入任何数据库（如 SQLite、Redis）。状态全在内存的 `graph.rs` 中，用极简的 HashMap 或 Arena Allocator 实现。
4. **CLI 输出美学**：终端打印不要用冗长的 JSON。使用类似于 `docker ps` 的紧凑型文本输出。可以引入 `cli-table` 或 `colored` 库增强可读性。

------

**用户指令：** “我已经准备好建立项目了。请阅读上述文档，确认你理解了整体架构，然后直接为我生成 `Cargo.toml` 以及 **Phase 1** 中 `src/event.rs` 和 `src/main.rs` 的初始代码骨架。”