/// 场景类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneType {
    // GPU 相关
    GpuOom,              // GPU OOM
    GpuUtilLow,          // GPU 利用率低
    GpuError,            // GPU 硬件错误
    
    // 网络相关
    NetworkStall,        // 网络阻塞
    NetworkDrop,         // 网络丢包
    
    // 存储相关
    StorageIoError,      // 存储 IO 错误
    StorageSlow,         // 存储慢
    
    // 进程相关
    ProcessBlocked,      // 进程阻塞
    ProcessCrash,        // 进程崩溃
}

impl SceneType {
    pub fn as_str(&self) -> &str {
        match self {
            SceneType::GpuOom => "gpu_oom",
            SceneType::GpuUtilLow => "gpu_util_low",
            SceneType::GpuError => "gpu_error",
            SceneType::NetworkStall => "network_stall",
            SceneType::NetworkDrop => "network_drop",
            SceneType::StorageIoError => "storage_io_error",
            SceneType::StorageSlow => "storage_slow",
            SceneType::ProcessBlocked => "process_blocked",
            SceneType::ProcessCrash => "process_crash",
        }
    }
}

/// 分析结果
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub scene: SceneType,
    pub root_causes: Vec<String>,
    pub confidence: f64,
    pub recommendations: Vec<String>,
}
