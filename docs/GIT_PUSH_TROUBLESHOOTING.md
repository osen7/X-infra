# Git Push 故障排除指南

## 问题：无法连接到 GitHub (端口 443)

### 解决方案 1：使用 SSH 代替 HTTPS（推荐）

如果 443 端口被阻止，可以改用 SSH（端口 22）：

```bash
# 1. 检查是否已有 SSH 密钥
ls ~/.ssh/id_rsa.pub

# 2. 如果没有，生成 SSH 密钥
ssh-keygen -t rsa -b 4096 -C "your_email@example.com"

# 3. 将公钥添加到 GitHub
cat ~/.ssh/id_rsa.pub
# 复制输出，在 GitHub Settings > SSH and GPG keys 中添加

# 4. 更改远程仓库 URL 为 SSH
git remote set-url origin git@github.com:osen7/X-infra.git

# 5. 测试连接
ssh -T git@github.com

# 6. 推送
git push
```

### 解决方案 2：配置 HTTP/HTTPS 代理

如果有可用的代理：

```bash
# 设置 HTTP 代理
git config --global http.proxy http://proxy.example.com:8080
git config --global https.proxy http://proxy.example.com:8080

# 或者只对 GitHub 设置代理
git config --global http.https://github.com.proxy http://proxy.example.com:8080

# 取消代理设置
git config --global --unset http.proxy
git config --global --unset https.proxy
```

### 解决方案 3：使用 GitHub CLI

```bash
# 安装 GitHub CLI (gh)
# Windows: winget install GitHub.cli

# 登录
gh auth login

# 推送（会自动使用认证）
git push
```

### 解决方案 4：使用镜像或 VPN

- 使用 VPN 服务
- 使用 GitHub 镜像（如 gitee 镜像）

### 解决方案 5：稍后重试

网络问题可能是临时的，可以稍后重试：

```bash
# 等待一段时间后重试
git push
```

## 当前状态

你的更改已经成功提交到本地仓库：

```
[main b007ad4] v0.2.0
 20 files changed, 1278 insertions(+), 163 deletions(-)
```

包括：
- 规则引擎（rules/）
- 场景化分析（src/scene/）
- eBPF 探针框架（examples/ebpf/）

这些更改在本地是安全的，可以在网络恢复后推送。

## 验证本地提交

```bash
# 查看提交历史
git log --oneline -5

# 查看更改统计
git show --stat HEAD
```
