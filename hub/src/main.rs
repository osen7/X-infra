//! ark-hub: 全局中控
//! 
//! 接收各节点的 WebSocket 连接，维护全局状态图
//! 提供跨节点的根因分析和集群级修复能力

use ark_core::event::Event;
use ark_core::graph::{StateGraph, NodeType};
use clap::Parser;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream, MaybeTlsStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use warp::Filter;
use serde_json::json;
use dashmap::DashMap;
mod metrics;
mod k8s_controller;
use metrics::HubMetricsCollector;
use k8s_controller::K8sController;

#[derive(Parser)]
#[command(name = "ark-hub")]
#[command(about = "Ark 全局中控：集群级状态图和根因分析")]
struct Cli {
    /// WebSocket 监听地址
    #[arg(long, default_value = "0.0.0.0:8080")]
    ws_listen: String,
    /// HTTP API 监听地址
    #[arg(long, default_value = "0.0.0.0:8081")]
    http_listen: String,
    /// 启用 Kubernetes 控制器（自动打污点和驱逐 Pod）
    #[arg(long)]
    enable_k8s_controller: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    println!("🚀 ark-hub 启动中...");
    println!("📡 WebSocket 监听地址: ws://{}", cli.ws_listen);
    println!("🌐 HTTP API 监听地址: http://{}", cli.http_listen);
    
    // 创建全局状态图
    let global_graph = Arc::new(StateGraph::new());
    
    // 创建 Metrics 收集器
    let metrics = Arc::new(HubMetricsCollector::new()?);
    
    // 创建 K8s 控制器（如果启用）
    let k8s_controller = if cli.enable_k8s_controller {
        match K8sController::new(true).await {
            Ok(controller) => {
                println!("✅ Kubernetes 控制器已启用");
                Some(Arc::new(controller))
            }
            Err(e) => {
                eprintln!("⚠️  无法初始化 Kubernetes 控制器: {}", e);
                eprintln!("   继续运行，但不会执行自动节点隔离操作");
                None
            }
        }
    } else {
        println!("ℹ️  Kubernetes 控制器未启用（使用 --enable-k8s-controller 启用）");
        None
    };
    
    // 创建 WebSocket 连接管理器（node_id -> sender）
    let connections: Arc<DashMap<String, mpsc::UnboundedSender<Message>>> = Arc::new(DashMap::new());
    
    // 启动 WebSocket 服务器
    let ws_listen = cli.ws_listen.clone();
    let ws_handle = {
        let graph = Arc::clone(&global_graph);
        let conns = Arc::clone(&connections);
        let k8s_ctrl = k8s_controller.clone();
        tokio::spawn(async move {
            let listener = TcpListener::bind(&ws_listen).await?;
            println!("✅ WebSocket 服务器已启动，等待节点连接...");
            
            while let Ok((stream, addr)) = listener.accept().await {
                let graph = Arc::clone(&graph);
                let conns = Arc::clone(&conns);
                let k8s_ctrl = k8s_ctrl.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, addr, graph, conns, k8s_ctrl).await {
                        eprintln!("[hub] 处理连接 {} 时出错: {}", addr, e);
                    }
                });
            }
            
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        })
    };
    
    // 启动指标更新任务（每 5 秒更新一次）
    let metrics_update_handle = {
        let graph = Arc::clone(&global_graph);
        let metrics = Arc::clone(&metrics);
        let connections = Arc::clone(&connections);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                metrics.update_graph_metrics(&graph).await;
                // 更新 WebSocket 连接数
                let connected = connections.len();
                metrics.update_websocket_connections(connected, 0);
            }
        })
    };
    
    // 启动 HTTP API 服务器
    let http_listen = cli.http_listen.clone();
    let http_handle = {
        let graph = Arc::clone(&global_graph);
        let conns = Arc::clone(&connections);
        let metrics = Arc::clone(&metrics);
        tokio::spawn(async move {
            // 创建 API 路由（包含 metrics 端点）
            let api = create_api_routes(graph, conns, metrics);
            println!("✅ HTTP API 服务器已启动");
            let port = http_listen.split(':').last().unwrap_or("8081").parse().unwrap_or(8081);
            println!("📊 Prometheus Metrics 端点: http://0.0.0.0:{}/metrics", port);
            warp::serve(api).run(([0, 0, 0, 0], port)).await;
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
    connections: Arc<DashMap<String, mpsc::UnboundedSender<Message>>>,
    k8s_controller: Option<Arc<K8sController>>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("[hub] 新节点连接: {}", addr);
    
    let ws_stream = accept_async(stream).await?;
    let (mut write, mut read) = ws_stream.split();
    
    // 创建用于发送消息的通道
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    
    // 从连接地址生成默认 node_id（Agent 会在第一个事件中提供真实的 node_id）
    let mut node_id = format!("node-{}", addr.ip());
    
    // 立即注册连接（使用默认 node_id，后续可能被事件中的 node_id 更新）
    connections.insert(node_id.clone(), tx.clone());
    println!("[hub] 注册节点连接: {} (临时)", node_id);
    
    // 启动消息转发任务（从通道转发到 WebSocket write 端）
    let write_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Err(e) = write.send(msg).await {
                eprintln!("[hub] 发送消息失败: {}", e);
                break;
            }
        }
    });
    
    // 读取事件并更新全局图
    while let Some(msg) = read.next().await {
        match msg? {
            Message::Text(text) => {
                // 解析事件
                match serde_json::from_str::<Event>(&text) {
                    Ok(mut event) => {
                        // 如果事件中包含 node_id，使用它并更新连接表
                        if let Some(event_node_id) = &event.node_id {
                            if *event_node_id != node_id {
                                // node_id 发生变化，更新连接表
                                connections.remove(&node_id);
                                node_id = event_node_id.clone();
                                connections.insert(node_id.clone(), tx.clone());
                                println!("[hub] 更新节点连接: {}", node_id);
                            }
                        } else {
                            // 事件中没有 node_id，使用默认值
                            event.node_id = Some(node_id.clone());
                        }
                        
                        // 更新全局图
                        if let Err(e) = graph.process_event(&event).await {
                            eprintln!("[hub] 处理事件失败: {}", e);
                        } else {
                            println!("[hub] 收到事件: {:?} from {}", event.event_type, node_id);
                            
                            // 检测不可逆故障并触发 K8s 操作
                            if let Some(ref controller) = k8s_controller {
                                if let Some(fault) = controller.detect_irreversible_fault(&event) {
                                    // 在后台任务中处理故障（避免阻塞事件处理）
                                    let controller_clone = Arc::clone(controller);
                                    tokio::spawn(async move {
                                        if let Err(e) = controller_clone.handle_irreversible_fault(&fault).await {
                                            eprintln!("[k8s-controller] 处理故障失败: {}", e);
                                        }
                                    });
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[hub] 解析事件失败: {}", e);
                    }
                }
            }
            Message::Close(_) => {
                println!("[hub] 节点 {} 断开连接", node_id);
                break;
            }
            _ => {}
        }
    }
    
    // 从连接表中移除
    connections.remove(&node_id);
    println!("[hub] 节点 {} 已从连接表移除", node_id);
    
    // 等待写任务结束
    write_task.abort();
    
    Ok(())
}

/// Warp Filter：注入 StateGraph 状态
fn with_graph(
    graph: Arc<StateGraph>,
) -> impl Filter<Extract = (Arc<StateGraph>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || graph.clone())
}

/// Warp Filter：注入连接管理器
fn with_connections(
    connections: Arc<DashMap<String, mpsc::UnboundedSender<Message>>>,
) -> impl Filter<Extract = (Arc<DashMap<String, mpsc::UnboundedSender<Message>>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || connections.clone())
}

/// Fix 请求结构
#[derive(serde::Deserialize)]
struct FixRequest {
    node_id: String,
    target_pid: u32,
    action: Option<String>, // 可选，默认 "GracefulShutdown"
}

/// Warp Filter：注入 Metrics 收集器
fn with_metrics(
    metrics: Arc<HubMetricsCollector>,
) -> impl Filter<Extract = (Arc<HubMetricsCollector>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || metrics.clone())
}

/// 创建 HTTP API 路由
fn create_api_routes(
    graph: Arc<StateGraph>,
    connections: Arc<DashMap<String, mpsc::UnboundedSender<Message>>>,
    metrics: Arc<HubMetricsCollector>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let graph_filter = with_graph(graph.clone());
    let conns_filter = with_connections(connections.clone());
    let metrics_filter = with_metrics(metrics.clone());
    
    // GET /metrics - Prometheus Metrics 端点
    let metrics_route = warp::path("metrics")
        .and(warp::get())
        .and(metrics_filter.clone())
        .and_then(|metrics: Arc<HubMetricsCollector>| async move {
            match metrics.gather() {
                Ok(body) => Ok(warp::reply::with_header(
                    body,
                    "content-type",
                    "text/plain; version=0.0.4",
                )),
                Err(e) => {
                    eprintln!("[hub-metrics] 收集指标失败: {}", e);
                    Ok(warp::reply::with_status(
                        format!("Error: {}", e),
                        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                    ))
                }
            }
        });
    
    // GET /api/v1/why?job_id=xxx
    let why_route = warp::path!("api" / "v1" / "why")
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and(graph_filter.clone())
        .and_then(
            |params: std::collections::HashMap<String, String>, graph: Arc<StateGraph>| async move {
                if let Some(job_id) = params.get("job_id") {
                    match cluster_why(graph, job_id).await {
                        Ok((causes, processes)) => Ok(warp::reply::json(&json!({
                            "job_id": job_id,
                            "causes": causes,
                            "processes": processes
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
            },
        );
    
    // GET /api/v1/ps
    let ps_route = warp::path!("api" / "v1" / "ps")
        .and(graph_filter.clone())
        .and_then(|graph: Arc<StateGraph>| async move {
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
        });
    
    // POST /api/v1/fix
    let fix_route = warp::path!("api" / "v1" / "fix")
        .and(warp::post())
        .and(warp::body::json())
        .and(conns_filter)
        .and_then(|req: FixRequest, conns: Arc<DashMap<String, mpsc::UnboundedSender<Message>>>| async move {
            // 查找节点连接
            if let Some(sender) = conns.get(&req.node_id) {
                // 构建命令 JSON
                let command = json!({
                    "intent": "fix",
                    "target_pid": req.target_pid,
                    "action": req.action.as_ref().unwrap_or(&"GracefulShutdown".to_string())
                });
                
                // 发送命令
                if let Ok(json_str) = serde_json::to_string(&command) {
                    if sender.send(Message::Text(json_str)).is_ok() {
                        Ok(warp::reply::json(&json!({
                            "success": true,
                            "message": format!("命令已发送到节点 {}", req.node_id)
                        })))
                    } else {
                        Ok(warp::reply::with_status(
                            warp::reply::json(&json!({
                                "error": "发送命令失败：连接已关闭"
                            })),
                            warp::http::StatusCode::INTERNAL_SERVER_ERROR
                        ))
                    }
                } else {
                    Ok(warp::reply::with_status(
                        warp::reply::json(&json!({
                            "error": "序列化命令失败"
                        })),
                        warp::http::StatusCode::INTERNAL_SERVER_ERROR
                    ))
                }
            } else {
                Ok(warp::reply::with_status(
                    warp::reply::json(&json!({
                        "error": format!("节点 {} 未连接", req.node_id)
                    })),
                    warp::http::StatusCode::NOT_FOUND
                ))
            }
        });
    
    metrics_route.or(why_route).or(ps_route).or(fix_route)
}

/// 集群级根因分析：根据 job_id 查找所有相关进程并分析根因
async fn cluster_why(
    graph: Arc<StateGraph>,
    target_job_id: &str,
) -> Result<(Vec<String>, Vec<serde_json::Value>), Box<dyn std::error::Error>> {
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
        return Ok((vec![format!("未找到 job_id={} 的进程", target_job_id)], Vec::new()));
    }
    
    // 2. 构建进程列表（用于 CLI 提取节点和 PID）
    let mut process_list = Vec::new();
    
    // 3. 对每个进程节点，在全局图中发起根因分析
    // 直接使用完整的节点 ID（包含命名空间），避免命名空间丢失
    for pid_id in &job_pids {
        // 提取节点 ID 和 PID 并添加到进程列表
        if pid_id.contains("::") {
            let parts: Vec<&str> = pid_id.split("::").collect();
            let node_id = parts[0].to_string();
            if let Some(pid_part) = parts.get(1) {
                if let Some(pid_str) = pid_part.strip_prefix("pid-") {
                    if let Ok(pid) = pid_str.parse::<u32>() {
                        process_list.push(json!({
                            "node_id": node_id,
                            "pid": pid,
                            "node_id_full": pid_id
                        }));
                    }
                }
            }
        }
        
        let causes = graph.find_root_cause_by_id(pid_id).await;
        for cause in causes {
            // 添加节点信息到根因描述中
            let node_info = if pid_id.contains("::") {
                let node_name = pid_id.split("::").next().unwrap_or("unknown");
                format!("{}: {}", node_name, cause)
            } else {
                cause
            };
            global_causes.push(node_info);
        }
    }
    
    // 4. 去重并返回全局根因和进程列表
    global_causes.sort();
    global_causes.dedup();
    
    Ok((global_causes, process_list))
}
