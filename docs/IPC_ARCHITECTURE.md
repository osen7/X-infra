# IPC 架构设计与演进

## 当前实现：TCP Socket (127.0.0.1:9090)

### 设计选择

xctl 当前使用 TCP Socket 作为 IPC 通信机制，原因：

1. **跨平台兼容**：Windows 和 Linux 都支持 TCP
2. **开发友好**：易于调试和测试
3. **网络隔离**：127.0.0.1 只监听本地，安全性足够

### 实现细节

- **协议**：自定义 JSON RPC over TCP
- **格式**：长度前缀（4字节）+ JSON 体
- **安全**：请求/响应大小限制（10MB/100MB）
- **端口**：默认 9090，可通过 `--port` 配置

## 未来演进：Unix Domain Socket (UDS)

### 为什么需要 UDS？

1. **端口冲突**：在生产环境（K8s DaemonSet）中，9090 可能与其他服务冲突
2. **权限控制**：UDS 可以通过文件系统权限控制访问
3. **性能优势**：UDS 比 TCP 更快（无需网络栈）
4. **安全性**：更细粒度的访问控制

### 迁移方案

#### Phase 1: 双协议支持（推荐）

同时支持 TCP 和 UDS，通过配置选择：

```rust
pub enum IpcTransport {
    Tcp { port: u16 },
    Unix { path: PathBuf },
}

impl IpcServer {
    pub fn new(graph: Arc<StateGraph>, transport: IpcTransport) -> Self {
        // 根据 transport 类型选择监听方式
    }
}
```

#### Phase 2: 默认 UDS，TCP 作为备选

- Linux: 默认使用 `/var/run/xctl.sock`
- Windows: 回退到 TCP
- 通过环境变量或配置切换

### 实现示例

```rust
// src/ipc.rs

#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

pub enum IpcTransport {
    Tcp { port: u16 },
    #[cfg(unix)]
    Unix { path: PathBuf },
}

impl IpcServer {
    #[cfg(unix)]
    pub async fn serve_unix(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // 删除已存在的 socket 文件
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        
        let listener = UnixListener::bind(path)?;
        println!("[xctl] IPC 服务器已启动，监听 Unix Socket: {:?}", path);
        
        // 设置权限（仅 owner 可读写）
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let graph = Arc::clone(&self.graph);
                    tokio::spawn(async move {
                        if let Err(e) = handle_unix_client(stream, graph).await {
                            eprintln!("[xctl] 处理客户端请求失败: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("[xctl] 接受连接失败: {}", e);
                }
            }
        }
    }
}
```

### 迁移时间表

- **v0.1.x（当前）**：TCP Socket，跨平台兼容
- **v0.2.x（计划）**：双协议支持，UDS 可选
- **v1.0.0（生产）**：Linux 默认 UDS，Windows 保持 TCP

## 安全考虑

### TCP Socket 安全

- ✅ 监听 127.0.0.1，不暴露到外部网络
- ✅ 请求大小限制（10MB）
- ✅ 响应大小限制（100MB）

### UDS 安全（未来）

- ✅ 文件系统权限控制（0o600）
- ✅ 通过用户组控制访问
- ✅ 避免端口冲突

## 性能对比

| 特性 | TCP Socket | Unix Domain Socket |
|------|-----------|-------------------|
| 延迟 | ~1-2ms | ~0.1ms |
| 吞吐 | ~100MB/s | ~500MB/s |
| 跨平台 | ✅ | ❌ (仅 Unix) |
| 权限控制 | 端口级别 | 文件系统级别 |
| 端口冲突 | 可能 | 不可能 |

## 建议

1. **当前阶段**：保持 TCP Socket，确保跨平台兼容
2. **生产准备**：实现 UDS 支持，作为 Linux 的默认选项
3. **向后兼容**：通过配置或环境变量切换，不破坏现有使用
