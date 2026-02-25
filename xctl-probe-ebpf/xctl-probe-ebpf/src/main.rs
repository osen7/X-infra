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
    let program: &mut KProbe = bpf.program_mut("tcp_retransmit_skb").unwrap().try_into()?;
    program.load()?;
    program.attach("tcp_retransmit_skb", 0)?;

    info!("eBPF 程序已加载并附加到 tcp_retransmit_skb");

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
            // 输出标准 xctl 事件格式
            let json_event = serde_json::json!({
                "ts": event.timestamp / 1_000_000, // 转换为毫秒
                "event_type": "transport.drop",
                "entity_id": format!("network-pid-{}", event.pid),
                "pid": event.pid,
                "value": event.retransmit_count.to_string(),
            });
            
            println!("{}", serde_json::to_string(&json_event).unwrap());
        }
        "debug" => {
            info!(
                "TCP Retransmit: pid={}, count={}, ts={}",
                event.pid, event.retransmit_count, event.timestamp
            );
        }
        _ => {
            warn!("未知的输出格式: {}", format);
        }
    }
}
