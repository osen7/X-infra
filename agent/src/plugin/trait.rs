use xctl_core::event::Event;
use async_trait::async_trait;
use tokio::sync::mpsc;

/// 事件源 Trait：所有探针必须实现此接口
#[async_trait]
pub trait EventSource: Send + Sync {
    /// 探针名称
    fn name(&self) -> &str;

    /// 启动探针，将产生的事件推入发送端
    /// 探针应该持续运行，直到通道关闭或发生错误
    async fn start_stream(&self, tx: mpsc::Sender<Event>) -> Result<(), String>;
}

/// 执行器 Trait：所有系统干预动作必须实现此接口
#[async_trait]
pub trait Actuator: Send + Sync {
    /// 执行器名称
    fn name(&self) -> &str;

    /// 执行动作
    /// - target_pid: 目标进程 PID
    /// - action: 动作类型（如 "kill", "reset"）
    async fn execute(&self, target_pid: u32, action: &str) -> Result<(), String>;
}
