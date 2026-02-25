#!/usr/bin/env python3
"""
示例探针脚本：模拟 GPU 和进程事件
输出格式：每行一个 JSON 对象（JSONL）
"""

import json
import time
import random
import sys

def generate_event():
    """生成随机事件（使用蛇形小写加点格式）"""
    event_types = [
        {"type": "compute.util", "entity": f"gpu-{random.randint(0, 7):02d}", "value": str(random.randint(0, 100))},
        {"type": "process.state", "entity": f"proc-{random.randint(1000, 9999)}", "value": "start"},
    ]
    
    event_template = random.choice(event_types)
    
    event = {
        "ts": int(time.time() * 1000),
        "event_type": event_template["type"],
        "entity_id": event_template["entity"],
        "job_id": f"job-{random.randint(1000, 9999)}" if random.random() > 0.5 else None,
        "pid": random.randint(1000, 9999) if random.random() > 0.5 else None,
        "value": event_template["value"]
    }
    
    return event

def main():
    """主循环：每秒输出 1-3 个事件"""
    try:
        while True:
            count = random.randint(1, 3)
            for _ in range(count):
                event = generate_event()
                # 输出 JSONL 格式（每行一个 JSON）
                print(json.dumps(event, ensure_ascii=False))
                sys.stdout.flush()
            
            time.sleep(1)
    except KeyboardInterrupt:
        pass
    except BrokenPipeError:
        # 父进程关闭了管道
        pass

if __name__ == "__main__":
    main()
