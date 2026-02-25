#!/bin/bash
# 生成内核绑定（CO-RE 支持）
# 使用 aya-tool 从 /sys/kernel/btf/vmlinux 生成 Rust 绑定
#
# 前置要求：
# 1. 内核必须支持 BTF（CONFIG_DEBUG_INFO_BTF=y）
# 2. 安装 aya-tool: cargo install aya-tool
# 3. 在 Linux 环境中运行（需要访问 /sys/kernel/btf/vmlinux）

set -e

echo "🔧 生成内核绑定（CO-RE 支持）..."
echo ""

# 检查是否在 Linux 环境
if [[ "$OSTYPE" != "linux-gnu"* ]]; then
    echo "⚠️  警告：此脚本需要在 Linux 环境中运行"
    echo "   当前系统: $OSTYPE"
    echo "   在 Windows/macOS 上，请使用 WSL 或 Linux 虚拟机"
    echo ""
fi

# 检查 aya-tool 是否安装
if ! command -v aya-tool &> /dev/null; then
    echo "📦 aya-tool 未安装，正在安装..."
    cargo install aya-tool
    echo ""
fi

# 检查 BTF 文件是否存在
if [ ! -f /sys/kernel/btf/vmlinux ]; then
    echo "❌ 错误：/sys/kernel/btf/vmlinux 不存在"
    echo ""
    echo "   可能的原因："
    echo "   1. 内核未启用 BTF 支持（需要 CONFIG_DEBUG_INFO_BTF=y）"
    echo "   2. 内核版本过旧（需要 Linux 5.2+）"
    echo ""
    echo "   检查方法："
    echo "   $ grep CONFIG_DEBUG_INFO_BTF /boot/config-$(uname -r)"
    echo ""
    echo "   如果未启用，需要重新编译内核或使用支持 BTF 的发行版（如 Ubuntu 20.04+）"
    exit 1
fi

# 创建 bindings 目录
mkdir -p ark-probe-ebpf-ebpf/src/bindings

# 生成绑定（只生成我们需要的结构体）
echo "📦 生成内核结构体绑定..."
echo "   从 /sys/kernel/btf/vmlinux 提取 sock 和 sock_common 结构体..."
echo ""

# 使用 aya-tool 生成绑定
# 注意：aya-tool 的 API 可能因版本而异，这里使用通用方法
if aya-tool generate --help | grep -q "btf"; then
    # 新版本 aya-tool
    aya-tool generate \
        --btf /sys/kernel/btf/vmlinux \
        --output ark-probe-ebpf-ebpf/src/bindings/mod.rs \
        --struct sock \
        --struct sock_common
else
    # 旧版本或使用 bindgen
    echo "⚠️  使用备用方法生成绑定..."
    # 这里可以添加备用生成逻辑
    # 或者提示用户手动生成
    echo "   请参考 Aya 文档手动生成绑定"
    exit 1
fi

echo ""
echo "✅ 内核绑定生成完成！"
echo "   文件位置: ark-probe-ebpf-ebpf/src/bindings/mod.rs"
echo ""
echo "📝 下一步："
echo "   1. 检查生成的绑定文件是否正确"
echo "   2. 运行 cargo build -p ark-probe-ebpf-ebpf 编译 eBPF 程序"
echo "   3. 运行 cargo build -p ark-probe-ebpf 编译用户态程序"