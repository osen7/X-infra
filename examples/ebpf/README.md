# eBPF 网络探针开发指南

## 概述

eBPF（extended Berkeley Packet Filter）是 Linux 内核的一个强大功能，允许在内核空间运行沙箱程序。对于网络监控，eBPF 可以提供：

1. **零开销监控**：在内核空间直接捕获网络事件，无需系统调用
2. **精确的进程关联**：直接获取 socket 对应的 PID
3. **实时阻塞检测**：监控 socket read/write 的阻塞时间
4. **低延迟**：事件在内核中直接处理

## 架构设计

### eBPF 程序挂载点

```
内核函数              eBPF Hook              监控内容
─────────────────────────────────────────────────────────
tcp_sendmsg    ->  kprobe/tracepoint  ->  TCP 发送延迟
tcp_recvmsg    ->  kprobe/tracepoint  ->  TCP 接收阻塞
udp_sendmsg    ->  kprobe/tracepoint  ->  UDP 发送
udp_recvmsg    ->  kprobe/tracepoint  ->  UDP 接收
tcp_retransmit ->  kprobe             ->  重传检测（丢包）
```

### 事件类型

eBPF 探针应该生成以下 xctl 事件：

1. **transport.bw**: 网络带宽使用
   ```json
   {
     "event_type": "transport.bw",
     "entity_id": "eth0",
     "pid": 12345,
     "value": "125.5"  // Mbps
   }
   ```

2. **transport.drop**: 网络阻塞/丢包
   ```json
   {
     "event_type": "transport.drop",
     "entity_id": "socket-12345",
     "pid": 12345,
     "value": "STALL_100ms"  // 阻塞时间
   }
   ```

3. **error.net**: 网络错误
   ```json
   {
     "event_type": "error.net",
     "entity_id": "eth0",
     "pid": null,
     "value": "RETRANSMIT:5"  // 重传次数
   }
   ```

## 实现步骤

### 1. 安装依赖

```bash
# 安装 libbpf 开发库
sudo apt-get install libbpf-dev  # Debian/Ubuntu
sudo yum install libbpf-devel    # RHEL/CentOS

# 添加 Rust 依赖
cargo add libbpf-rs
cargo add libbpf-sys
```

### 2. 编写 eBPF 程序

创建 `network_probe.bpf.c`:

```c
#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

// 定义事件结构
struct network_event {
    u32 pid;
    u64 bytes;
    u64 timestamp;
    char event_type[8];  // "send" or "recv"
};

// eBPF map：存储事件
struct {
    __uint(type, BPF_MAP_TYPE_RINGBUF);
    __uint(max_entries, 256 * 1024);
} events SEC(".maps");

// 挂载到 tcp_sendmsg
SEC("kprobe/tcp_sendmsg")
int trace_tcp_sendmsg(struct pt_regs *ctx) {
    struct network_event *event;
    event = bpf_ringbuf_reserve(&events, sizeof(*event), 0);
    if (!event) {
        return 0;
    }
    
    event->pid = bpf_get_current_pid_tgid() >> 32;
    event->timestamp = bpf_ktime_get_ns();
    __builtin_memcpy(event->event_type, "send", 4);
    
    bpf_ringbuf_submit(event, 0);
    return 0;
}

// 类似地实现 tcp_recvmsg, udp_sendmsg, udp_recvmsg
```

### 3. 编译 eBPF 程序

```bash
# 使用 clang 编译
clang -O2 -target bpf -c network_probe.bpf.c -o network_probe.bpf.o

# 或使用 libbpf-rs 的构建脚本
```

### 4. 用户空间程序

使用 libbpf-rs 加载和运行 eBPF 程序：

```rust
use libbpf_rs::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 加载 eBPF 程序
    let skel = NetworkProbeSkel::open()?;
    skel.load()?;
    skel.attach()?;
    
    // 读取事件并转换为 xctl 格式
    let ringbuf = skel.maps().events();
    // ... 事件循环
}
```

## 替代方案：基于 /proc/net 的探针

由于 eBPF 需要：
- Linux 内核 4.18+
- root 权限
- 复杂的编译和部署

我们提供了一个基于 `/proc/net` 的探针作为替代方案：

```bash
python3 examples/xctl-probe-network.py
```

这个探针：
- ✅ 无需 root 权限（读取 /proc）
- ✅ 跨 Linux 发行版兼容
- ✅ 易于调试和维护
- ❌ 性能开销略高（需要系统调用）
- ❌ 精度略低（基于采样）

## 性能对比

| 特性 | eBPF 探针 | /proc/net 探针 |
|------|-----------|----------------|
| 开销 | 极低（内核空间） | 中等（用户空间） |
| 精度 | 纳秒级 | 秒级 |
| 权限 | root/CAP_BPF | 普通用户 |
| 部署复杂度 | 高 | 低 |
| 实时性 | 实时 | 采样（1-2秒） |

## 推荐使用场景

- **生产环境（高性能）**: 使用 eBPF 探针
- **开发/测试**: 使用 /proc/net 探针
- **容器环境**: 根据权限选择

## 下一步

1. 完善 eBPF 程序实现
2. 添加 socket 延迟检测
3. 集成到 xctl 主程序（可选）
4. 添加单元测试和性能基准
