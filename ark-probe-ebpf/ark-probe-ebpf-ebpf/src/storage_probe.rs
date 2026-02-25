//! eBPF 存储探针（NVMe/VFS）
//! 
//! 监控底层文件系统读取延迟，抓出导致 Dataloader 卡顿的慢 I/O

use aya_bpf::{
    macros::{kprobe, map},
    maps::PerfEventArray,
    programs::ProbeContext,
};

use ark_probe_ebpf_ebpf::StorageEvent;

#[map]
static mut STORAGE_EVENTS: PerfEventArray<StorageEvent> = PerfEventArray::with_max_entries(1024, 0);

/// Hook vfs_read - 文件系统读取
#[kprobe(name = "vfs_read")]
pub fn vfs_read(ctx: ProbeContext) -> u32 {
    // TODO: 使用 CO-RE 提取文件路径和 I/O 大小
    // 1. 从 kiocb 结构体提取文件描述符
    // 2. 从 file 结构体提取路径（如果可获取）
    // 3. 记录 I/O 开始时间
    // 4. 在 vfs_read 返回时计算延迟
    
    // 占位实现
    0
}

/// Hook vfs_write - 文件系统写入
#[kprobe(name = "vfs_write")]
pub fn vfs_write(ctx: ProbeContext) -> u32 {
    // TODO: 类似 vfs_read 的实现
    0
}

/// Hook blk_mq_submit_bio - 块设备层提交
#[kprobe(name = "blk_mq_submit_bio")]
pub fn blk_mq_submit_bio(ctx: ProbeContext) -> u32 {
    // TODO: 提取 I/O 大小和延迟
    // 1. 从 bio 结构体提取大小
    // 2. 记录提交时间
    // 3. 在完成时计算延迟
    
    // 占位实现
    0
}
