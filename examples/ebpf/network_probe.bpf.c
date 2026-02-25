// eBPF 网络探针程序
// 监控 TCP 重传和网络延迟
//
// 编译: clang -O2 -target bpf -c network_probe.bpf.c -o network_probe.bpf.o
// 加载: 使用 libbpf 或 bpf() 系统调用加载

#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

// 定义事件结构
struct network_event {
    u32 pid;
    u64 timestamp;
    u32 event_type;  // 0=retransmit, 1=stall, 2=drop
    u64 bytes;
    char ifname[16];
};

// Ring buffer 用于向用户空间传递事件
struct {
    __uint(type, BPF_MAP_TYPE_RINGBUF);
    __uint(max_entries, 256 * 1024);  // 256KB
} events SEC(".maps");

// 监控 TCP 重传
SEC("kprobe/tcp_retransmit_skb")
int trace_tcp_retransmit(struct pt_regs *ctx) {
    struct network_event *event;
    
    event = bpf_ringbuf_reserve(&events, sizeof(*event), 0);
    if (!event) {
        return 0;
    }
    
    event->pid = bpf_get_current_pid_tgid() >> 32;
    event->timestamp = bpf_ktime_get_ns();
    event->event_type = 0;  // retransmit
    event->bytes = 0;
    
    bpf_ringbuf_submit(event, 0);
    return 0;
}

// 监控网络延迟（通过 tcp_sendmsg）
SEC("kprobe/tcp_sendmsg")
int trace_tcp_sendmsg(struct pt_regs *ctx) {
    // 记录发送时间，用于计算延迟
    // 实际实现需要更复杂的逻辑
    return 0;
}

// 监控网络丢包
SEC("kprobe/__skb_drop")
int trace_skb_drop(struct pt_regs *ctx) {
    struct network_event *event;
    
    event = bpf_ringbuf_reserve(&events, sizeof(*event), 0);
    if (!event) {
        return 0;
    }
    
    event->pid = bpf_get_current_pid_tgid() >> 32;
    event->timestamp = bpf_ktime_get_ns();
    event->event_type = 2;  // drop
    event->bytes = 0;
    
    bpf_ringbuf_submit(event, 0);
    return 0;
}

char LICENSE[] SEC("license") = "Dual BSD/GPL";
