use aya::{
    maps::perf::AsyncPerfEventArrayBuffer,
    programs::KProbe,
    util::online_cpus,
    Bpf,
};
use aya_log::BpfLogger;
use bytes::BytesMut;
use clap::Parser;
use log::{info, warn};
use std::convert::TryFrom;
use tokio::signal;
use xctl_probe_ebpf_ebpf::NetworkEvent;

/// xctl eBPF 网络探针
/// 监控 TCP 重传事件，输出 JSONL 格式给 xctl 核心
#[derive(Parser)]
#[command(name = "xctl-probe-ebpf")]
#[command(about = "eBPF 网络探针：监控 TCP 重传和丢包事件")]
struct Cli {
    /// 输出格式：jsonl（默认）或 debug
    #[arg(long, default_value = "jsonl")]
    format: String,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    
    let cli = Cli::parse();
    
    // 加载 eBPF 程序
    // 注意：实际运行时，eBPF 字节码应该从文件系统加载
    // 这里使用 include_bytes! 是为了简化部署，生产环境建议从文件加载
    let mut bpf = Bpf::load(include_bytes!(
        "../target/bpfel-unknown-none/release/xctl-probe-ebpf-ebpf"
    ))?;

    // 初始化日志
    if let Err(e) = BpfLogger::init(&mut bpf) {
        warn!("Failed to initialize eBPF logger: {}", e);
    }

    // 加载 kprobe 程序
    // 1. tcp_retransmit_skb：捕获 TCP 重传事件
    let program: &mut KProbe = bpf.program_mut("tcp_retransmit_skb").unwrap().try_into()?;
    program.load()?;
    program.attach("tcp_retransmit_skb", 0)?;
    info!("eBPF 程序已加载并附加到 tcp_retransmit_skb");

    // 2. tcp_sendmsg：建立 socket -> PID 映射
    let program2: &mut KProbe = bpf.program_mut("tcp_sendmsg").unwrap().try_into()?;
    program2.load()?;
    program2.attach("tcp_sendmsg", 0)?;
    info!("eBPF 程序已加载并附加到 tcp_sendmsg");

    // 获取 PerfEventArray
    let mut perf_array = bpf.take_map("NETWORK_EVENTS").unwrap();
    let perf_array = perf_array.try_into().unwrap();

    // 为每个 CPU 创建异步缓冲区
    let cpus = online_cpus().unwrap();
    let mut handles = Vec::new();

    for cpu_id in cpus {
        let mut buf = AsyncPerfEventArrayBuffer::new(perf_array.clone(), cpu_id)?;
        let format = cli.format.clone();
        
        let handle = tokio::spawn(async move {
            let mut buffers = (0..10)
                .map(|_| BytesMut::with_capacity(1024))
                .collect::<Vec<_>>();

            loop {
                let events = buf.read_events(&mut buffers).await;
                match events {
                    Ok(events) => {
                        for buf in buffers.iter().take(events.read) {
                            if let Ok(event) = parse_network_event(buf) {
                                output_event(&event, &format);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("读取 PerfEventArray 失败: {}", e);
                        break;
                    }
                }
            }
        });
        
        handles.push(handle);
    }

    info!("开始监控 TCP 重传事件...");
    info!("按 Ctrl+C 退出");

    // 等待退出信号
    signal::ctrl_c().await?;
    info!("收到退出信号，正在关闭...");

    // 取消所有任务
    for handle in handles {
        handle.abort();
    }

    Ok(())
}

/// 解析网络事件
fn parse_network_event(buf: &BytesMut) -> Result<NetworkEvent, anyhow::Error> {
    if buf.len() < core::mem::size_of::<NetworkEvent>() {
        return Err(anyhow::anyhow!("缓冲区太小"));
    }

    let event = unsafe {
        core::ptr::read(buf.as_ptr() as *const NetworkEvent)
    };

    Ok(event)
}

/// 输出事件（JSONL 格式）
fn output_event(event: &NetworkEvent, format: &str) {
    match format {
        "jsonl" => {
            // 构建 entity_id：优先使用 socket 四元组，否则使用 PID
            let entity_id = if event.socket_tuple.src_ip != 0 || event.socket_tuple.dst_ip != 0 {
                // 有 socket 信息，使用四元组构建 entity_id
                format!(
                    "network-{}-{}-{}-{}",
                    u32_to_ip_string(event.socket_tuple.src_ip),
                    u32_to_ip_string(event.socket_tuple.dst_ip),
                    u16::from_be(event.socket_tuple.src_port),
                    u16::from_be(event.socket_tuple.dst_port)
                )
            } else {
                // 没有 socket 信息，使用 PID（第一版）
                format!("network-pid-{}", event.pid)
            };
            
            // 输出标准 xctl 事件格式
            let json_event = serde_json::json!({
                "ts": event.timestamp / 1_000_000, // 转换为毫秒
                "event_type": "transport.drop",
                "entity_id": entity_id,
                "pid": event.pid,
                "value": event.retransmit_count.to_string(),
            });
            
            println!("{}", serde_json::to_string(&json_event).unwrap());
        }
        "debug" => {
            if event.socket_tuple.src_ip != 0 || event.socket_tuple.dst_ip != 0 {
                info!(
                    "TCP Retransmit: pid={}, socket={}:{}->{}:{}, count={}, ts={}",
                    event.pid,
                    u32_to_ip_string(event.socket_tuple.src_ip),
                    u16::from_be(event.socket_tuple.src_port),
                    u32_to_ip_string(event.socket_tuple.dst_ip),
                    u16::from_be(event.socket_tuple.dst_port),
                    event.retransmit_count,
                    event.timestamp
                );
            } else {
                info!(
                    "TCP Retransmit: pid={}, count={}, ts={} (第一版：无 socket 信息)",
                    event.pid, event.retransmit_count, event.timestamp
                );
            }
        }
        _ => {
            warn!("未知的输出格式: {}", format);
        }
    }
}

/// 将 u32 IP 地址转换为点分十进制字符串
fn u32_to_ip_string(ip: u32) -> String {
    if ip == 0 {
        "0.0.0.0".to_string()
    } else {
        format!(
            "{}.{}.{}.{}",
            (ip >> 24) & 0xFF,
            (ip >> 16) & 0xFF,
            (ip >> 8) & 0xFF,
            ip & 0xFF
        )
    }
}
