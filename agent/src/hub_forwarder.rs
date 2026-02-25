//! Hub 事件转发器：将本地重要事件推送到 Hub
//! 
//! 实现边缘折叠（Edge Roll-up）逻辑：
//! - 只推送错误事件、进程状态变化、触发规则的事件
//! - 过滤高频波动（如 gpu.util 的微小变化）

use xctl_core::event::{Event, EventType};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json;

/// Hub 事件转发器
pub struct HubForwarder {
    hub_url: String,
    node_id: String,
    ws_sender: Option<Arc<RwLock<Option<futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>>>>>,
}

impl HubForwarder {
    /// 创建新的 Hub 转发器
    pub fn new(hub_url: String, node_id: String) -> Self {
        Self {
            hub_url,
            node_id,
            ws_sender: None,
            forwarded_bindings: Arc::new(RwLock::new(HashSet::new())),
            last_util_values: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// 连接到 Hub WebSocket 服务器
    pub async fn connect(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let url = url::Url::parse(&self.hub_url)?;
        let (ws_stream, _) = connect_async(url).await?;
        let (write, _read) = ws_stream.split();
        
        let sender = Arc::new(RwLock::new(Some(write)));
        self.ws_sender = Some(sender);
        
        println!("[hub-forwarder] 已连接到 Hub: {}", self.hub_url);
        Ok(())
    }

    /// 判断事件是否应该推送到 Hub（边缘折叠逻辑）
    pub async fn should_forward(&self, event: &Event) -> bool {
        match event.event_type {
            // 错误事件：必须推送
            EventType::ErrorHw | EventType::ErrorNet => true,
            
            // 进程状态变化：必须推送
            EventType::ProcessState => true,
            
            // 网络丢包：必须推送（重要阻塞信号）
            EventType::TransportDrop => true,
            
            // 拓扑降级：必须推送
            EventType::TopoLinkDown => true,
            
            // 计算资源事件：只在建立新绑定或利用率剧烈变化时推送
            EventType::ComputeUtil | EventType::ComputeMem => {
                if let Some(pid) = event.pid {
                    let binding_key = (pid, event.entity_id.clone());
                    
                    // 检查是否是新的资源绑定（第一次推送）
                    let mut bindings = self.forwarded_bindings.write().await;
                    if !bindings.contains(&binding_key) {
                        // 新绑定，标记为已推送并允许推送
                        bindings.insert(binding_key.clone());
                        // 记录当前利用率
                        if let Ok(util) = event.value.parse::<f64>() {
                            self.last_util_values.write().await.insert(binding_key, util);
                        }
                        return true;
                    }
                    
                    // 检查利用率是否发生剧烈变化（从 >80% 跌到 <1%，或从 <1% 升到 >80%）
                    if let Ok(current_util) = event.value.parse::<f64>() {
                        let mut last_utils = self.last_util_values.write().await;
                        if let Some(&last_util) = last_utils.get(&binding_key) {
                            // 检测剧烈变化：高->低 或 低->高
                            if (last_util > 80.0 && current_util < 1.0) || 
                               (last_util < 1.0 && current_util > 80.0) {
                                last_utils.insert(binding_key, current_util);
                                return true;
                            }
                        }
                    }
                }
                false
            }
            
            // 存储事件：类似计算资源，只在建立新绑定或剧烈变化时推送
            EventType::StorageIops | EventType::StorageQDepth => {
                if let Some(pid) = event.pid {
                    let binding_key = (pid, event.entity_id.clone());
                    let mut bindings = self.forwarded_bindings.write().await;
                    if !bindings.contains(&binding_key) {
                        bindings.insert(binding_key);
                        return true;
                    }
                }
                false
            }
            
            // 传输带宽：只在建立新绑定时推送
            EventType::TransportBw => {
                if let Some(pid) = event.pid {
                    let binding_key = (pid, event.entity_id.clone());
                    let mut bindings = self.forwarded_bindings.write().await;
                    if !bindings.contains(&binding_key) {
                        bindings.insert(binding_key);
                        return true;
                    }
                }
                false
            }
            
            // 其他事件：不推送（高频波动，由 Hub 通过查询获取）
            _ => false,
        }
    }

    /// 推送事件到 Hub
    pub async fn forward_event(&self, mut event: Event) -> Result<(), Box<dyn std::error::Error>> {
        // 注入 node_id
        event.node_id = Some(self.node_id.clone());
        
        // 序列化为 JSON
        let json = serde_json::to_string(&event)?;
        
        // 发送到 WebSocket
        if let Some(ref sender_arc) = self.ws_sender {
            let mut sender = sender_arc.write().await;
            if let Some(ref mut ws_sender) = *sender {
                ws_sender.send(Message::Text(json)).await?;
                return Ok(());
            }
        }
        
        Err("WebSocket 连接未建立".into())
    }
}

/// 获取当前节点 ID（使用 hostname）
pub fn get_node_id() -> String {
    use std::process::Command;
    
    // 尝试获取 hostname
    if let Ok(output) = Command::new("hostname").output() {
        if let Ok(hostname) = String::from_utf8(output.stdout) {
            return hostname.trim().to_string();
        }
    }
    
    // 回退到环境变量
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown-node".to_string())
}
