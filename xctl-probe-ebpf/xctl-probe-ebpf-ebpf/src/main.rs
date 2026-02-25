#![no_std]
#![no_main]

use aya_bpf::{
    macros::{kprobe, map},
    maps::{HashMap, PerfEventArray},
    programs::ProbeContext,
};
use aya_log_ebpf::info;

use xctl_probe_ebpf_ebpf::{NetworkEvent, SocketTuple};

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
    // 注意：这里需要根据内核版本调整偏移量
    // 为了简化，我们暂时跳过 socket 信息提取
    // 实际实现中需要：
    // 1. 读取 sk->sk_rcv_saddr (源 IP)
    // 2. 读取 sk->sk_daddr (目的 IP)
    // 3. 读取 sk->sk_num (源端口)
    // 4. 读取 sk->sk_dport (目的端口)
    
    // TODO: 提取 socket 四元组并存入 SOCKET_TO_PID map
    // 由于需要内核结构体偏移量，这里先留空
    // 第一版可以先使用当前 PID（虽然不完美，但可以工作）
    
    info!(&ctx, "tcp_sendmsg: pid={}", pid);
    
    Ok(0)
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
    // 我们先用当前 PID 作为占位符，后续通过 socket 映射改进
    let pid_tgid = unsafe { aya_bpf::helpers::bpf_get_current_pid_tgid() };
    let current_pid = (pid_tgid >> 32) as u32;
    
    // TODO: 从 tcp_retransmit_skb 的参数中提取 socket 信息
    // 然后从 SOCKET_TO_PID map 中查询真实 PID
    // 第一版先使用 current_pid（虽然可能不准确，但可以工作）
    let pid = current_pid;
    
    // 创建空的 socket tuple（第一版暂不提取）
    let socket_tuple = SocketTuple {
        src_ip: 0,
        dst_ip: 0,
        src_port: 0,
        dst_port: 0,
    };
    
    // 创建网络事件
    let event = NetworkEvent {
        pid,
        event_type: 1, // transport.drop
        retransmit_count: 1, // 每次重传计数为 1
        timestamp: unsafe { aya_bpf::helpers::bpf_ktime_get_ns() },
        socket_tuple,
    };
    
    // 输出到 PerfEventArray
    unsafe {
        NETWORK_EVENTS.output(&ctx, &event, 0);
    }
    
    info!(&ctx, "TCP retransmit: pid={} (⚠️ 可能不准确，需要 socket 映射)", pid);
    
    Ok(0)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
