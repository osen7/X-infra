//! xctl-hub: 全局中控
//! 
//! 接收各节点的 WebSocket 连接，维护全局状态图
//! 提供跨节点的根因分析和集群级修复能力

use xctl_core::event::Event;
use xctl_core::graph::{StateGraph, NodeType};
use clap::Parser;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use warp::Filter;
use serde_json::json;

#[derive(Parser)]
#[command(name = "xctl-hub")]
#[command(about = "xctl 全局中控：集群级状态图和根因分析")]
struct Cli {
    /// WebSocket 监听地址
    #[arg(long, default_value = "0.0.0.0:8080")]
    ws_listen: String,
    /// HTTP API 监听地址
    #[arg(long, default_value = "0.0.0.0:8081")]
    http_listen: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    println!("🚀 xctl-hub 启动中...");
    println!("📡 WebSocket 监听地址: ws://{}", cli.ws_listen);
    println!("🌐 HTTP API 监听地址: http://{}", cli.http_listen);
    
    // 创建全局状态图
    let global_graph = Arc::new(StateGraph::new());
    
    // 启动 WebSocket 服务器
    let ws_listen = cli.ws_listen.clone();
    let ws_handle = {
        let graph = Arc::clone(&global_graph);
        tokio::spawn(async move {
            let listener = TcpListener::bind(&ws_listen).await?;
            println!("✅ WebSocket 服务器已启动，等待节点连接...");
            
            while let Ok((stream, addr)) = listener.accept().await {
                let graph = Arc::clone(&graph);
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, addr, graph).await {
                        eprintln!("[hub] 处理连接 {} 时出错: {}", addr, e);
                    }
                });
            }
            
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        })
    };
    
    // 启动 HTTP API 服务器
    let http_listen = cli.http_listen.clone();
    let http_handle = {
        let graph = Arc::clone(&global_graph);
        tokio::spawn(async move {
            let api = create_api_routes(graph);
            println!("✅ HTTP API 服务器已启动");
            warp::serve(api).run(([0, 0, 0, 0], http_listen.split(':').last().unwrap_or("8081").parse().unwrap_or(8081))).await;
        })
    };
    
    // 等待任一服务器退出
    tokio::select! {
        result = ws_handle => {
            if let Err(e) = result {
                eprintln!("[hub] WebSocket 服务器错误: {:?}", e);
            }
        }
        _ = http_handle => {
            println!("[hub] HTTP 服务器已关闭");
        }
    }
    
    Ok(())
}

/// 处理单个 WebSocket 连接
async fn handle_connection(
    stream: TcpStream,
    addr: std::net::SocketAddr,
    graph: Arc<StateGraph>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("[hub] 新节点连接: {}", addr);
    
    let ws_stream = accept_async(stream).await?;
    let (_write, mut read) = ws_stream.split();
    
    // 读取事件并更新全局图
    while let Some(msg) = read.next().await {
        match msg? {
            Message::Text(text) => {
                // 解析事件
                match serde_json::from_str::<Event>(&text) {
                    Ok(mut event) => {
                        // 确保 node_id 已设置（从连接地址推断，如果未设置）
                        if event.node_id.is_none() {
                            event.node_id = Some(format!("node-{}", addr.ip()));
                        }
                        
                        // 更新全局图
                        if let Err(e) = graph.process_event(&event).await {
                            eprintln!("[hub] 处理事件失败: {}", e);
                        } else {
                            println!("[hub] 收到事件: {:?} from {}", event.event_type, event.node_id.as_ref().unwrap_or(&"unknown".to_string()));
                        }
                    }
                    Err(e) => {
                        eprintln!("[hub] 解析事件失败: {}", e);
                    }
                }
            }
            Message::Close(_) => {
                println!("[hub] 节点 {} 断开连接", addr);
                break;
            }
            _ => {}
        }
    }
    
    Ok(())
}

/// 创建 HTTP API 路由
fn create_api_routes(
    graph: Arc<StateGraph>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let graph_clone = graph.clone();
    
    // GET /api/v1/why?job_id=xxx
    let why_route = warp::path("api")
        .and(warp::path("v1"))
        .and(warp::path("why"))
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and_then(move |params: std::collections::HashMap<String, String>| {
            let graph = Arc::clone(&graph_clone);
            async move {
                if let Some(job_id) = params.get("job_id") {
                    match cluster_why(graph, job_id).await {
                        Ok(causes) => Ok(warp::reply::json(&json!({
                            "job_id": job_id,
                            "causes": causes
                        }))),
                        Err(e) => Ok(warp::reply::json(&json!({
                            "error": e.to_string()
                        }))),
                    }
                } else {
                    Ok(warp::reply::json(&json!({
                        "error": "missing job_id parameter"
                    })))
                }
            }
        });
    
    // GET /api/v1/ps
    let ps_route = warp::path("api")
        .and(warp::path("v1"))
        .and(warp::path("ps"))
        .and_then(move || {
            let graph = Arc::clone(&graph);
            async move {
                let processes = graph.get_active_processes().await;
                let result: Vec<serde_json::Value> = processes
                    .iter()
                    .map(|node| {
                        json!({
                            "id": node.id,
                            "job_id": node.metadata.get("job_id").unwrap_or(&"-".to_string()),
                            "state": node.metadata.get("state").unwrap_or(&"unknown".to_string()),
                        })
                    })
                    .collect();
                Ok::<_, warp::Rejection>(warp::reply::json(&json!({
                    "processes": result
                })))
            }
        });
    
    why_route.or(ps_route)
}

/// 集群级根因分析：根据 job_id 查找所有相关进程并分析根因
async fn cluster_why(
    graph: Arc<StateGraph>,
    target_job_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let nodes = graph.nodes.read().await;
    let mut global_causes = Vec::new();
    
    // 1. 在全局图中找出所有属于这个 job_id 的进程节点
    let job_pids: Vec<String> = nodes
        .iter()
        .filter(|(_, n)| {
            n.node_type == NodeType::Process
                && n.metadata.get("job_id") == Some(&target_job_id.to_string())
        })
        .map(|(id, _)| id.clone())
        .collect();
    
    drop(nodes);
    
    if job_pids.is_empty() {
        return Ok(vec![format!("未找到 job_id={} 的进程", target_job_id)]);
    }
    
    // 2. 对每个进程节点，在全局图中发起根因分析
    for pid_id in job_pids {
        // 从节点 ID 中提取 PID（格式可能是 "node-a::pid-1234"）
        let pid = if let Some(pid_part) = pid_id.split("::").last() {
            pid_part.strip_prefix("pid-").and_then(|s| s.parse::<u32>().ok())
        } else {
            pid_id.strip_prefix("pid-").and_then(|s| s.parse::<u32>().ok())
        };
        
        if let Some(pid) = pid {
            let causes = graph.find_root_cause(pid).await;
            for cause in causes {
                // 添加节点信息到根因描述中
                let node_info = if pid_id.contains("::") {
                    format!("{}: {}", pid_id.split("::").next().unwrap_or("unknown"), cause)
                } else {
                    cause
                };
                global_causes.push(node_info);
            }
        }
    }
    
    // 3. 去重并返回全局根因
    global_causes.sort();
    global_causes.dedup();
    
    Ok(global_causes)
}
