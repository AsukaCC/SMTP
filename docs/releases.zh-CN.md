# 版本发布、更新和回退

本工具在 Linux / macOS 上使用 Python 3.11+ 和 Docker Compose v2。每个安装目录有独立的锁、状态和 Compose 项目名，支持同服务器多实例；各实例必须使用不同 HTTP/SMTP 端口。安装目录以解析后的绝对路径标识，软链接不会绕过同目录限制。

## 后台检查版本

在“服务设置”的“版本更新”中查看当前版本，点击“检查更新”读取 GitHub 最新正式 Release。默认仓库为 `AsukaCC/SMTP`；可在服务器 `.env` 设置 `UPDATE_REPOSITORY=owner/repository`，留空则关闭检查。配置修改后需要重新创建应用容器。版本号使用本项目的版本，不比较上游 BunMail 的版本。

只有管理员可以访问更新接口，发信 API 密钥无权访问。点击“检查更新”时才查询 GitHub，不在启动时或按定时任务自动检查。需能访问 `https://api.github.com`；结果在当前进程内保留，每 60 秒最多访问一次 GitHub，应用重启后显示未检查。页面轮询只读取缓存及安装任务状态，不触发版本检查；管理员确认安装后仍会重新核对发布与构建包。网络故障、GitHub 限流或无效发布数据会显示检查失败，不能据此判断已是最新。404 显示未找到可访问的正式发布，可能是没有 Release、仓库不存在或非公开仓库。当前不支持私有仓库令牌。

启用下述宿主机更新服务后，匹配的构建包会显示“安装更新”。点击并确认后，后台创建持久化任务，下载包、验证 SHA256 和清单、备份数据库及 `.env`、切换容器并等待健康检查。页面显示进度，服务重启期间自动重连。没有匹配附件、没有 GitHub SHA256 摘要、更新服务离线或仓库不一致时，安装按钮不可用。只检查不自动安装，必须由管理员点击确认。

“发布页面”打开对应版本；“服务器命令”保留受管更新、回退和恢复入口。GitHub 自动提供的 Source code 压缩包不是受管构建包。自动下载仅允许配置仓库的正式 Release 附件和 GitHub 资源域名，不接受网页提交的 URL、目录或命令。

版本提示仅说明有新版本，不代表数据库迁移兼容。实际更新仍由部署工具校验迁移指纹、备份与健康检查。检查更新不会提交代码、发布镜像或创建 GitHub Release。

## 生成版本包

仓库的 Release 工作流在推送 `v<版本号>` 标签后，在 Linux amd64、arm64 原生运行器从该标签源码打包。两个架构均通过安装、更新、回退及备份恢复测试，且 Rust/数据库集成检查通过后，才发布正式 Release、两份安装包及 `SHA256SUMS`。标签必须与 `Cargo.toml` 版本一致，并提供 `docs/release-notes/<版本号>.zh-CN.md` 发布说明。

在源码仓库完成检查后，把 `Cargo.toml` / `Cargo.lock` 更新到发布版本，再运行：

```bash
python3 scripts/deploy.py package 0.2.0 --output /tmp/bunmail-packages
```

生成 `/tmp/bunmail-packages/0.2.0/`，包含应用 Docker 镜像、Compose 配置、安装工具和 SHA256 清单，并生成 `bunmail-rs-0.2.0-linux-amd64.tar.gz` 或 `bunmail-rs-0.2.0-linux-arm64.tar.gz`。将对应压缩包作为 `v0.2.0` 正式 GitHub Release 的附件上传，等待 GitHub API 提供 `state=uploaded` 及 `digest=sha256:...`。可分别在两种 Linux 架构构建后上传两个附件；不要改名或用源码包替代。同版本镜像或包已存在时拒绝覆盖。

这是本地打包命令，不会提交 Git、推送镜像或发布 GitHub Release。校验和用于检测损坏，不是发布者签名；只从可信渠道接收整套发布目录。PostgreSQL 镜像首次安装仍需联网拉取，因此该包不是完整离线依赖包。

## 首次受管安装

将安装包放在安装目录之外，例如 `/tmp/bunmail-packages/0.2.0`：

```bash
python3 /tmp/bunmail-packages/0.2.0/setup.py --directory /srv/bunmail
```

编辑 `/srv/bunmail/.env` 完成服务器配置，再运行：

```bash
python3 /tmp/bunmail-packages/0.2.0/deploy.py --directory /srv/bunmail install /tmp/bunmail-packages/0.2.0
python3 /tmp/bunmail-packages/0.2.0/deploy.py --directory /srv/bunmail status
```

再次 install 会拒绝；目录已有 Compose 容器或同项目数据库卷时也会拒绝。不能直接将源码部署覆盖成受管安装，应先制定现有数据的迁移方案。

## 启用后台安装

仅适用于 Linux Docker 受管安装。宿主机需 Python 3.11+、Docker Compose v2、访问 GitHub 的网络，以及足够存放构建包、镜像和数据库备份的磁盘空间。应用容器不挂载 Docker socket；宿主机更新进程通过专用 Unix socket 只提供状态查询和安装已发布版本两个操作。

首次启用时，在宿主机运行当前可信安装包中的脚本（会重建应用容器以挂载专用 socket 目录）：

```bash
sudo python3 /tmp/bunmail-packages/0.2.0/updater.py --directory /srv/bunmail enable
```

该命令把可信更新引擎复制到 `.bunmail/updater-service/`，不随应用容器切换而停止，也不会直接执行下载包中的 Python 文件。默认 socket 组为容器 GID `10001`；自定义容器 GID 时同时给 `enable` 和 `serve` 传 `--socket-gid`。受管部署根目录及 `.env` 必须只允许可信服务器管理员写入。

创建 `/etc/systemd/system/bunmail-updater.service`，目录按实际实例修改：

```ini
[Unit]
Description=BunMail managed updater
After=docker.service network-online.target
Requires=docker.service
Wants=network-online.target

[Service]
Type=simple
User=root
UMask=0077
ExecStart=/usr/bin/python3 /srv/bunmail/.bunmail/updater-service/updater.py --directory /srv/bunmail serve
Restart=on-failure
RestartSec=5
TimeoutStopSec=240

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now bunmail-updater.service
```

之后管理员直接在“服务设置 → 版本更新”操作。多实例使用不同的安装目录和 systemd 服务名；socket、任务状态、锁和备份均按实例隔离。修改 `UPDATE_REPOSITORY` 后需重建应用容器并重启该更新服务。更新引擎自身需由服务器管理员从可信包重新启用和重启服务；应用更新不会自动替换正在运行的宿主机引擎。

任务保存在 `.bunmail/update-job.json`。更新进程中断后不会自动重试安装，页面显示中断或未完成部署；查看 `deploy.py status`，存在 pending 时按下文运行 `resume`。更新前会检查包空间，但数据库备份所需空间取决于实际数据量；备份失败不会启动新版本。

## 更新和回退

```bash
python3 /tmp/bunmail-packages/0.2.1/deploy.py --directory /srv/bunmail update /tmp/bunmail-packages/0.2.1
python3 /tmp/bunmail-packages/0.2.1/deploy.py --directory /srv/bunmail rollback
```

工具校验包、镜像版本与数据库迁移指纹，停止应用并等待正在进行的发送退出，备份 PostgreSQL 与 `.env`，然后启动新镜像并等待健康检查。成功后只保留当前版本与上一版本的受管发布文件。更新/回退均进行备份；备份失败不启动目标版本。

新版本未健康时自动启动原镜像，保留数据和失败记录。只允许数据库迁移指纹相同的更新和回退，因此自动恢复无需把数据库倒退到旧快照；新版本运行期间已经接受的数据仍保留。数据库迁移变化时工具会拒绝操作，须单独制定并验证迁移及恢复流程。

`.bunmail/releases/` 中只保存两套轻量发布文件，不复制 `image.tar.gz`；用户下载的源包可在成功安装后自行移走。备份保存在 `.bunmail/backups/`，含私钥加密密钥和数据库内容，权限受限。备份不会自动删除，Docker 共享镜像也不会自动删除，避免破坏其他目录实例；根据磁盘容量和恢复策略另行归档。

如果操作中断或新旧版本均无法启动：

```bash
python3 /tmp/bunmail-packages/0.2.1/deploy.py --directory /srv/bunmail status
python3 /tmp/bunmail-packages/0.2.1/deploy.py --directory /srv/bunmail resume
```

`resume` 重试待完成操作，已有备份会复用。旧镜像缺失时在停止应用前失败；重新导入该版本的镜像包后再恢复。目录锁由系统在进程退出后释放，无需删除锁文件。不要删除状态文件或数据库卷重装。

## 修改配置

编辑 `.env` 后使用当前版本工具：

```bash
python3 /srv/bunmail/.bunmail/releases/0.2.0/deploy.py --directory /srv/bunmail reconfigure
```

启用 SMTP 时按部署文档准备 `certs/`、配置端口，再创建空文件 `/srv/bunmail/.smtp-enabled` 并 reconfigure；禁用时删除该标记后重新执行。配置变更不属于镜像更新，不承诺自动恢复旧配置，请保留修改前的配置副本。

以上路径中的版本号以 `status` 显示的 current 为准。不要把部署目录的绝对路径重命名后直接启动，避免创建新项目和空数据库卷。
