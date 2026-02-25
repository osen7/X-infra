# 多阶段构建 Dockerfile：构建 xctl 和 xctl-hub
# 使用方法：
#   docker build -t xctl:v1.0.0 --build-arg BINARY=xctl -f deploy/Dockerfile .
#   docker build -t xctl-hub:v1.0.0 --build-arg BINARY=xctl-hub -f deploy/Dockerfile .

# 构建阶段
FROM rust:1.70-slim as builder

# 安装构建依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# 复制 Cargo 配置文件
COPY Cargo.toml Cargo.toml
COPY core/Cargo.toml core/Cargo.toml
COPY agent/Cargo.toml agent/Cargo.toml
COPY hub/Cargo.toml hub/Cargo.toml

# 复制源代码
COPY core core
COPY agent agent
COPY hub hub

# 构建参数：指定要构建的二进制文件
ARG BINARY=xctl

# 构建二进制文件
RUN if [ "$BINARY" = "xctl" ]; then \
        cargo build --release -p xctl; \
    elif [ "$BINARY" = "xctl-hub" ]; then \
        cargo build --release -p xctl-hub; \
    else \
        echo "Unknown BINARY: $BINARY"; exit 1; \
    fi

# 运行阶段
FROM debian:bookworm-slim

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# 创建非 root 用户
RUN useradd -m -u 1000 xctl

# 从构建阶段复制二进制文件
ARG BINARY=xctl
COPY --from=builder /build/target/release/$BINARY /opt/xctl/$BINARY

# 设置权限
RUN chmod +x /opt/xctl/$BINARY && \
    chown -R xctl:xctl /opt/xctl

# 创建必要的目录
RUN mkdir -p /opt/xctl/rules /opt/xctl/probes /var/run/xctl && \
    chown -R xctl:xctl /opt/xctl /var/run/xctl

# 切换到非 root 用户
USER xctl

WORKDIR /opt/xctl

# 设置 PATH
ENV PATH="/opt/xctl:${PATH}"

# 暴露端口（Hub 使用）
EXPOSE 8080 8081

# 入口点（根据 BINARY 参数动态设置）
# 注意：实际使用时需要在 docker run 时指定正确的入口点
# 或使用不同的 Dockerfile（如 Dockerfile.hub）
