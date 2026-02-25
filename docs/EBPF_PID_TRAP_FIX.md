# eBPF PID 陷阱修复指南

## ⚠️ 问题描述

### 软中断（SoftIRQ）PID 陷阱

在 Linux 内核中，`tcp_retransmit_skb`（TCP 重传）通常由定时器中断触发，运行在**软中断上下文（SoftIRQ context）**中，而不是用户进程上下文。

**问题**：当 Hook 被触发时，`bpf_get_current_pid_tgid()` 获取到的可能是：
- 恰好那一瞬间正在被 CPU 调度的任意进程的 PID
- 内核空闲线程的 PID 0
- **绝对不是真正拥有那个 TCP Socket 的训练任务的 PID！**

## 🔧 解决方案

### 方案 1：Socket 映射表（推荐）

使用 eBPF Map 建立 Socket 四元组到 PID 的映射：

1. **Hook `tcp_sendmsg`**（运行在真实进程上下文）
   - 使用 `bpf_get_current_pid_tgid()` 获取**准确的 PID**
   - 从 `struct sock *sk` 提取 Socket 四元组（源IP、目的IP、源端口、目的端口）
   - 存入 `HashMap<SocketTuple, u32>` Map

2. **Hook `tcp_retransmit_skb`**（运行在软中断上下文）
   - 从 `struct sock *sk` 提取 Socket 四元组
   - 从 Map 中反查**真实的 PID**
   - 使用真实 PID 输出事件

### 方案 2：第一版简化实现（当前）

由于提取 Socket 信息需要内核结构体偏移量（不同内核版本不同），第一版可以：
- 使用当前 PID（虽然可能不准确，但可以工作）
- 添加警告日志，提醒 PID 可能不准确
- 后续版本再完善 Socket 映射

## 📋 实现细节

### Socket 四元组结构

```rust
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SocketTuple {
    pub src_ip: u32,      // 源 IP（IPv4，网络字节序）
    pub dst_ip: u32,      // 目的 IP（IPv4，网络字节序）
    pub src_port: u16,    // 源端口（网络字节序）
    pub dst_port: u16,    // 目的端口（网络字节序）
}
```

### eBPF Map

```rust
#[map]
static mut SOCKET_TO_PID: HashMap<SocketTuple, u32> = HashMap::with_max_entries(8192, 0);
```

### tcp_sendmsg Hook（建立映射）

```rust
#[kprobe(name = "tcp_sendmsg")]
pub fn tcp_sendmsg(ctx: ProbeContext) -> u32 {
    // 1. 获取真实 PID（这里运行在进程上下文，PID 准确）
    let pid_tgid = unsafe { aya_bpf::helpers::bpf_get_current_pid_tgid() };
    let pid = (pid_tgid >> 32) as u32;
    
    // 2. 从 struct sock *sk 提取四元组
    // TODO: 需要内核结构体偏移量
    
    // 3. 存入 Map
    // unsafe { SOCKET_TO_PID.insert(&socket_tuple, &pid); }
    
    Ok(0)
}
```

### tcp_retransmit_skb Hook（查询映射）

```rust
#[kprobe(name = "tcp_retransmit_skb")]
pub fn tcp_retransmit_skb(ctx: ProbeContext) -> u32 {
    // 1. 从 struct sock *sk 提取四元组
    // TODO: 需要内核结构体偏移量
    
    // 2. 从 Map 查询真实 PID
    // let real_pid = unsafe { SOCKET_TO_PID.get(&socket_tuple) };
    
    // 3. 使用真实 PID 输出事件
    Ok(0)
}
```

## 🔍 内核结构体偏移量

提取 Socket 信息需要访问 `struct sock` 的成员：

```c
struct sock {
    // ... 其他成员
    __be16          sk_num;         // 源端口
    __be16          sk_dport;       // 目的端口
    __be32          sk_rcv_saddr;   // 源 IP
    __be32          sk_daddr;       // 目的 IP
    // ...
};
```

**问题**：不同内核版本的偏移量不同，需要：
1. 使用 BTF (BPF Type Format) 自动获取偏移量
2. 或手动维护不同内核版本的偏移量表

## 🚀 当前实现状态

### ✅ 已完成

- [x] 添加 `SocketTuple` 结构体定义
- [x] 添加 `SOCKET_TO_PID` Map 定义
- [x] 添加 `tcp_sendmsg` Hook 框架
- [x] 在 `tcp_retransmit_skb` 中添加警告日志
- [x] 完善图引擎的 `handle_transport_event` 逻辑

### ⚠️ 待完善

- [ ] 实现 Socket 四元组提取（需要内核偏移量）
- [ ] 实现 Map 的插入和查询逻辑
- [ ] 测试 PID 准确性
- [ ] 处理 Map 溢出（LRU 淘汰）

## 🎯 验证方法

### 测试 PID 准确性

```bash
# 1. 启动一个已知 PID 的网络进程
python3 -c "import socket; s=socket.socket(); s.connect(('8.8.8.8', 80))" &
KNOWN_PID=$!

# 2. 运行 eBPF 探针
sudo ./xctl-probe-ebpf/target/release/xctl-probe-ebpf

# 3. 触发网络重传（模拟网络拥塞）
# 4. 检查输出中的 PID 是否匹配 KNOWN_PID
```

### 预期结果

- **第一版**：PID 可能不准确，但事件能正常输出
- **完善版**：PID 应该准确匹配拥有 Socket 的进程

## 📚 参考资料

- [Linux 内核网络栈](https://www.kernel.org/doc/html/latest/networking/)
- [eBPF 内核结构体访问](https://aya-rs.dev/book/programs/kprobe.html)
- [BTF 和 CO-RE](https://nakryiko.com/posts/bpf-core-reference-guide/)

## 💡 建议

1. **第一版**：先使用当前 PID，确保整体流程打通
2. **第二版**：实现 Socket 映射，提高 PID 准确性
3. **生产版**：使用 BTF/CO-RE 自动适配不同内核版本
