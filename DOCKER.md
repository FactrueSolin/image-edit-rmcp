# Docker 部署指南（image-edit-rmcp）

本文档介绍如何使用 Docker / Docker Compose 部署并运行 **image-edit-rmcp**（Rust MCP 服务）。项目已提供镜像构建与编排配置，分别见 [`Dockerfile`](Dockerfile:1) 与 [`docker-compose.yml`](docker-compose.yml:1)。

> 参考：环境变量示例见 [`.env.example`](.env.example:1)，项目功能与 HTTP 入口说明见 [`README.md`](README.md:1)。

---

## 1. 概述

使用 Docker 部署的主要优势：

- **环境一致性**：运行时依赖被封装在镜像内，减少“本地能跑、线上不能跑”。
- **部署简单**：一条命令即可构建并启动，适合服务器/CI/CD。
- **隔离与可移植**：服务、依赖与宿主系统隔离，可在不同机器快速迁移。

本项目镜像采用 **多阶段构建**：在 builder 阶段编译 Rust 可执行文件，再拷贝到精简运行时镜像中（见 [`Dockerfile`](Dockerfile:1)）。

---

## 2. 前置条件

- Docker（建议使用较新版本）
- Docker Compose（Docker Desktop 已内置；或使用 `docker compose` 子命令）

验证安装：

```bash
docker --version
docker compose version
```

---

## 3. 快速开始（最简单启动）

> 说明：当前 Compose 配置仅定义了构建与环境变量注入（见 [`docker-compose.yml`](docker-compose.yml:1)），**未映射端口**。这适合把服务作为“同一 Compose 网络内的内部服务”使用。

1）准备环境变量文件：

```bash
cp .env.example .env
```

2）构建并启动：

```bash
docker compose up -d --build
```

3）查看日志：

```bash
docker compose logs -f
```

如需从宿主机访问服务端口，请参考下文“运行容器”中的端口映射说明。

---

## 4. 环境变量配置

Compose 会从 `.env` 文件读取环境变量（见 [`docker-compose.yml`](docker-compose.yml:8)）。建议从模板复制（见 [`.env.example`](.env.example:1)）：

```bash
cp .env.example .env
```

常用变量说明（以 [`.env.example`](.env.example:1) 为准）：

- `MODELSCOPE_API_KEY`
  - **必填（使用 AI 能力时）**：ModelScope 的 API Key。
- `MCP_PORT`
  - 服务监听端口（默认 `3000`，见 [`.env.example`](.env.example:2)）。
  - 使用 Docker 时，若要从宿主机访问，需要同时配置 **容器端口监听** 与 **宿主机端口映射**（见下文）。
- `SECRET_KEY`
  - **必填**：用于图床上传与 MCP 路由前缀。
  - MCP 地址格式示例见 [`README.md`](README.md:140)：`http://localhost:3000/{SECRET_KEY}/mcp`
- `CACHE_DIR`
  - 缓存目录（默认 `~/.cache/image-edit-rmcp`，见 [`.env.example`](.env.example:4)）。
  - **Docker 场景建议改为容器内路径**（例如 `/data`），并通过 volume 挂载实现持久化（见“数据持久化”）。
- `CACHE_URL`
  - 用于生成公开访问链接的基础 URL（默认 `http://localhost:3000`，见 [`.env.example`](.env.example:5)）。
  - 若部署在服务器或反向代理后，请改为外部可访问的 URL（例如 `https://your.domain`）。

---

## 5. 构建镜像（手动）

使用项目内 [`Dockerfile`](Dockerfile:1) 构建：

```bash
docker build -t image-edit-rmcp:local .
```

说明：

- builder 阶段基于 `rust:1.85-bookworm` 编译 release 二进制（见 [`Dockerfile`](Dockerfile:1)）。
- 运行时阶段基于 `debian:bookworm-slim`，仅包含证书与可执行文件（见 [`Dockerfile`](Dockerfile:11)）。

---

## 6. 运行容器

### 6.1 使用 `docker run`

适用于：希望**直接在宿主机通过端口访问**服务。

```bash
docker run -d \
  --name image-edit-rmcp \
  --env-file .env \
  -p 3000:3000 \
  image-edit-rmcp:local
```

如需启用缓存持久化（推荐），并将 `CACHE_DIR` 指向容器内固定目录：

```bash
docker run -d \
  --name image-edit-rmcp \
  --env-file .env \
  -e CACHE_DIR=/data \
  -p 3000:3000 \
  -v image-edit-rmcp-cache:/data \
  image-edit-rmcp:local
```

> 注意：如果你把 `MCP_PORT` 改成了其他端口，请同步修改 `-p <host>:<container>` 的端口值。

启动后访问（以 `MCP_PORT=3000` 为例）：

- Web/图床：`http://localhost:3000/upload`
- MCP（httpstream）：`http://localhost:3000/{SECRET_KEY}/mcp`（格式见 [`README.md`](README.md:140)）

### 6.2 使用 Docker Compose

当前 Compose 服务定义见 [`docker-compose.yml`](docker-compose.yml:1)：

```bash
docker compose up -d --build
```

由于默认未配置端口映射，你可以在本地将 [`docker-compose.yml`](docker-compose.yml:1) 的 service 增加 `ports`（示例）：

```yaml
services:
  image-edit-rmcp:
    # ...
    ports:
      - "3000:3000"
```

如果还希望把缓存目录持久化，建议同时：

- 在 `.env` 中设置 `CACHE_DIR=/data`
- 在 Compose 中挂载 volume（示例见下文“数据持久化”）

---

## 7. 数据持久化（volume 挂载）

项目会将处理结果与元数据写入 `CACHE_DIR`（示例默认见 [`.env.example`](.env.example:4)）。在 Docker 场景下，建议：

1. 将 `.env` 中的 `CACHE_DIR` 改为容器内目录（例如 `/data`）
2. 将该目录挂载到 Docker volume 或宿主机目录

### 7.1 Docker volume（推荐）

`docker run` 示例见 [“6.1 使用 docker run”](DOCKER.md:1) 中的 `-v image-edit-rmcp-cache:/data`。

Compose 示例（在 [`docker-compose.yml`](docker-compose.yml:1) 的基础上本地补充）：

```yaml
services:
  image-edit-rmcp:
    # ...
    environment:
      - CACHE_DIR=/data
    volumes:
      - image-edit-rmcp-cache:/data

volumes:
  image-edit-rmcp-cache:
```

### 7.2 挂载宿主机目录

```bash
docker run -d \
  --name image-edit-rmcp \
  --env-file .env \
  -e CACHE_DIR=/data \
  -p 3000:3000 \
  -v "$(pwd)/data:/data" \
  image-edit-rmcp:local
```

---

## 8. 常用命令

### 8.1 Docker Compose

```bash
# 启动（后台）
docker compose up -d

# 重新构建并启动
docker compose up -d --build

# 查看服务状态
docker compose ps

# 查看日志
docker compose logs -f

# 停止
docker compose stop

# 停止并删除容器/网络（不会删除命名 volume）
docker compose down
```

### 8.2 Docker（单容器）

```bash
# 查看容器
docker ps

# 查看日志
docker logs -f image-edit-rmcp

# 进入容器（排查用）
docker exec -it image-edit-rmcp bash

# 重启
docker restart image-edit-rmcp

# 停止并删除
docker rm -f image-edit-rmcp
```

---

## 9. 故障排除

### 9.1 Compose 启动后，宿主机访问不到 `localhost:3000`

原因：默认 Compose 配置未暴露端口（见 [`docker-compose.yml`](docker-compose.yml:1)）。

解决：

- 使用 `docker run -p 3000:3000 ...`（见 [“6.1 使用 docker run”](DOCKER.md:1)）；或
- 在 Compose 中添加 `ports` 映射（见 [“6.2 使用 Docker Compose”](DOCKER.md:1)）。

### 9.2 AI 功能不可用 / 调用失败

排查：

1. 确认 `.env` 中已设置 `MODELSCOPE_API_KEY`（示例见 [`.env.example`](.env.example:1)）。
2. 查看日志定位错误：

```bash
docker compose logs -f
# 或
docker logs -f image-edit-rmcp
```

### 9.3 上传/访问链接不正确（URL 指向 localhost 或不可访问）

原因：`CACHE_URL` 用于生成对外可访问链接（见 [`.env.example`](.env.example:5)）。

解决：将 `CACHE_URL` 改为真实对外域名/地址，例如：

```env
CACHE_URL=https://your.domain
```

### 9.4 缓存目录无写入权限 / 容器重启后缓存丢失

原因：

- `CACHE_DIR` 指向了容器内临时文件系统，容器重建会丢失数据；或
- 挂载的宿主机目录权限不正确。

解决：

- 使用命名 volume（见“数据持久化”）；或
- 确保挂载目录可写，并将 `CACHE_DIR` 指向挂载点（例如 `/data`）。

