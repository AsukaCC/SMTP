# 版本发布、更新和回退

本工具在 Linux / macOS 上使用 Python 3.11+ 和 Docker Compose v2。每个安装目录有独立的锁、状态和 Compose 项目名，支持同服务器多实例；各实例必须使用不同 HTTP/SMTP 端口。安装目录以解析后的绝对路径标识，软链接不会绕过同目录限制。

## 生成版本包

在源码仓库完成检查后，把 `Cargo.toml` / `Cargo.lock` 更新到发布版本，再运行：

```bash
python3 scripts/deploy.py package 0.1.0 --output /tmp/bunmail-packages
```

生成 `/tmp/bunmail-packages/0.1.0/`，包含应用 Docker 镜像、Compose 配置、安装工具和 SHA256 清单。同版本镜像或包已存在时拒绝覆盖。包只能用于其清单标明的 CPU 架构。

这是本地打包命令，不会提交 Git、推送镜像或发布 GitHub Release。校验和用于检测损坏，不是发布者签名；只从可信渠道接收整套发布目录。PostgreSQL 镜像首次安装仍需联网拉取，因此该包不是完整离线依赖包。

## 首次受管安装

将安装包放在安装目录之外，例如 `/tmp/bunmail-packages/0.1.0`：

```bash
python3 /tmp/bunmail-packages/0.1.0/setup.py --directory /srv/bunmail
```

编辑 `/srv/bunmail/.env` 完成服务器配置，再运行：

```bash
python3 /tmp/bunmail-packages/0.1.0/deploy.py --directory /srv/bunmail install /tmp/bunmail-packages/0.1.0
python3 /tmp/bunmail-packages/0.1.0/deploy.py --directory /srv/bunmail status
```

再次 install 会拒绝；目录已有 Compose 容器或同项目数据库卷时也会拒绝。不能直接将源码部署覆盖成受管安装，应先制定现有数据的迁移方案。

## 更新和回退

```bash
python3 /tmp/bunmail-packages/0.1.1/deploy.py --directory /srv/bunmail update /tmp/bunmail-packages/0.1.1
python3 /tmp/bunmail-packages/0.1.1/deploy.py --directory /srv/bunmail rollback
```

工具校验包、镜像版本与数据库迁移指纹，停止应用并等待正在进行的发送退出，备份 PostgreSQL 与 `.env`，然后启动新镜像并等待健康检查。成功后只保留当前版本与上一版本的受管发布文件。更新/回退均进行备份；备份失败不启动目标版本。

新版本未健康时自动启动原镜像，保留数据和失败记录。只允许数据库迁移指纹相同的更新和回退，因此自动恢复无需把数据库倒退到旧快照；新版本运行期间已经接受的数据仍保留。数据库迁移变化时工具会拒绝操作，须单独制定并验证迁移及恢复流程。

`.bunmail/releases/` 中只保存两套轻量发布文件，不复制 `image.tar.gz`；用户下载的源包可在成功安装后自行移走。备份保存在 `.bunmail/backups/`，含私钥加密密钥和数据库内容，权限受限。备份不会自动删除，Docker 共享镜像也不会自动删除，避免破坏其他目录实例；根据磁盘容量和恢复策略另行归档。

如果操作中断或新旧版本均无法启动：

```bash
python3 /tmp/bunmail-packages/0.1.1/deploy.py --directory /srv/bunmail status
python3 /tmp/bunmail-packages/0.1.1/deploy.py --directory /srv/bunmail resume
```

`resume` 重试待完成操作，已有备份会复用。旧镜像缺失时在停止应用前失败；重新导入该版本的镜像包后再恢复。目录锁由系统在进程退出后释放，无需删除锁文件。不要删除状态文件或数据库卷重装。

## 修改配置

编辑 `.env` 后使用当前版本工具：

```bash
python3 /srv/bunmail/.bunmail/releases/0.1.0/deploy.py --directory /srv/bunmail reconfigure
```

启用 SMTP 时按部署文档准备 `certs/`、配置端口，再创建空文件 `/srv/bunmail/.smtp-enabled` 并 reconfigure；禁用时删除该标记后重新执行。配置变更不属于镜像更新，不承诺自动恢复旧配置，请保留修改前的配置副本。

密码恢复：

```bash
python3 /srv/bunmail/.bunmail/releases/0.1.0/reset-password.py --directory /srv/bunmail
```

以上路径中的版本号以 `status` 显示的 current 为准。不要把部署目录的绝对路径重命名后直接启动，避免创建新项目和空数据库卷。
