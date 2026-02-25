# xctl 使用指南

## 快速开始

### 1. 构建项目

```bash
cargo build --release
```

### 2. 运行 Daemon 模式

启动事件总线、状态图和 IPC 服务，持续收集事件：

```bash
# 使用内置探针（演示用）
cargo run -- run

# 使用真实的 NVIDIA NVML 探针（推荐）
# 首先安装依赖: pip install pynvml
cargo run -- run --probe examples/xctl-probe-nvml.py

# 使用模拟探针（测试用）
cargo run -- run --probe examples/xctl-probe-dummy.py

# 指定自定义 IPC 端口
cargo run -- run --port 9091
```

### 3. 查询进程列表

在另一个终端中查询当前活跃进程（通过 IPC 连接 daemon）：

```bash
cargo run -- ps

# 指定端口（如果 daemon 使用了非默认端口）
cargo run -- ps --port 9091
```

### 4. 分析进程阻塞根因

查询指定进程的阻塞原因（通过 IPC 连接 daemon）：

```bash
cargo run -- why <PID>

# 指定端口
cargo run -- why <PID> --port 9091
```

### 5. 强制终止进程

终止指定进程及其进程树（不依赖 daemon）：

```bash
cargo run -- zap <PID>
```

### 6. AI 诊断（使用大模型分析问题）

使用大模型分析进程阻塞根因并提供修复建议：

```bash
# 使用 OpenAI（默认）
export OPENAI_API_KEY=your_key
cargo run -- diag <PID>

# 使用 Claude
export ANTHROPIC_API_KEY=your_key
cargo run -- diag <PID> --provider claude

# 指定端口
cargo run -- diag <PID> --port 9091
```

## 架构说明

### IPC 通信机制

xctl 采用客户端-服务器架构：

- **Daemon 模式** (`xctl run`): 启动后台服务，监听 TCP 端口（默认 9090）
  - 运行事件总线，接收探针事件
  - 维护状态图（StateGraph）
  - 提供 IPC 服务，响应查询请求

- **CLI 命令** (`xctl ps`, `xctl why`): 通过 TCP 连接到 daemon
  - 发送 JSON RPC 请求
  - 接收并渲染查询结果

这种设计完全符合 Docker 的架构模式（`docker client -> /var/run/docker.sock -> dockerd`），实现了控制面和数据面的彻底分离。

### 使用外部探针

xctl 支持通过子进程探针集成外部监控工具。探针脚本需要：

1. 通过 stdout 输出 JSONL 格式的事件（每行一个 JSON 对象）
2. 事件格式必须符合 `Event` 结构体的定义
3. 使用蛇形小写加点格式的事件类型（如 `compute.util`, `process.state`）

### NVIDIA NVML 探针（真实 GPU 监控）

这是 xctl 提供的生产级探针，可以真实抓取 NVIDIA GPU 的利用率、显存、温度等信息。

#### 安装依赖

```bash
pip install pynvml
# 或使用 requirements.txt
pip install -r examples/requirements.txt
```

#### 使用方法

```bash
cargo run -- run --probe examples/xctl-probe-nvml.py
```

#### 监控指标

NVML 探针会定期输出以下事件：

- **compute.util**: GPU 利用率（0-100%）
- **compute.mem**: GPU 显存使用率（0-100%）
- **error.hw**: GPU 硬件错误（ECC 错误、高温告警等）
- **进程关联**: 自动关联使用 GPU 的进程 PID

#### 环境变量

- `XCTL_NVML_INTERVAL`: 采样间隔（秒），默认 1.0

```bash
XCTL_NVML_INTERVAL=2.0 cargo run -- run --probe examples/xctl-probe-nvml.py
```

### 网络探针（监控网络阻塞）

监控网络 I/O 阻塞、丢包和带宽使用，建立 WaitsOn 关系。

#### 基于 /proc/net 的探针（推荐，无需 root）

```bash
cargo run -- run --probe examples/xctl-probe-network.py
```

**功能**:
- 监控网络接口带宽使用
- 检测丢包和网络错误
- 识别等待网络 I/O 的进程
- 自动建立 WaitsOn 边（进程等待网络资源）

**环境变量**:
- `XCTL_NETWORK_INTERVAL`: 采样间隔（秒），默认 2.0

#### eBPF 探针（高性能，需要 root）

eBPF 探针提供零开销的网络监控，但需要：
- Linux 内核 4.18+
- root 权限或 CAP_BPF 能力
- libbpf 库

详见 [examples/ebpf/README.md](examples/ebpf/README.md)

### 模拟探针（测试用）

```bash
cargo run -- run --probe examples/xctl-probe-dummy.py
```

### 组合使用多个探针

xctl 支持同时运行多个探针。你可以：

1. **启动 GPU 探针**:
   ```bash
   cargo run -- run --probe examples/xctl-probe-nvml.py
   ```

2. **在另一个终端启动网络探针**（需要修改代码支持多探针，或使用进程管理工具）:
   ```bash
   python3 examples/xctl-probe-network.py | nc localhost 9090
   ```

注意：当前版本一次只能运行一个探针。多探针支持是未来增强功能。

#### 事件 JSON 格式

```json
{
  "ts": 1234567890123,
  "event_type": "compute.util",
  "entity_id": "gpu-03",
  "job_id": "job-1234",
  "pid": 5678,
  "value": "85"
}
```

#### 支持的事件类型

- `compute.util` - 算力利用率
- `compute.mem` - 显存/内存使用率
- `transport.bw` - 网络吞吐
- `transport.drop` - 丢包/重传
- `storage.iops` - 存储 IO
- `storage.qdepth` - 队列深度
- `process.state` - 进程状态 (start/exit/zombie)
- `error.hw` - 硬件级报错
- `error.net` - 网络阻塞报错
- `topo.link_down` - NVLink/PCIe 降级或断开
- `intent.run` - 调度器元数据
- `action.exec` - 系统干预动作

## 核心模块

- **事件总线** (`event.rs`): 基于 tokio mpsc 通道的事件分发器
- **状态图** (`graph.rs`): 实时因果图，维护进程-资源-错误的关系
- **IPC 服务** (`ipc.rs`): TCP 服务器，提供远程查询接口
- **查询引擎** (`query.rs`): 提供 `ps` 和 `why` 查询接口（已集成到 IPC）
- **探针系统** (`plugin/`): 可插拔的事件源，支持子进程探针
- **执行器** (`exec/`): 系统级操作（如进程清理）

## AI 诊断功能

`xctl diag` 使用大模型分析进程问题并提供修复建议。

### 配置

```bash
# 设置 API Key（二选一）
export OPENAI_API_KEY=sk-...
export ANTHROPIC_API_KEY=sk-ant-...

# 选择提供商（可选，默认从环境变量推断）
export XCTL_LLM_PROVIDER=openai  # 或 claude
```

### 使用

```bash
cargo run -- diag <PID>
```

详见 [examples/DIAG.md](examples/DIAG.md)

## 注意事项

1. **Windows 支持**: 
   - `zap` 命令在 Windows 上使用 `taskkill /F /T`，在 Linux 上使用 `kill -9`
   - Python 探针在 Windows 上使用 `python` 命令，在 Linux 上使用 `python3`

2. **状态持久化**: 
   - 当前实现中，状态图完全在内存中
   - `ps`、`why` 和 `diag` 命令必须通过 IPC 连接到运行中的 daemon 才能获取实时状态
   - 如果 daemon 未运行，这些命令会提示错误

3. **内存管理**: 
   - 状态图会自动清理过期的错误节点（5分钟窗口）
   - 只清理明确标记为 exit/zombie 的进程，不清理稳态运行的进程
   - 资源节点（Resource）不会被自动清理，需要探针发送心跳事件来维持

4. **探针心跳**: 
   - 建议探针即使数据不变，也要定期发送当前状态事件（心跳）
   - 这可以防止长时间没有更新的资源节点被误认为过期

5. **AI 诊断成本**: 
   - 每次诊断约消耗 $0.001-0.01（取决于问题复杂度）
   - 默认使用成本较低的模型（gpt-4o-mini / claude-3-haiku）
   - 建议在生产环境中设置使用频率限制

## 故障排查

### 无法连接到 daemon

```
[xctl] 错误：无法连接到 daemon (端口 9090)
[xctl] 请先运行: xctl run
```

**解决方案**: 确保 daemon 正在运行，或检查端口是否正确。

### 探针脚本无法启动

**解决方案**: 
- 检查脚本路径是否正确
- 确保 Python 已安装并在 PATH 中
- 检查脚本是否有执行权限（Linux/Mac）

### IPC 端口被占用

**解决方案**: 使用 `--port` 参数指定其他端口。
