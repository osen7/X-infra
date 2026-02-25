//! 原生探针模块
//! 
//! 使用 FFI 直接绑定 NVML 和华为 CANN 库，淘汰 Python 包装层

pub mod nvml;
pub mod cann;

use async_trait::async_trait;
use xctl_core::event::Event;
use tokio::sync::mpsc;
use crate::plugin::EventSource;

/// 原生探针（统一接口）
pub struct NativeProbe {
    probe_type: ProbeType,
}

#[derive(Debug, Clone)]
pub enum ProbeType {
    Nvml,
    Cann,
}

impl NativeProbe {
    pub fn new(probe_type: ProbeType) -> Self {
        Self { probe_type }
    }
}

#[async_trait]
impl EventSource for NativeProbe {
    fn name(&self) -> &str {
        match self.probe_type {
            ProbeType::Nvml => "nvml-native",
            ProbeType::Cann => "cann-native",
        }
    }

    async fn start_stream(&self, tx: mpsc::Sender<Event>) -> Result<(), String> {
        match self.probe_type {
            ProbeType::Nvml => {
                nvml::start_nvml_probe(tx).await
            }
            ProbeType::Cann => {
                cann::start_cann_probe(tx).await
            }
        }
    }
}
