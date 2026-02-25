use std::process::Command;

fn main() {
    // 构建 eBPF 内核程序
    println!("🔨 构建 eBPF 内核程序...");
    let status = Command::new("cargo")
        .args(&[
            "+nightly",
            "build",
            "--release",
            "--target",
            "bpfel-unknown-none",
            "--manifest-path",
            "../xctl-probe-ebpf-ebpf/Cargo.toml",
        ])
        .status()
        .expect("Failed to build eBPF program");

    if !status.success() {
        eprintln!("❌ eBPF 程序构建失败");
        std::process::exit(1);
    }

    // 构建用户态程序
    println!("🔨 构建用户态程序...");
    let status = Command::new("cargo")
        .args(&["build", "--release", "--manifest-path", "../xctl-probe-ebpf/Cargo.toml"])
        .status()
        .expect("Failed to build user-space program");

    if !status.success() {
        eprintln!("❌ 用户态程序构建失败");
        std::process::exit(1);
    }

    println!("✅ 构建完成！");
    println!("📦 可执行文件位置: xctl-probe-ebpf/target/release/xctl-probe-ebpf");
}
