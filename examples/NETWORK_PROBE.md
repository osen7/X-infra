# 网络探针使用指南

## 概述

网络探针是 xctl 实现 **WaitsOn（网络阻塞推导）** 能力的核心组件。它能够：

1. **检测网络 I/O 阻塞**：识别进程等待网络资源的情况
2. **监控网络质量**：检测丢包、错误和带宽使用
3. **建立因果关系**：自动在图引擎中建立 WaitsOn 边

## 快速开始

### 1. 启动网络探针

```bash
cargo run --release -- run --probe examples/xctl-probe-network.py
```

### 2. 验证网络事件

在另一个终端运行网络负载测试：

```bash
# 生成网络流量
curl -o /dev/null https://www.example.com
# 或
wget -O /dev/null https://www.example.com
```

### 3. 查询进程和网络关系

```bash
# 查看进程列表（会显示网络资源）
cargo run --release -- ps

# 分析进程阻塞根因（会显示网络等待）
cargo run --release -- why <PID>
```

## WaitsOn 关系示例

当进程等待网络 I/O 时，xctl 会自动建立 WaitsOn 边：

```
进程 PID-12345
    |
    | WaitsOn
    v
网络接口 eth0 (阻塞中)
```

### 查询示例

```bash
$ cargo run --release -- why 12345

进程 12345 的阻塞根因分析:
────────────────────────────────────────────────────────────
  1. 等待资源: eth0
  2. 等待资源: socket-12345
```

## 监控指标

网络探针会生成以下事件：

### 1. transport.bw（带宽使用）

```json
{
  "ts": 1234567890,
  "event_type": "transport.bw",
  "entity_id": "eth0",
  "pid": null,
  "value": "125.5"
}
```

- **entity_id**: 网络接口名称（如 `eth0`, `ens33`）
- **value**: 带宽使用（Mbps）

### 2. transport.drop（网络阻塞/丢包）

```json
{
  "ts": 1234567890,
  "event_type": "transport.drop",
  "entity_id": "eth0",
  "pid": 12345,
  "value": "IO_WAIT"
}
```

- **pid**: 等待网络 I/O 的进程 PID
- **value**: 阻塞类型（`IO_WAIT` 表示 I/O 等待）

### 3. error.net（网络错误）

```json
{
  "ts": 1234567890,
  "event_type": "error.net",
  "entity_id": "eth0",
  "pid": null,
  "value": "RX_ERR:5,TX_ERR:2"
}
```

## 高级用法

### 自定义采样间隔

```bash
XCTL_NETWORK_INTERVAL=5.0 cargo run --release -- run --probe examples/xctl-probe-network.py
```

### 组合 GPU 和网络探针

虽然当前版本一次只能运行一个探针，但你可以：

1. **优先使用网络探针**（如果网络是瓶颈）:
   ```bash
   cargo run --release -- run --probe examples/xctl-probe-network.py
   ```

2. **优先使用 GPU 探针**（如果计算是瓶颈）:
   ```bash
   cargo run --release -- run --probe examples/xctl-probe-nvml.py
   ```

3. **未来支持多探针**（计划中）:
   ```bash
   cargo run --release -- run \
     --probe examples/xctl-probe-nvml.py \
     --probe examples/xctl-probe-network.py
   ```

## 故障排查

### 问题: "此探针需要 Linux 系统"

**原因**: 网络探针依赖 `/proc/net`，只在 Linux 上可用

**解决**: 
- Windows/Mac: 使用模拟探针或等待 eBPF 探针支持
- Linux: 确保有 `/proc/net` 目录

### 问题: "权限被拒绝"

**原因**: 读取 `/proc` 需要权限

**解决**: 
- 确保用户有读取 `/proc` 的权限
- 某些系统可能需要将用户添加到特定组

### 问题: "未检测到网络事件"

**原因**: 
- 没有网络活动
- 采样间隔太长
- 网络接口名称不匹配

**解决**:
1. 生成一些网络流量（curl, wget）
2. 减小采样间隔: `XCTL_NETWORK_INTERVAL=1.0`
3. 检查网络接口: `ip addr` 或 `ifconfig`

## 性能考虑

### /proc/net 探针的开销

- **CPU**: 每 2 秒约 1-5ms（取决于接口数量）
- **内存**: 约 10-50MB（缓存统计信息）
- **I/O**: 每 2 秒读取 `/proc/net/*` 文件

### 优化建议

1. **增加采样间隔**（如果不需要实时监控）:
   ```bash
   XCTL_NETWORK_INTERVAL=5.0
   ```

2. **使用 eBPF 探针**（如果可用）:
   - 零开销监控
   - 实时事件捕获
   - 需要 root 权限

3. **过滤接口**（未来功能）:
   - 只监控特定接口
   - 排除回环接口（已实现）

## 下一步

- 查看 [ebpf/README.md](ebpf/README.md) 了解 eBPF 探针
- 查看 [README.md](../README.md) 了解完整架构
- 准备进入路线 C（大模型诊断）
