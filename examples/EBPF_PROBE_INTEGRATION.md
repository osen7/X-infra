# eBPF 探针集成指南

## 🎯 集成到 xctl

eBPF 网络探针已经可以通过 `SubprocessProbe` 无缝集成到 xctl。

### 方式 1：手动指定探针路径

```bash
# 构建 eBPF 探针
cd xctl-probe-ebpf
./build.sh

# 启动 xctl daemon，使用 eBPF 探针
xctl run --probe ./xctl-probe-ebpf/target/release/xctl-probe-ebpf
```

### 方式 2：自动检测（未来实现）

xctl 可以自动检测并启动 eBPF 探针：

```bash
# xctl 会自动查找 eBPF 探针
xctl run --probe auto
```

## 📊 数据流

```
Linux 内核 TCP 重传
    ↓
tcp_retransmit_skb (被 eBPF Hook)
    ↓
eBPF 程序捕获事件
    ↓
PerfEventArray (RingBuffer)
    ↓
xctl-probe-ebpf 用户态程序
    ↓
JSONL 输出到 stdout
    ↓
SubprocessProbe 读取
    ↓
xctl 事件总线
    ↓
状态图建立 WaitsOn 边
    ↓
workload-stalled.yaml 规则触发
    ↓
SRE 收到告警
```

## 🔧 配置示例

### systemd 服务（生产环境）

```ini
[Unit]
Description=xctl eBPF Network Probe
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/xctl-probe-ebpf
Restart=on-failure
RestartSec=5
User=root
CapabilityBoundingSet=CAP_BPF CAP_SYS_ADMIN

[Install]
WantedBy=multi-user.target
```

### xctl 集成

```bash
# 在 xctl run 中自动启动
xctl run --probe ./xctl-probe-ebpf/target/release/xctl-probe-ebpf
```

## 🎯 验证

运行后，检查事件是否正确输出：

```bash
# 查看 xctl 日志
xctl run --probe ./xctl-probe-ebpf/target/release/xctl-probe-ebpf 2>&1 | grep transport.drop
```

应该看到类似输出：
```json
{"ts":1710000000000,"event_type":"transport.drop","entity_id":"network-pid-1024","pid":1024,"value":"1"}
```

## 🔒 权限要求

eBPF 探针需要 root 权限：

```bash
# 方式 1：使用 sudo
sudo xctl run --probe ./xctl-probe-ebpf/target/release/xctl-probe-ebpf

# 方式 2：设置 capability（推荐）
sudo setcap cap_bpf,cap_sys_admin+ep ./xctl-probe-ebpf/target/release/xctl-probe-ebpf
sudo setcap cap_bpf,cap_sys_admin+ep ./target/release/xctl
xctl run --probe ./xctl-probe-ebpf/target/release/xctl-probe-ebpf
```
