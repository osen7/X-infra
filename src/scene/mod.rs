mod types;
mod analyzer;
mod gpu_oom;
mod network_stall;
mod process_crash;

pub use types::{SceneType, AnalysisResult};
pub use analyzer::{SceneAnalyzer, SceneRegistry};
pub use gpu_oom::GpuOomAnalyzer;
pub use network_stall::NetworkStallAnalyzer;
pub use process_crash::ProcessCrashAnalyzer;

use crate::graph::StateGraph;

/// 场景识别器
pub struct SceneIdentifier {
    registry: SceneRegistry,
}

impl SceneIdentifier {
    pub fn new() -> Self {
        let mut registry = SceneRegistry::new();
        
        // 注册所有场景分析器
        registry.register(GpuOomAnalyzer);
        registry.register(NetworkStallAnalyzer);
        registry.register(ProcessCrashAnalyzer);
        
        Self { registry }
    }

    /// 识别场景类型
    pub async fn identify_scene(
        &self,
        graph: &StateGraph,
        pid: u32,
    ) -> Option<SceneType> {
        let pid_str = format!("pid-{}", pid);
        let edges = graph.get_all_edges_async().await;
        let nodes = graph.get_nodes_async().await;

        // 检查 GPU 相关错误
        for edge in &edges {
            if edge.from == pid_str && edge.edge_type == crate::graph::EdgeType::BlockedBy {
                if let Some(node) = nodes.get(&edge.to) {
                    if node.id.starts_with("gpu-") || node.id.contains("gpu") {
                        if let Some(error_type) = node.metadata.get("error_type") {
                            if error_type.contains("OOM") || error_type.contains("out of memory") {
                                return Some(SceneType::GpuOom);
                            }
                            if error_type.contains("error") || error_type.contains("XID") {
                                return Some(SceneType::GpuError);
                            }
                        }
                    }
                }
            }
        }

        // 检查网络阻塞
        for edge in &edges {
            if edge.from == pid_str && edge.edge_type == crate::graph::EdgeType::WaitsOn {
                if edge.to.starts_with("network-") || edge.to.contains("net") {
                    return Some(SceneType::NetworkStall);
                }
            }
        }

        // 检查进程状态
        if let Some(node) = nodes.get(&pid_str) {
            if let Some(state) = node.metadata.get("state") {
                if state == "exit" || state == "crash" || state == "failed" {
                    return Some(SceneType::ProcessCrash);
                }
                if state == "blocked" || state == "waiting" {
                    return Some(SceneType::ProcessBlocked);
                }
            }
        }

        None
    }

    /// 使用场景分析器分析
    pub async fn analyze_scene(
        &self,
        scene: SceneType,
        graph: &StateGraph,
        pid: u32,
    ) -> Option<AnalysisResult> {
        let pid_str = format!("pid-{}", pid);
        
        if let Some(analyzer) = self.registry.get_analyzer(scene) {
            Some(analyzer.analyze(graph, &pid_str).await)
        } else {
            None
        }
    }
}

impl Default for SceneIdentifier {
    fn default() -> Self {
        Self::new()
    }
}
