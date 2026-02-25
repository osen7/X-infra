#!/bin/bash
# 生成内核绑定（CO-RE 支持）
# 使用 aya-tool 从 /sys/kernel/btf/vmlinux 生成 Rust 绑定

set -e

echo "🔧 生成内核绑定（CO-RE 支持）..."

# 检查 aya-tool 是否安装
if ! command -v aya-tool &> /dev/null; then
    echo "❌ aya-tool 未安装，正在安装..."
    cargo install aya-tool
fi

# 检查 BTF 文件是否存在
if [ ! -f /sys/kernel/btf/vmlinux ]; then
    echo "❌ 错误：/sys/kernel/btf/vmlinux 不存在"
    echo "   请确保内核支持 BTF（CONFIG_DEBUG_INFO_BTF=y）"
    exit 1
fi

# 创建 bindings 目录
mkdir -p xctl-probe-ebpf-ebpf/src/bindings

# 生成绑定（只生成我们需要的结构体）
echo "📦 生成内核结构体绑定..."
aya-tool generate \
    --btf /sys/kernel/btf/vmlinux \
    --output xctl-probe-ebpf-ebpf/src/bindings/mod.rs \
    --struct sock \
    --struct sock_common

echo "✅ 内核绑定生成完成！"
echo "   文件位置: xctl-probe-ebpf-ebpf/src/bindings/mod.rs"
