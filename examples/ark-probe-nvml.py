#!/usr/bin/env python3
"""
Ark NVML 探针：真实抓取 NVIDIA GPU 利用率
依赖: pynvml (pip install pynvml)

输出格式：每行一个 JSON 对象（JSONL）
"""

import json
import time
import sys
import os

try:
    import pynvml
except ImportError:
    print("错误: 未安装 pynvml，请运行: pip install pynvml", file=sys.stderr)
    sys.exit(1)

# 全局变量
nvml_initialized = False
gpu_count = 0

def init_nvml():
    """初始化 NVML"""
    global nvml_initialized, gpu_count
    try:
        pynvml.nvmlInit()
        nvml_initialized = True
        gpu_count = pynvml.nvmlDeviceGetCount()
        return True
    except pynvml.NVMLError as e:
        print(f"错误: NVML 初始化失败: {e}", file=sys.stderr)
        return False

def get_gpu_utilization(handle):
    """获取 GPU 利用率（0-100）"""
    try:
        util = pynvml.nvmlDeviceGetUtilizationRates(handle)
        return util.gpu
    except pynvml.NVMLError:
        return None

def get_gpu_memory(handle):
    """获取 GPU 显存使用情况（MB）"""
    try:
        mem_info = pynvml.nvmlDeviceGetMemoryInfo(handle)
        used_mb = mem_info.used // (1024 * 1024)
        total_mb = mem_info.total // (1024 * 1024)
        usage_percent = int((mem_info.used / mem_info.total) * 100) if mem_info.total > 0 else 0
        return {
            "used_mb": used_mb,
            "total_mb": total_mb,
            "usage_percent": usage_percent
        }
    except pynvml.NVMLError:
        return None

def get_gpu_processes(handle):
    """获取使用该 GPU 的进程列表"""
    try:
        procs = pynvml.nvmlDeviceGetComputeRunningProcesses(handle)
        return [(proc.pid, proc.usedGpuMemory // (1024 * 1024)) for proc in procs]
    except pynvml.NVMLError:
        return []

def get_gpu_temperature(handle):
    """获取 GPU 温度（摄氏度）"""
    try:
        temp = pynvml.nvmlDeviceGetTemperature(handle, pynvml.NVML_TEMPERATURE_GPU)
        return temp
    except pynvml.NVMLError:
        return None

def get_gpu_power(handle):
    """获取 GPU 功耗（瓦特）"""
    try:
        power = pynvml.nvmlDeviceGetPowerUsage(handle) / 1000.0  # 转换为瓦特
        return int(power)
    except pynvml.NVMLError:
        return None

def get_gpu_errors(handle, gpu_id):
    """检查 GPU 错误（XID 错误等）"""
    errors = []
    try:
        # 检查 ECC 错误
        ecc_errors = pynvml.nvmlDeviceGetTotalEccErrors(
            handle, pynvml.NVML_MEMORY_ERROR_TYPE_UNCORRECTED,
            pynvml.NVML_VOLATILE_ECC
        )
        if ecc_errors > 0:
            errors.append(f"ECC_UNCORRECTED:{ecc_errors}")
    except pynvml.NVMLError:
        pass
    
    try:
        # 检查 XID 错误（通过驱动日志，这里简化处理）
        # 实际生产环境可能需要读取 /var/log/syslog 或 dmesg
        pass
    except:
        pass
    
    return errors

def generate_events():
    """生成所有 GPU 事件"""
    events = []
    current_ts = int(time.time() * 1000)
    
    for gpu_id in range(gpu_count):
        try:
            handle = pynvml.nvmlDeviceGetHandleByIndex(gpu_id)
            entity_id = f"gpu-{gpu_id:02d}"
            
            # 1. GPU 利用率事件
            util = get_gpu_utilization(handle)
            if util is not None:
                events.append({
                    "ts": current_ts,
                    "event_type": "compute.util",
                    "entity_id": entity_id,
                    "job_id": None,
                    "pid": None,
                    "value": str(util)
                })
            
            # 2. GPU 显存使用率事件
            mem_info = get_gpu_memory(handle)
            if mem_info is not None:
                events.append({
                    "ts": current_ts,
                    "event_type": "compute.mem",
                    "entity_id": entity_id,
                    "job_id": None,
                    "pid": None,
                    "value": str(mem_info["usage_percent"])
                })
            
            # 3. 为每个使用 GPU 的进程生成事件
            processes = get_gpu_processes(handle)
            for pid, mem_mb in processes:
                # 创建 Consumes 关系：进程消耗 GPU
                events.append({
                    "ts": current_ts,
                    "event_type": "compute.util",
                    "entity_id": entity_id,
                    "job_id": None,
                    "pid": pid,
                    "value": str(util) if util is not None else "0"
                })
            
            # 4. GPU 温度事件（可选，可以作为额外指标）
            temp = get_gpu_temperature(handle)
            if temp is not None and temp > 85:  # 高温告警
                events.append({
                    "ts": current_ts,
                    "event_type": "error.hw",
                    "entity_id": entity_id,
                    "job_id": None,
                    "pid": None,
                    "value": f"TEMP_HIGH:{temp}"
                })
            
            # 5. GPU 错误事件
            errors = get_gpu_errors(handle, gpu_id)
            for error in errors:
                events.append({
                    "ts": current_ts,
                    "event_type": "error.hw",
                    "entity_id": entity_id,
                    "job_id": None,
                    "pid": None,
                    "value": error
                })
            
        except pynvml.NVMLError as e:
            # GPU 访问失败，记录错误事件
            events.append({
                "ts": current_ts,
                "event_type": "error.hw",
                "entity_id": f"gpu-{gpu_id:02d}",
                "job_id": None,
                "pid": None,
                "value": f"NVML_ERROR:{str(e)}"
            })
    
    return events

def main():
    """主循环：定期输出 GPU 事件"""
    # 初始化 NVML
    if not init_nvml():
        sys.exit(1)
    
    if gpu_count == 0:
        print("警告: 未检测到 NVIDIA GPU", file=sys.stderr)
        sys.exit(1)
    
    print(f"已检测到 {gpu_count} 个 GPU，开始监控...", file=sys.stderr)
    
    # 获取采样间隔（秒），默认 1 秒
    interval = float(os.environ.get("XCTL_NVML_INTERVAL", "1.0"))
    
    try:
        while True:
            events = generate_events()
            
            # 输出所有事件（JSONL 格式）
            for event in events:
                print(json.dumps(event, ensure_ascii=False))
                sys.stdout.flush()
            
            time.sleep(interval)
            
    except KeyboardInterrupt:
        pass
    except BrokenPipeError:
        # 父进程关闭了管道
        pass
    finally:
        if nvml_initialized:
            try:
                pynvml.nvmlShutdown()
            except:
                pass

if __name__ == "__main__":
    main()
