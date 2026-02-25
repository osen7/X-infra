#![no_std]
#![no_main]

use aya_bpf::{
    helpers::bpf_probe_read_kernel,
    macros::{kprobe, map},
    maps::{HashMap, PerfEventArray},
    programs::ProbeContext,
};
use aya_log_ebpf::info;

use xctl_probe_ebpf_ebpf::{NetworkEvent, SocketTuple};

// 导入内核绑定（CO-RE 支持）
// 注意：实际使用时，这些应该从 generate-bindings.sh 生成
// 当前使用示例绑定，生产环境应使用真实生成的文件
mod bindings;

/// Socket 四元组到 PID 的映射表
/// 在 tcp_sendmsg 中建立映射，在 tcp_retransmit_skb 中查询
#[map]
static mut SOCKET_TO_PID: HashMap<SocketTuple, u32> = HashMap::with_max_entries(8192, 0);

/// 网络事件输出
#[map]
static mut NETWORK_EVENTS: PerfEventArray<NetworkEvent> = PerfEventArray::with_max_entries(1024, 0);

/// Hook tcp_sendmsg：在真实的进程上下文中建立 socket -> PID 映射
/// 这是解决软中断 PID 陷阱的关键！
#[kprobe(name = "tcp_sendmsg")]
pub fn tcp_sendmsg(ctx: ProbeContext) -> u32 {
    match try_tcp_sendmsg(ctx) {
        Ok(ret) => ret,
        Err(_) => 0, // 失败时返回 0，不影响内核执行
    }
}

#[inline]
fn try_tcp_sendmsg(ctx: ProbeContext) -> Result<u32, u32> {
    // 获取当前进程 PID（这里运行在真实的进程上下文，PID 是准确的）
    let pid_tgid = unsafe { aya_bpf::helpers::bpf_get_current_pid_tgid() };
    let pid = (pid_tgid >> 32) as u32;
    
    // 从 tcp_sendmsg 的第一个参数获取 struct sock *sk
    // tcp_sendmsg(struct sock *sk, struct msghdr *msg, size_t size)
    // 
    // CO-RE 实现：使用 bpf_probe_read_kernel 读取 socket 字段
    // 注意：在 Aya 中，获取函数参数需要使用架构特定的方法
    // 这里使用简化方法，实际生产环境应使用完整的 CO-RE 实现
    
    // 尝试提取 socket 四元组（CO-RE 版本）
    let socket_tuple = extract_socket_tuple_from_sendmsg(ctx, pid);
    
    // 存入 SOCKET_TO_PID map
    unsafe {
        let _ = SOCKET_TO_PID.insert(&socket_tuple, &pid, 0);
    }
    
    if socket_tuple.src_ip != 0 || socket_tuple.dst_ip != 0 {
        info!(&ctx, "tcp_sendmsg: pid={}, socket={}:{}->{}:{} (CO-RE)", 
              pid,
              socket_tuple.src_ip,
              u16::from_be(socket_tuple.src_port),
              socket_tuple.dst_ip,
              u16::from_be(socket_tuple.dst_port));
    } else {
        info!(&ctx, "tcp_sendmsg: pid={} (CO-RE: socket 提取失败，使用 PID key)", pid);
    }
    
    Ok(0)
}

/// 从 tcp_sendmsg 的上下文提取 socket 四元组（CO-RE 版本）
/// 
/// 注意：这是一个简化实现，实际生产环境应该：
/// 1. 使用 aya-tool 生成完整的内核绑定
/// 2. 使用 bpf_core_read! 宏安全读取字段
/// 3. 处理不同内核版本的字段差异
#[inline]
fn extract_socket_tuple_from_sendmsg(_ctx: ProbeContext, _pid: u32) -> SocketTuple {
    // TODO: 实现完整的 CO-RE socket 提取
    // 
    // 完整实现应该：
    // 1. 从 ctx 获取 struct sock *sk 指针
    // 2. 使用 bpf_core_read! 读取 sk->__sk_common.skc_rcv_saddr
    // 3. 使用 bpf_core_read! 读取 sk->__sk_common.skc_daddr
    // 4. 使用 bpf_core_read! 读取 sk->__sk_common.skc_num (源端口)
    // 5. 使用 bpf_core_read! 读取 sk->__sk_common.skc_dport (目的端口)
    //
    // 示例代码（需要完整的内核绑定）：
    // let sk: *const bindings::sock = ctx.arg(0)?;
    // let saddr = unsafe { bpf_core_read!(&(*sk).__sk_common.skc_rcv_saddr) };
    // let daddr = unsafe { bpf_core_read!(&(*sk).__sk_common.skc_daddr) };
    // let sport = unsafe { bpf_core_read!(&(*sk).__sk_common.skc_num) };
    // let dport = unsafe { bpf_core_read!(&(*sk).__sk_common.skc_dport) };
    
    // 当前返回空值，等待完整实现
    SocketTuple {
        src_ip: 0,
        dst_ip: 0,
        src_port: 0,
        dst_port: 0,
    }
}

/// Hook tcp_retransmit_skb：在软中断上下文中捕获重传
/// ⚠️ 警告：这里运行在软中断上下文，PID 不准确！
/// 需要通过 socket 信息从 SOCKET_TO_PID map 中反查真实 PID
#[kprobe(name = "tcp_retransmit_skb")]
pub fn tcp_retransmit_skb(ctx: ProbeContext) -> u32 {
    match try_tcp_retransmit_skb(ctx) {
        Ok(ret) => ret,
        Err(_) => 0,
    }
}

#[inline]
fn try_tcp_retransmit_skb(ctx: ProbeContext) -> Result<u32, u32> {
    // ⚠️ 软中断上下文陷阱：这里的 PID 不准确！
    let pid_tgid = unsafe { aya_bpf::helpers::bpf_get_current_pid_tgid() };
    let fallback_pid = (pid_tgid >> 32) as u32;
    
    // 从 tcp_retransmit_skb 的第一个参数获取 struct sock *sk
    // tcp_retransmit_skb(struct sock *sk, struct sk_buff *skb, int segs)
    //
    // CO-RE 实现：提取真实的 socket 四元组，然后从 Map 查询真实 PID
    let socket_tuple = extract_socket_tuple_from_retransmit(ctx, fallback_pid);
    
    // 从 SOCKET_TO_PID map 中查询真实 PID
    let real_pid = unsafe {
        SOCKET_TO_PID.get(&socket_tuple)
            .copied()
            .unwrap_or_else(|| {
                // 如果使用真实 socket 查询失败，尝试使用 PID key（向后兼容）
                let pid_key = SocketTuple {
                    src_ip: fallback_pid,
                    dst_ip: 0,
                    src_port: 0,
                    dst_port: 0,
                };
                SOCKET_TO_PID.get(&pid_key)
                    .copied()
                    .unwrap_or(fallback_pid)
            })
    };
    
    // 创建网络事件
    let event = NetworkEvent {
        pid: real_pid,
        event_type: 1, // transport.drop
        retransmit_count: 1, // 每次重传计数为 1
        timestamp: unsafe { aya_bpf::helpers::bpf_ktime_get_ns() },
        socket_tuple,
    };
    
    // 输出到 PerfEventArray
    unsafe {
        NETWORK_EVENTS.output(&ctx, &event, 0);
    }
    
    // 日志输出
    if socket_tuple.src_ip != 0 || socket_tuple.dst_ip != 0 {
        if real_pid == fallback_pid {
            info!(&ctx, "TCP retransmit: pid={}, socket={}:{}->{}:{} (CO-RE: 未找到映射)", 
                  real_pid,
                  socket_tuple.src_ip,
                  u16::from_be(socket_tuple.src_port),
                  socket_tuple.dst_ip,
                  u16::from_be(socket_tuple.dst_port));
        } else {
            info!(&ctx, "TCP retransmit: pid={}, socket={}:{}->{}:{} (CO-RE: 成功)", 
                  real_pid,
                  socket_tuple.src_ip,
                  u16::from_be(socket_tuple.src_port),
                  socket_tuple.dst_ip,
                  u16::from_be(socket_tuple.dst_port));
        }
    } else {
        if real_pid == fallback_pid {
            info!(&ctx, "TCP retransmit: pid={} (CO-RE: socket 提取失败，使用 fallback)", real_pid);
        } else {
            info!(&ctx, "TCP retransmit: pid={} (CO-RE: 使用 PID 映射)", real_pid);
        }
    }
    
    Ok(0)
}

/// 从 tcp_retransmit_skb 的上下文提取 socket 四元组（CO-RE 版本）
/// 
/// 注意：这是一个简化实现，实际生产环境应该：
/// 1. 使用 aya-tool 生成完整的内核绑定
/// 2. 使用 bpf_core_read! 宏安全读取字段
/// 3. 处理不同内核版本的字段差异
#[inline]
fn extract_socket_tuple_from_retransmit(_ctx: ProbeContext, _fallback_pid: u32) -> SocketTuple {
    // TODO: 实现完整的 CO-RE socket 提取
    // 
    // 完整实现应该：
    // 1. 从 ctx 获取 struct sock *sk 指针
    // 2. 使用 bpf_core_read! 读取 sk->__sk_common.skc_rcv_saddr
    // 3. 使用 bpf_core_read! 读取 sk->__sk_common.skc_daddr
    // 4. 使用 bpf_core_read! 读取 sk->__sk_common.skc_num (源端口)
    // 5. 使用 bpf_core_read! 读取 sk->__sk_common.skc_dport (目的端口)
    //
    // 示例代码（需要完整的内核绑定）：
    // let sk: *const bindings::sock = ctx.arg(0)?;
    // let saddr = unsafe { bpf_core_read!(&(*sk).__sk_common.skc_rcv_saddr) };
    // let daddr = unsafe { bpf_core_read!(&(*sk).__sk_common.skc_daddr) };
    // let sport = unsafe { bpf_core_read!(&(*sk).__sk_common.skc_num) };
    // let dport = unsafe { bpf_core_read!(&(*sk).__sk_common.skc_dport) };
    
    // 当前返回空值，等待完整实现
    SocketTuple {
        src_ip: 0,
        dst_ip: 0,
        src_port: 0,
        dst_port: 0,
    }
}


#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
