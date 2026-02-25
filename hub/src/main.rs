//! xctl-hub: 全局中控
//! 
//! 接收各节点的 WebSocket 连接，维护全局状态图
//! 提供跨节点的根因分析和集群级修复能力

use xctl_core::event::Event;
use xctl_core::graph::StateGraph;
use clap::Parser;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Parser)]
#[command(name = "xctl-hub")]
#[command(about = "xctl 全局中控：集群级状态图和根因分析")]
struct Cli {
    /// WebSocket 监听地址
    #[arg(long, default_value = "0.0.0.0:8080")]
    listen: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    println!("🚀 xctl-hub 启动中...");
    println!("📡 监听地址: ws://{}", cli.listen);
    
    // 创建全局状态图
    let global_graph = Arc::new(StateGraph::new());
    
    // TODO: 启动 WebSocket 服务器
    // TODO: 接收节点连接和事件
    // TODO: 维护全局图状态
    // TODO: 提供集群级查询接口
    
    println!("✅ xctl-hub 已启动（功能开发中）");
    
    // 保持运行
    tokio::signal::ctrl_c().await?;
    println!("收到退出信号，正在关闭...");
    
    Ok(())
}
