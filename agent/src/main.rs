mod plugin;
mod exec;
mod ipc;
mod diag;
mod scene;

use clap::{Parser, Subcommand};
use xctl_core::event::{Event, EventBus};
use xctl_core::graph::StateGraph;
use ipc::{IpcClient, IpcServer, default_socket_path};
use plugin::SubprocessProbe;
use exec::{SystemActuator, FixEngine};
use diag::run_diagnosis;
use scene::{SceneIdentifier, SceneType};
use std::sync::Arc;
use std::path::PathBuf;

#[cfg(windows)]
const DEFAULT_IPC_PORT: u16 = 9090;

#[derive(Parser)]
#[command(name = "xctl")]
#[command(about = "极简主义异构 AI 算力集群管控底座", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 启动后台 Daemon 模式（运行事件总线和探针）
    Run {
        #[cfg(unix)]
        /// Unix Domain Socket 路径（默认: /var/run/xctl.sock 或 ~/.xctl/xctl.sock）
        #[arg(long)]
        socket_path: Option<PathBuf>,
        #[cfg(windows)]
        /// IPC 服务端口（默认: 9090）
        #[arg(long, default_value_t = DEFAULT_IPC_PORT)]
        port: u16,
        /// 探针脚本路径（可选，默认使用内置 dummy_probe）
        #[arg(long)]
        probe: Option<PathBuf>,
    },
    /// 查询当前活跃进程列表
    Ps {
        #[cfg(unix)]
        /// Unix Domain Socket 路径（默认: /var/run/xctl.sock 或 ~/.xctl/xctl.sock）
        #[arg(long)]
        socket_path: Option<PathBuf>,
        #[cfg(windows)]
        /// IPC 服务端口（默认: 9090）
        #[arg(long, default_value_t = DEFAULT_IPC_PORT)]
        port: u16,
    },
    /// 分析进程阻塞根因
    Why {
        /// 目标进程 PID
        pid: u32,
        #[cfg(unix)]
        /// Unix Domain Socket 路径（默认: /var/run/xctl.sock 或 ~/.xctl/xctl.sock）
        #[arg(long)]
        socket_path: Option<PathBuf>,
        #[cfg(windows)]
        /// IPC 服务端口（默认: 9090）
        #[arg(long, default_value_t = DEFAULT_IPC_PORT)]
        port: u16,
    },
    /// 强制终止进程（包括进程树）
    Zap {
        /// 目标进程 PID
        pid: u32,
    },
    /// AI 诊断：使用大模型分析进程阻塞根因并提供修复建议
    Diag {
        /// 目标进程 PID
        pid: u32,
        #[cfg(unix)]
        /// Unix Domain Socket 路径（默认: /var/run/xctl.sock 或 ~/.xctl/xctl.sock）
        #[arg(long)]
        socket_path: Option<PathBuf>,
        #[cfg(windows)]
        /// IPC 服务端口（默认: 9090）
        #[arg(long, default_value_t = DEFAULT_IPC_PORT)]
        port: u16,
        /// 大模型提供商（openai/claude/local，默认从环境变量读取）
        #[arg(long)]
        provider: Option<String>,
        /// 规则文件目录（默认: ./rules）
        #[arg(long)]
        rules_dir: Option<PathBuf>,
    },
    /// 自动修复：根据诊断结果执行推荐动作（优雅降级、发信号、限流等）
    Fix {
        /// 目标进程 PID
        pid: u32,
        #[cfg(unix)]
        /// Unix Domain Socket 路径（默认: /var/run/xctl.sock 或 ~/.xctl/xctl.sock）
        #[arg(long)]
        socket_path: Option<PathBuf>,
        #[cfg(windows)]
        /// IPC 服务端口（默认: 9090）
        #[arg(long, default_value_t = DEFAULT_IPC_PORT)]
        port: u16,
        /// 规则文件目录（默认: ./rules）
        #[arg(long)]
        rules_dir: Option<PathBuf>,
        /// 是否自动执行（不询问确认）
        #[arg(long)]
        yes: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        #[cfg(unix)]
        Commands::Run { socket_path, probe } => {
            run_daemon(socket_path, probe).await?;
        }
        #[cfg(windows)]
        Commands::Run { port, probe } => {
            run_daemon(port, probe).await?;
        }
        #[cfg(unix)]
        Commands::Ps { socket_path } => {
            query_processes(socket_path).await?;
        }
        #[cfg(windows)]
        Commands::Ps { port } => {
            query_processes(port).await?;
        }
        #[cfg(unix)]
        Commands::Why { pid, socket_path } => {
            query_why(pid, socket_path).await?;
        }
        #[cfg(windows)]
        Commands::Why { pid, port } => {
            query_why(pid, port).await?;
        }
        Commands::Zap { pid } => {
            zap_process(pid).await?;
        }
        #[cfg(unix)]
        Commands::Diag { pid, socket_path, provider, rules_dir } => {
            diagnose_process(pid, socket_path, provider, rules_dir).await?;
        }
        #[cfg(windows)]
        Commands::Diag { pid, port, provider, rules_dir } => {
            diagnose_process(pid, port, provider, rules_dir).await?;
        }
        #[cfg(unix)]
        Commands::Fix { pid, socket_path, rules_dir, yes } => {
            fix_process(pid, socket_path, rules_dir, yes).await?;
        }
        #[cfg(windows)]
        Commands::Fix { pid, port, rules_dir, yes } => {
            fix_process(pid, port, rules_dir, yes).await?;
        }
    }

    Ok(())
}

/// Daemon 模式：启动事件总线、状态图、IPC 服务和探针
#[cfg(unix)]
async fn run_daemon(
    socket_path: Option<PathBuf>,
    probe_path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("[xctl] 启动事件总线...");
    
    // 创建事件总线
    let mut bus = EventBus::new(1000);
    let tx = bus.sender();

    // 创建状态图
    let graph = Arc::new(StateGraph::new());

    // 启动探针
    let probe_handle = {
        let tx = tx.clone();
        tokio::spawn(async move {
            if let Some(ref path) = probe_path {
                // 使用外部探针脚本
                // 尝试 python3，如果失败则尝试 python（Windows 兼容）
                let python_cmd = if cfg!(windows) {
                    "python"
                } else {
                    "python3"
                };
                
                let probe = SubprocessProbe::new(
                    python_cmd.to_string(),
                    vec![path.to_string_lossy().to_string()],
                );
                
                if let Err(e) = probe.start_stream(tx).await {
                    eprintln!("[xctl] 外部探针异常退出: {}", e);
                }
            } else {
                // 使用内置 dummy_probe（向后兼容）
                eprintln!("[xctl] 警告：使用内置 dummy_probe，建议使用 --probe 指定外部探针脚本");
                if let Err(e) = event::dummy_probe(tx).await {
                    eprintln!("[xctl] 内置探针异常退出: {}", e);
                }
            }
        })
    };

    // 启动事件消费和图形更新任务
    let graph_handle = {
        let graph = Arc::clone(&graph);
        let mut rx = bus.receiver();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Some(event) => {
                        if let Err(e) = graph.process_event(&event).await {
                            eprintln!("[xctl] 处理事件失败: {}", e);
                        }
                    }
                    None => {
                        eprintln!("[xctl] 事件通道已关闭");
                        break;
                    }
                }
            }
        })
    };

    // 启动 IPC 服务器（在后台任务中运行）
    let socket_path = socket_path.unwrap_or_else(default_socket_path);
    let socket_path_clone = socket_path.clone();
    
    let ipc_handle = {
        let graph = Arc::clone(&graph);
        tokio::spawn(async move {
            let server = IpcServer::new(graph, Some(socket_path_clone));
            if let Err(e) = server.serve().await {
                eprintln!("[xctl] IPC 服务器异常退出: {}", e);
            }
        })
    };

    println!("[xctl] 探针已启动，状态图已初始化");
    println!("[xctl] IPC 服务器已启动，监听 Unix Socket: {}", socket_path.display());
    println!("[xctl] 按 Ctrl+C 退出\n");

    // 等待退出信号
    tokio::signal::ctrl_c().await?;
    println!("\n[xctl] 收到退出信号，正在关闭...");
    
    probe_handle.abort();
    graph_handle.abort();
    ipc_handle.abort();

    // 清理 Socket 文件
    if socket_path.exists() {
        if let Err(e) = std::fs::remove_file(&socket_path) {
            eprintln!("[xctl] 警告：删除 Socket 文件失败: {}", e);
        }
    }

    println!("[xctl] 退出完成");
    Ok(())
}

#[cfg(windows)]
async fn run_daemon(
    port: u16,
    probe_path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("[xctl] 启动事件总线...");
    
    // 创建事件总线
    let mut bus = EventBus::new(1000);
    let tx = bus.sender();

    // 创建状态图
    let graph = Arc::new(StateGraph::new());

    // 启动探针
    let probe_handle = {
        let tx = tx.clone();
        tokio::spawn(async move {
            if let Some(ref path) = probe_path {
                let probe = SubprocessProbe::new(
                    "python".to_string(),
                    vec![path.to_string_lossy().to_string()],
                );
                
                if let Err(e) = probe.start_stream(tx).await {
                    eprintln!("[xctl] 外部探针异常退出: {}", e);
                }
            } else {
                eprintln!("[xctl] 警告：使用内置 dummy_probe，建议使用 --probe 指定外部探针脚本");
                if let Err(e) = event::dummy_probe(tx).await {
                    eprintln!("[xctl] 内置探针异常退出: {}", e);
                }
            }
        })
    };

    // 启动事件消费和图形更新任务
    let graph_handle = {
        let graph = Arc::clone(&graph);
        let mut rx = bus.receiver();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Some(event) => {
                        if let Err(e) = graph.process_event(&event).await {
                            eprintln!("[xctl] 处理事件失败: {}", e);
                        }
                    }
                    None => {
                        eprintln!("[xctl] 事件通道已关闭");
                        break;
                    }
                }
            }
        })
    };

    // 启动 IPC 服务器（在后台任务中运行）
    let ipc_handle = {
        let graph = Arc::clone(&graph);
        tokio::spawn(async move {
            let server = IpcServer::new(graph, port);
            if let Err(e) = server.serve().await {
                eprintln!("[xctl] IPC 服务器异常退出: {}", e);
            }
        })
    };

    println!("[xctl] 探针已启动，状态图已初始化");
    println!("[xctl] IPC 服务器已启动，监听端口 {}", port);
    println!("[xctl] 按 Ctrl+C 退出\n");

    // 等待退出信号
    tokio::signal::ctrl_c().await?;
    println!("\n[xctl] 收到退出信号，正在关闭...");
    
    probe_handle.abort();
    graph_handle.abort();
    ipc_handle.abort();

    println!("[xctl] 退出完成");
    Ok(())
}

/// 查询进程列表（通过 IPC）
#[cfg(unix)]
async fn query_processes(socket_path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let client = IpcClient::new(socket_path);
    
    // 检查 daemon 是否运行
    if !client.ping().await? {
        eprintln!("[xctl] 错误：无法连接到 daemon");
        eprintln!("[xctl] 请先运行: xctl run");
        return Err("daemon 未运行".into());
    }

    // 查询进程列表
    let processes = client.list_processes().await?;

    if processes.is_empty() {
        println!("没有活跃进程");
        return Ok(());
    }

    // 打印表头
    use colored::*;
    println!(
        "{:>8} | {:>12} | {:>20} | {}",
        "PID".bright_cyan(),
        "JOB_ID".bright_cyan(),
        "RESOURCES".bright_cyan(),
        "STATE".bright_cyan()
    );
    println!("{}", "-".repeat(80));

    // 打印每个进程
    for proc in processes {
        let pid = proc["pid"].as_u64().unwrap_or(0) as u32;
        let job_id = proc["job_id"]
            .as_str()
            .unwrap_or("-")
            .to_string();
        let state = proc["state"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        // 从 IPC 响应中获取资源列表
        let resources: Vec<String> = proc["resources"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let resources_str = if resources.is_empty() {
            "-".to_string()
        } else {
            resources.join(", ")
        };

        println!(
            "{:>8} | {:>12} | {:>20} | {}",
            pid.to_string().bright_green(),
            job_id.bright_yellow(),
            resources_str.bright_white(),
            state.bright_blue()
        );
    }

    Ok(())
}

#[cfg(windows)]
async fn query_processes(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let client = IpcClient::new(port);
    
    // 检查 daemon 是否运行
    if !client.ping().await? {
        eprintln!("[xctl] 错误：无法连接到 daemon (端口 {})", port);
        eprintln!("[xctl] 请先运行: xctl run");
        return Err("daemon 未运行".into());
    }

    // 查询进程列表
    let processes = client.list_processes().await?;

    if processes.is_empty() {
        println!("没有活跃进程");
        return Ok(());
    }

    // 打印表头
    use colored::*;
    println!(
        "{:>8} | {:>12} | {:>20} | {}",
        "PID".bright_cyan(),
        "JOB_ID".bright_cyan(),
        "RESOURCES".bright_cyan(),
        "STATE".bright_cyan()
    );
    println!("{}", "-".repeat(80));

    // 打印每个进程
    for proc in processes {
        let pid = proc["pid"].as_u64().unwrap_or(0) as u32;
        let job_id = proc["job_id"]
            .as_str()
            .unwrap_or("-")
            .to_string();
        let state = proc["state"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        // 从 IPC 响应中获取资源列表
        let resources: Vec<String> = proc["resources"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let resources_str = if resources.is_empty() {
            "-".to_string()
        } else {
            resources.join(", ")
        };

        println!(
            "{:>8} | {:>12} | {:>20} | {}",
            pid.to_string().bright_green(),
            job_id.bright_yellow(),
            resources_str.bright_white(),
            state.bright_blue()
        );
    }

    Ok(())
}

/// 查询进程阻塞根因（通过 IPC）
#[cfg(unix)]
async fn query_why(pid: u32, socket_path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    use colored::*;
    use crate::ipc::IpcClient;
    
    let client = IpcClient::new(socket_path);
    
    // 检查 daemon 是否运行
    if !client.ping().await? {
        eprintln!("[xctl] 错误：无法连接到 daemon");
        eprintln!("[xctl] 请先运行: xctl run");
        return Err("daemon 未运行".into());
    }

    // 查询根因
    let causes = client.why_process(pid).await?;

    // 尝试场景识别和分析（需要访问图状态，当前通过 IPC 无法直接访问）
    // 这里先使用基本的根因分析，场景分析功能可以在未来扩展 IPC 接口后启用
    
    if causes.is_empty() {
        println!(
            "进程 {} 未发现阻塞问题",
            pid.to_string().bright_green()
        );
        return Ok(());
    }

    println!(
        "进程 {} 的阻塞根因分析:",
        pid.to_string().bright_green()
    );
    println!("{}", "-".repeat(60));

    // 尝试识别场景类型（基于根因文本）
    let scene_hint = if causes.iter().any(|c| c.contains("GPU") || c.contains("OOM") || c.contains("显存")) {
        Some("GPU OOM")
    } else if causes.iter().any(|c| c.contains("网络") || c.contains("network") || c.contains("等待资源")) {
        Some("网络阻塞")
    } else if causes.iter().any(|c| c.contains("exit") || c.contains("crash") || c.contains("failed")) {
        Some("进程崩溃")
    } else {
        None
    };

    if let Some(scene) = scene_hint {
        println!("  [场景识别] {}", scene.bright_cyan());
        println!();
    }

    for (idx, cause) in causes.iter().enumerate() {
        if cause.starts_with("等待资源") {
            println!("  {}. {}", idx + 1, cause.bright_yellow());
        } else if cause.contains("error") {
            println!("  {}. {}", idx + 1, cause.bright_red());
        } else {
            println!("  {}. {}", idx + 1, cause);
        }
    }

    Ok(())
}

/// 强制终止进程
async fn zap_process(pid: u32) -> Result<(), Box<dyn std::error::Error>> {
    println!("[xctl] 正在终止进程 {}...", pid);
    
    let actuator = SystemActuator::new();
    match actuator.execute(pid, "zap").await {
        Ok(_) => {
            println!("[xctl] 进程 {} 已成功终止", pid);
        }
        Err(e) => {
            eprintln!("[xctl] 终止进程失败: {}", e);
            return Err(e.into());
        }
    }
    
    Ok(())
}

#[cfg(windows)]
async fn query_why(pid: u32, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    use colored::*;
    use crate::ipc::IpcClient;
    
    let client = IpcClient::new(port);
    
    // 检查 daemon 是否运行
    if !client.ping().await? {
        eprintln!("[xctl] 错误：无法连接到 daemon (端口 {})", port);
        eprintln!("[xctl] 请先运行: xctl run");
        return Err("daemon 未运行".into());
    }

    // 查询根因
    let causes = client.why_process(pid).await?;

    if causes.is_empty() {
        println!(
            "进程 {} 未发现阻塞问题",
            pid.to_string().bright_green()
        );
        return Ok(());
    }

    println!(
        "进程 {} 的阻塞根因分析:",
        pid.to_string().bright_green()
    );
    println!("{}", "-".repeat(60));

    // 尝试识别场景类型（基于根因文本）
    let scene_hint = if causes.iter().any(|c| c.contains("GPU") || c.contains("OOM") || c.contains("显存")) {
        Some("GPU OOM")
    } else if causes.iter().any(|c| c.contains("网络") || c.contains("network") || c.contains("等待资源")) {
        Some("网络阻塞")
    } else if causes.iter().any(|c| c.contains("exit") || c.contains("crash") || c.contains("failed")) {
        Some("进程崩溃")
    } else {
        None
    };

    if let Some(scene) = scene_hint {
        println!("  [场景识别] {}", scene.bright_cyan());
        println!();
    }

    for (idx, cause) in causes.iter().enumerate() {
        if cause.starts_with("等待资源") {
            println!("  {}. {}", idx + 1, cause.bright_yellow());
        } else if cause.contains("error") {
            println!("  {}. {}", idx + 1, cause.bright_red());
        } else {
            println!("  {}. {}", idx + 1, cause);
        }
    }

    Ok(())
}

/// AI 诊断：使用大模型分析进程问题
#[cfg(unix)]
async fn diagnose_process(
    pid: u32,
    socket_path: Option<PathBuf>,
    provider: Option<String>,
    rules_dir: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    use colored::*;

    println!(
        "[xctl] 正在诊断进程 {}...",
        pid.to_string().bright_green()
    );
    println!("[xctl] 收集诊断信息...\n");

    // 如果没有指定规则目录，尝试使用默认的 ./rules
    let rules_path = rules_dir.or_else(|| {
        let default = PathBuf::from("rules");
        if default.exists() {
            Some(default)
        } else {
            None
        }
    });

    // 执行诊断
    let diagnosis = match run_diagnosis(pid, socket_path, provider, rules_path).await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[xctl] 诊断失败: {}", e);
            eprintln!("\n提示:");
            eprintln!("  1. 确保 daemon 正在运行: xctl run");
            eprintln!("  2. 设置 API Key:");
            eprintln!("     export OPENAI_API_KEY=your_key");
            eprintln!("     或");
            eprintln!("     export ANTHROPIC_API_KEY=your_key");
            eprintln!("  3. 检查网络连接");
            return Err(e);
        }
    };

    // 显示诊断结果
    println!("{}", "=".repeat(70).bright_cyan());
    println!("{}", "AI 诊断报告".bright_cyan().bold());
    println!("{}", "=".repeat(70).bright_cyan());
    println!();

    // 阻塞根因
    if !diagnosis.causes.is_empty() {
        println!("{}", "阻塞根因:".bright_yellow().bold());
        for (idx, cause) in diagnosis.causes.iter().enumerate() {
            if cause.starts_with("等待资源") {
                println!("  {}. {}", idx + 1, cause.bright_yellow());
            } else if cause.contains("error") {
                println!("  {}. {}", idx + 1, cause.bright_red());
            } else {
                println!("  {}. {}", idx + 1, cause);
            }
        }
        println!();
    }

    // AI 建议
    println!("{}", "AI 诊断建议:".bright_green().bold());
    println!("{}", "-".repeat(70));
    
    // 格式化输出建议（按段落分割）
    let lines: Vec<&str> = diagnosis.recommendation.lines().collect();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            println!();
        } else if trimmed.starts_with('#') || trimmed.starts_with("##") {
            // 标题
            println!("{}", trimmed.bright_cyan());
        } else if trimmed.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            // 编号列表
            println!("  {}", trimmed);
        } else {
            // 普通文本
            println!("  {}", trimmed);
        }
    }

    println!();
    println!(
        "{}",
        format!("置信度: {:.0}%", diagnosis.confidence * 100.0).bright_white()
    );
    println!();

    Ok(())
}

/// 自动修复进程：根据诊断结果执行推荐动作
#[cfg(unix)]
async fn fix_process(
    pid: u32,
    socket_path: Option<PathBuf>,
    rules_dir: Option<PathBuf>,
    auto_yes: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use colored::Colorize;
    
    println!(
        "[xctl] 正在修复进程 {}...",
        pid.to_string().bright_green()
    );
    
    // 连接到 daemon
    let client = IpcClient::new(socket_path);
    if !client.ping().await? {
        return Err("无法连接到 daemon，请先运行: xctl run".into());
    }
    
    // 获取根因分析（用于场景识别）
    let causes = client.why_process(pid).await?;
    
    // 识别场景（简化版：基于根因文本）
    let scene = identify_scene_from_causes(&causes);
    
    if scene.is_none() {
        println!("{}", "[xctl] 未识别到问题场景，无法自动修复".bright_yellow());
        println!("提示: 可以尝试手动执行: xctl zap {}", pid);
        return Ok(());
    }
    
    let scene = scene.unwrap();
    println!("[xctl] 识别到场景: {:?}", scene);
    
    // 创建分析结果（基于根因）
    let analysis = create_analysis_from_causes(scene, &causes);
    
    if analysis.is_none() {
        println!("{}", "[xctl] 无法分析场景，无法自动修复".bright_yellow());
        return Ok(());
    }
    
    let analysis = analysis.unwrap();
    
    // 显示推荐动作
    if !analysis.recommended_actions.is_empty() {
        println!("\n{}", "推荐动作:".bright_cyan().bold());
        for (idx, action) in analysis.recommended_actions.iter().enumerate() {
            println!("  {}. {}", idx + 1, action);
        }
        println!();
    }
    
    // 确认执行
    if !auto_yes {
        use std::io::{self, Write};
        print!("{}", "是否执行修复? [y/N]: ".bright_yellow());
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") && !input.trim().eq_ignore_ascii_case("yes") {
            println!("{}", "已取消".bright_yellow());
            return Ok(());
        }
    }
    
    // 执行修复
    let fix_engine = FixEngine::new();
    let result = fix_engine.fix_from_analysis(&analysis, pid).await?;
    
    // 显示结果
    println!("\n{}", "=".repeat(70).bright_cyan());
    println!("{}", "修复结果".bright_cyan().bold());
    println!("{}", "=".repeat(70).bright_cyan());
    println!();
    
    if result.success {
        println!("{}", format!("✅ {}", result.message).bright_green());
    } else {
        println!("{}", format!("⚠️  {}", result.message).bright_yellow());
    }
    
    if !result.executed_actions.is_empty() {
        println!("\n{}", "已执行的动作:".bright_green().bold());
        for action in &result.executed_actions {
            println!("  ✅ {}: {}", action.action, action.result);
        }
    }
    
    if !result.failed_actions.is_empty() {
        println!("\n{}", "失败的动作:".bright_red().bold());
        for action in &result.failed_actions {
            println!("  ❌ {}: {}", action.action, action.error);
        }
    }
    
    Ok(())
}

#[cfg(windows)]
async fn fix_process(
    pid: u32,
    port: u16,
    rules_dir: Option<PathBuf>,
    auto_yes: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use colored::Colorize;
    
    println!(
        "[xctl] 正在修复进程 {}...",
        pid.to_string().bright_green()
    );
    
    // 连接到 daemon
    let client = IpcClient::new(port);
    if !client.ping().await? {
        return Err("无法连接到 daemon，请先运行: xctl run".into());
    }
    
    // 获取根因分析
    let causes = client.why_process(pid).await?;
    
    // 识别场景
    let scene = identify_scene_from_causes(&causes);
    
    if scene.is_none() {
        println!("{}", "[xctl] 未识别到问题场景，无法自动修复".bright_yellow());
        return Ok(());
    }
    
    let scene = scene.unwrap();
    
    // 创建分析结果
    let analysis = create_analysis_from_causes(scene, &causes);
    
    // 执行修复
    let fix_engine = FixEngine::new();
    let result = fix_engine.fix_from_analysis(&analysis, pid).await?;
    
    println!("修复结果: {}", result.message);
    
    Ok(())
}

/// 从根因识别场景（简化版）
fn identify_scene_from_causes(causes: &[String]) -> Option<SceneType> {
    for cause in causes {
        let cause_lower = cause.to_lowercase();
        if cause_lower.contains("gpu") && cause_lower.contains("oom") {
            return Some(SceneType::GpuOom);
        }
        if cause_lower.contains("network") || cause_lower.contains("网络") {
            return Some(SceneType::NetworkStall);
        }
        if cause_lower.contains("storage") || cause_lower.contains("存储") {
            return Some(SceneType::StorageIoError);
        }
        if cause_lower.contains("crash") || cause_lower.contains("崩溃") {
            return Some(SceneType::ProcessCrash);
        }
    }
    Some(SceneType::WorkloadStalled) // 默认场景
}

/// 从根因创建分析结果（简化版）
fn create_analysis_from_causes(scene: SceneType, causes: &[String]) -> scene::AnalysisResult {
    let mut recommended_actions = Vec::new();
    
    // 根据场景类型添加推荐动作
    match scene {
        SceneType::GpuOom => {
            recommended_actions.push("尝试触发框架层的 Checkpoint Dump 信号 (SIGUSR1)".to_string());
            recommended_actions.push("隔离该节点，执行 xctl zap 清理僵尸进程".to_string());
        }
        SceneType::NetworkStall => {
            recommended_actions.push("检查交换机 PFC 配置".to_string());
            recommended_actions.push("检查 RoCE/HCCS 连接状态".to_string());
        }
        SceneType::WorkloadStalled => {
            recommended_actions.push("如果确认卡死，执行 xctl zap 终止进程".to_string());
            recommended_actions.push("检查是否有 Checkpoint 可以恢复".to_string());
        }
        _ => {
            recommended_actions.push("执行 xctl zap 终止进程".to_string());
        }
    }
    
    scene::AnalysisResult {
        scene,
        root_causes: causes.to_vec(),
        confidence: 0.7,
        recommendations: vec!["根据根因分析执行修复".to_string()],
        recommended_actions,
        severity: scene::Severity::Warning,
    }
}
