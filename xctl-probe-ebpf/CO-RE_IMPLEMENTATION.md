# eBPF CO-RE 四元组提取实现指南

本文档说明如何在 Linux 环境中完成 eBPF CO-RE（Compile Once, Run Everywhere）的四元组提取实现。

## 🎯 目标

实现像素级的"拥塞到进程"溯源，彻底解决软中断（SoftIRQ）上下文的 PID 错乱陷阱。

## 📋 前置要求

1. **Linux 内核支持 BTF**
   - 内核版本 >= 5.2
   - 编译时启用 `CONFIG_DEBUG_INFO_BTF=y`
   - 检查方法：`grep CONFIG_DEBUG_INFO_BTF /boot/config-$(uname -r)`
   - 或检查文件：`ls -l /sys/kernel/btf/vmlinux`

2. **安装 aya-tool**
   ```bash
   cargo install aya-tool
   ```

3. **Rust 工具链**
   - Rust nightly（用于编译 eBPF 程序）
   - 安装：`rustup install nightly`
   - 组件：`rustup component add rust-src --toolchain nightly`

## 🚀 实现步骤

### 步骤 1：生成内核绑定

在 Linux 环境中运行：

```bash
cd xctl-probe-ebpf
chmod +x generate-bindings.sh
./generate-bindings.sh
```

这将从 `/sys/kernel/btf/vmlinux` 生成 `xctl-probe-ebpf-ebpf/src/bindings/mod.rs`，包含：
- `struct sock`
- `struct sock_common`

### 步骤 2：验证绑定生成

检查生成的文件：

```bash
cat xctl-probe-ebpf-ebpf/src/bindings/mod.rs
```

应该看到类似这样的结构体定义：

```rust
#[repr(C)]
pub struct sock_common {
    pub skc_family: u16,
    pub skc_num: u16,              // 源端口
    pub skc_dport: u16,            // 目的端口
    pub skc_daddr: u32,            // 目的 IP
    pub skc_rcv_saddr: u32,        // 源 IP
    // ... 其他字段
}

#[repr(C)]
pub struct sock {
    pub __sk_common: sock_common,
    // ... 其他字段
}
```

### 步骤 3：编译 eBPF 程序

```bash
# 编译内核态 eBPF 程序
cargo build --release -p xctl-probe-ebpf-ebpf

# 编译用户态程序
cargo build --release -p xctl-probe-ebpf
```

### 步骤 4：测试运行

```bash
# 需要 root 权限
sudo ./target/release/xctl-probe-ebpf
```

## 🔍 实现细节

### 核心函数：`extract_socket_tuple_from_sendmsg`

在 `tcp_sendmsg` Hook 中（真实进程上下文）：

1. **获取 socket 指针**：`ctx.arg(0)` 获取 `struct sock *sk`
2. **读取 sock_common**：使用 `bpf_probe_read_kernel` 安全读取
3. **检查协议族**：只处理 IPv4（`skc_family == 2`）
4. **提取四元组**：
   - `skc_rcv_saddr` → 源 IP（网络字节序）
   - `skc_daddr` → 目的 IP（网络字节序）
   - `skc_num` → 源端口（网络字节序）
   - `skc_dport` → 目的端口（网络字节序）
5. **存入 Map**：`SOCKET_TO_PID.insert(&tuple, &pid)`

### 核心函数：`extract_socket_tuple_from_retransmit`

在 `tcp_retransmit_skb` Hook 中（软中断上下文）：

1. **获取 socket 指针**：`ctx.arg(0)` 获取 `struct sock *sk`
2. **提取四元组**：使用相同的逻辑提取
3. **查询 Map**：`SOCKET_TO_PID.get(&tuple)` 反查真实 PID
4. **输出事件**：使用反查到的真实 PID 创建 `NetworkEvent`

## ⚠️ 注意事项

### 网络字节序

所有 IP 地址和端口都是**网络字节序（大端序）**：
- 在用户态解析时需要转换：`u16::from_be(port)`
- IP 地址：`u32::from_be(ip)` 或使用 `std::net::Ipv4Addr`

### IPv4 限制

当前实现只处理 IPv4 连接：
- 检查 `skc_family == 2`（`AF_INET`）
- IPv6 连接会被跳过（返回空值）

### 错误处理

- 如果无法获取 `sk` 指针，返回空值
- 如果读取 `sock_common` 失败，返回空值
- 如果 Map 查询失败，使用 fallback PID

## 🐛 故障排查

### 问题 1：BTF 文件不存在

```
错误：/sys/kernel/btf/vmlinux 不存在
```

**解决方案**：
- 使用支持 BTF 的发行版（Ubuntu 20.04+, Fedora 33+）
- 或重新编译内核，启用 `CONFIG_DEBUG_INFO_BTF=y`

### 问题 2：绑定生成失败

```
错误：aya-tool generate 失败
```

**解决方案**：
- 更新 aya-tool：`cargo install --force aya-tool`
- 检查 aya-tool 版本：`aya-tool --version`
- 参考 [Aya 文档](https://aya-rs.dev/book/) 手动生成

### 问题 3：编译错误

```
错误：无法找到 bindings::sock
```

**解决方案**：
- 确保已运行 `generate-bindings.sh`
- 检查 `xctl-probe-ebpf-ebpf/src/bindings/mod.rs` 是否存在
- 确保 `mod bindings;` 在 `main.rs` 中正确声明

### 问题 4：运行时 PID 不准确

如果仍然出现 PID 不准确：

1. **检查 Map 是否正常工作**：
   - 查看日志：`dmesg | grep xctl`
   - 确认 `tcp_sendmsg` Hook 是否成功建立映射

2. **验证四元组提取**：
   - 检查日志中的 socket 信息是否正确
   - 确认 IP 和端口格式正确

3. **检查 Map 大小**：
   - 默认 `SOCKET_TO_PID` 最大 8192 条目
   - 如果连接数过多，可能需要增加大小

## 📚 参考资料

- [Aya eBPF 框架文档](https://aya-rs.dev/book/)
- [Linux 内核 BTF 文档](https://www.kernel.org/doc/html/latest/bpf/btf.html)
- [eBPF CO-RE 最佳实践](https://nakryiko.com/posts/bpf-core-reference-guide/)

## ✅ 验证清单

完成实现后，验证以下功能：

- [ ] `generate-bindings.sh` 成功生成绑定文件
- [ ] eBPF 程序编译成功
- [ ] 用户态程序编译成功
- [ ] 运行后能捕获 `tcp_sendmsg` 事件
- [ ] 运行后能捕获 `tcp_retransmit_skb` 事件
- [ ] PID 映射正确（Map 查询成功）
- [ ] 网络事件中的 PID 准确
- [ ] 四元组信息正确（IP 和端口格式正确）

## 🎉 完成标志

当以下条件满足时，说明实现成功：

1. **编译通过**：`cargo build --release` 无错误
2. **运行正常**：程序能正常启动并捕获事件
3. **PID 准确**：`tcp_retransmit_skb` 中的 PID 与 `tcp_sendmsg` 中的 PID 一致
4. **四元组完整**：日志中显示正确的 IP:Port 信息

此时，xctl 已具备**像素级的"拥塞到进程"溯源能力**，这是整个项目最硬核的技术护城河！
