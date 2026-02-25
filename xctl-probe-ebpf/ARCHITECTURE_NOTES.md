# eBPF 探针架构设计说明

## ⚠️ 重要提示：PID 陷阱

### 问题

`tcp_retransmit_skb` 运行在**软中断上下文**，`bpf_get_current_pid_tgid()` 获取的 PID **不准确**！

### 当前实现

第一版使用当前 PID（可能不准确），但可以工作。已添加警告日志。

### 未来改进

使用 Socket 映射表：
1. Hook `tcp_sendmsg`（进程上下文）建立 Socket -> PID 映射
2. Hook `tcp_retransmit_skb`（软中断上下文）通过 Socket 反查 PID

详见：`../docs/EBPF_PID_TRAP_FIX.md`

## 🎯 设计原则

1. **零侵入**：不需要修改业务代码
2. **高性能**：eBPF 在内核态执行
3. **实时性**：事件延迟 < 1ms
4. **可扩展**：支持多 CPU 并发

## 📊 数据流

```
内核 TCP 重传
    ↓
tcp_retransmit_skb (软中断上下文)
    ↓
eBPF Hook 捕获
    ↓
PerfEventArray (RingBuffer)
    ↓
用户态程序读取
    ↓
JSONL 输出
    ↓
xctl 事件总线
    ↓
状态图建立 WaitsOn 边
    ↓
规则引擎触发
```

## 🔧 技术细节

### 内核态

- **Hook 点**：`tcp_retransmit_skb`
- **数据采集**：PID（当前版本可能不准确）、时间戳
- **输出**：`PerfEventArray`

### 用户态

- **加载**：eBPF 字节码
- **附加**：kprobe 到内核函数
- **读取**：异步读取 `PerfEventArray`
- **输出**：JSONL 格式

## 🚀 使用方式

```bash
# 构建
./build.sh

# 运行（需要 root）
sudo ./xctl-probe-ebpf/target/release/xctl-probe-ebpf

# 集成到 xctl
xctl run --probe ./xctl-probe-ebpf/target/release/xctl-probe-ebpf
```
