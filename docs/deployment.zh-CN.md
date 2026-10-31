# 服务器部署

## 准备服务器

建议使用 Linux、固定公网 IP、至少 2 GiB 内存、Docker Compose v2、Python 3.11+。源码编译建议 4 GiB 内存；离线应用包与目标 CPU 架构必须一致。当前 Docker 构建在本机架构执行，不能直接把 ARM64 包用于 AMD64 服务器。

向服务器供应商确认出站 TCP 25 已放行，建议选择支持设置公网 IP PTR 的供应商；未配置 PTR 时应用只提示投递风险。无须向公网开放入站 25；本项目不接收互联网邮件。HTTP 默认只绑定服务器 `127.0.0.1:3000`，通过现有 Nginx / Caddy 提供 HTTPS。可选入站 587 仅用于认证 SMTP 提交。

## 配置与启动

```bash
python3 scripts/setup.py
```

编辑生成的 `.env`，至少配置：

| 变量 | 内容 |
| --- | --- |
| `APP_ENV` | `production` |
| `APP_URL` | 管理后台 HTTPS origin，例如 `https://mail-admin.example.com`，不能含路径或查询参数 |
| `OWNER_EMAIL` | 管理员邮箱，仅首次初始化生效 |
| `BOOTSTRAP_PASSWORD` | 自动生成的初始管理员密码，仅首次初始化生效 |
| `MAIL_HOSTNAME` | SMTP EHLO 身份，例如 `mail.example.com` |
| `PUBLIC_IP` | 实际出站公网 IPv4 或 IPv6 |
| `DELIVERY_ENABLED` | 初始保持 `false`；后台未保存投递开关时使用此值 |

`POSTGRES_PASSWORD` 和 `DKIM_ENCRYPTION_KEY` 已自动生成。加密密钥不可丢失或随意替换，否则现有 DKIM 私钥无法解密。不要公开 `.env`、证书私钥、数据库备份或运行 `docker compose config` 输出完整配置；校验时用 `--quiet`。

源码部署：

```bash
docker compose config --quiet
docker compose up -d --build --wait
```

需要自动更新和回退时，建议首次就使用[受管安装方式](releases.zh-CN.md)，两种部署方式不要混用。

## 源码部署更新

后台“服务设置 → 版本更新”可检查正式发布并打开发布页面。当前源码部署应先同步经过审核的目标版本源码，保留服务器原有 `.env`、证书和 Docker 项目名；不要重新运行初始化或删除数据卷。确认目标版本的数据库迁移兼容后，在原部署目录执行：

```bash
set -eu
umask 077
backup_dir="backups/$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$backup_dir"
cp .env "$backup_dir/config.env"
docker compose exec -T db pg_dump -U bunmail -d bunmail -Fc > "$backup_dir/database.dump"
test -s "$backup_dir/database.dump"
docker compose config --quiet
docker compose build app
docker compose up -d --no-build --wait app
```

备份含数据库和 DKIM 加密密钥，须移至受保护的备份存储，不提交到仓库。若使用 SMTP 覆盖文件，以上 Compose 命令都须加上原有的 `-f docker-compose.yml -f docker-compose.smtp.yml`。源码部署的这些命令不提供自动回退；需要自动备份、失败恢复和两版本保留时，使用受管发布流程，但不能直接覆盖已有源码实例。

## HTTPS 代理

Caddy 配置示例：

```caddyfile
mail-admin.example.com {
    reverse_proxy 127.0.0.1:3000
}
```

使用其他反向代理时保留正常的 `Origin` 请求头，浏览器必须通过 `APP_URL` 访问。登录限流按 TCP 对端地址计算，代理后的所有登录共享该限额；建议只通过 VPN 或受限网络访问管理后台。

## DNS 与首次投递

1. 为 `MAIL_HOSTNAME` 添加 A 或 AAAA，指向 `PUBLIC_IP`。
2. 建议在服务器供应商的控制台把 `PUBLIC_IP` 的 PTR 设置为 `MAIL_HOSTNAME`。PTR 不能通过普通域名 DNS 面板代设；缺失、不匹配或查询异常时显示告警，不阻止发送。
3. 登录后台添加发信域名，根据页面记录设置 SPF、DKIM 和 DMARC。长 DKIM TXT 可由 DNS 平台分成多个字符串，但必须仍是同一条记录。
4. 点击“检查 DNS”，确认 SPF、DKIM、DMARC、A/AAAA 四项通过。PTR 为可选告警；域名列表、发信入口和投递队列统一按最近 24 小时的这四项检查判断，不再依赖旧版本保存的 `verified` 标记。升级后，如果原有检查仍有效且四项通过，即使旧标记为未验证也允许发送；缺少检查结果或已过期时仍须重新检查。服务每小时重新检查；超过 24 小时未成功更新验证的域名不可发信。
5. 确认服务器的实际出口 IP，尤其是多网卡、NAT 和 Docker IPv6 场景。设置 `PUBLIC_IP` 不会自动修改操作系统路由。
6. 在“服务设置 → 配置状态”开启“投递开关”。必须已配置公网 IP，且至少一个发信域名在最近 24 小时内通过四项必需 DNS 检查；缺少 PTR/rDNS 不阻止开启。开关保存在数据库中，立即生效并在重启、升级后保留，不需要修改 `.env` 或重建容器。暂停时保留队列，正在投递的邮件仍可能完成；再次开启会继续发送队列中的邮件。

后台尚未保存开关时，兼容原有 `.env` 中的 `DELIVERY_ENABLED`；保存后以数据库中的开关为准。旧版本没有后台开关时，可设置 `DELIVERY_ENABLED=true` 并执行 `docker compose up -d --wait app`（受管安装使用 `reconfigure`）。本次后台开关新增 `0002_delivery_settings.sql` 数据库迁移，只添加开关表，不改动邮件、队列和域名数据。源码部署按上面的备份及更新流程执行，应用启动时运行迁移；受管安装因迁移指纹变化会拒绝自动更新和回退，需要单独安排迁移及恢复流程，不能跳过指纹校验。
7. 发送到自己控制的测试收件箱，检查邮件原始头中的 SPF、DKIM、DMARC 结果及投递位置，再用于日常邮件。

完成配置后刷新后台，“概览”和“服务设置”会出现“发送测试邮件”。填写已验证域名下的发件地址、自己的测试收件地址，并明确选择邮件语言。弹窗会跟踪队列状态；“对方已接收”仍需结合实际收件箱及原始头确认最终结果。配置缺失、关闭投递或域名验证失效时，此入口自动隐藏。

推荐为发信单独使用子域。当前 SPF 检查不支持与其他服务组合的策略，不能为了通过检查覆盖正在使用的域名策略。DMARC 初始 `p=none` 用于观察，确认所有合法来源后再调整策略。

域名托管在 Cloudflare 时，可按 [Cloudflare DNS 自动配置](cloudflare.zh-CN.md) 配置受限 API Token，再在域名页预览和创建缺失的记录。已有冲突记录不会被自动覆盖。

重试间隔依次为 1 分钟、5 分钟、15 分钟、1 小时、3 小时，之后 6 小时；最多 12 次或达到 48 小时。永久 SMTP 错误直接失败。人工重试只适用于失败记录。

## 可选 SMTP 提交

准备 `certs/fullchain.pem` 和 `certs/privkey.pem`，证书覆盖 `MAIL_HOSTNAME`，文件需可被容器 UID/GID `10001:10001` 读取。私钥不要设为所有人可读，使用专用组及 `0640` 权限。证书更新后重启应用加载证书。

如需公网提交，在 `.env` 加入 `SMTP_HOST_BIND=0.0.0.0`；默认只绑定本机。启动：

```bash
docker compose -f docker-compose.yml -f docker-compose.smtp.yml config --quiet
docker compose -f docker-compose.yml -f docker-compose.smtp.yml up -d --wait
```

客户端使用 `MAIL_HOSTNAME:587`、STARTTLS、用户名 `bunmail`，密码为后台创建的有效 API 密钥。证书信任验证必须开启，不提供无认证中继或明文 AUTH。管理页面显示的 SMTP 端口是容器内部监听端口 `2525`，客户端使用映射端口 `SMTP_PORT`（默认 `587`）。

## 密码恢复与数据

修改 `.env` 的 `BOOTSTRAP_PASSWORD` 不会覆盖已有密码。服务器管理员可通过标准输入重置密码并撤销全部浏览器会话：

```bash
python3 scripts/reset-password.py
```

脚本隐藏输入，密码不出现在命令参数或日志；API 密钥不受密码重置影响。受管部署加上 `--directory /srv/bunmail`。

数据库位于独立 Docker 卷；日常备份必须同时保存数据库和 `.env` 中的 DKIM 加密密钥。不要用删除卷处理启动问题。服务器迁移需要转移数据卷、配置、证书并重建部署状态；不能把目录改名后直接当成新安装。

## 浏览器报错排查

- `/api/v1/me` 在尚未登录或会话过期时返回 `401` 属于正常认证检查。登录接口的 `401 INVALID_LOGIN` 表示输入与数据库中已有的管理员凭据不符；如果后来已经登录，不需要为历史控制台日志重置密码。账号和初始密码仅在第一次初始化时生效。
- 旧版域名验证接口在同一域名 60 秒内检查超过 3 次时返回 `409 Please wait before checking DNS again`，这不是 DNS 记录冲突。停止重复点击并等待最多 60 秒。修复版使用 `429 RATE_LIMITED` 和 `Retry-After`，按钮显示倒计时，刷新页面后也会读取服务端剩余等待时间。实际 DNS 未全部通过时返回 `200`、`verified:false`，查看各项检查结果；DNS 查询异常则返回 `422`。
- `static.cloudflareinsights.com/beacon.min.js` 被 CSP 阻止的是 Cloudflare 统计脚本，不是本应用的登录或 DNS 检查脚本。本应用维持 `script-src 'self'`，不会为解决统计提示而允许任意第三方脚本。响应新增 `Cache-Control: no-store, no-transform`，禁止缓存并阻止代理修改页面。Cloudflare 自动统计注入遵循 `no-transform`；如果仍有提示，在该域名的 Web Analytics / Browser Insights 配置中关闭自动注入，并检查是否配置了额外的脚本注入规则或 Worker。

Cloudflare 说明：[Web Analytics 自动设置](https://developers.cloudflare.com/web-analytics/get-started/)。管理后台可以经 Cloudflare HTTPS 代理；如果邮件主机名也使用相同域名，必须单独确认它的 A/AAAA 直接指向真实出站服务器，而不是 Cloudflare 代理 IP。推荐管理后台和邮件主机使用不同的 DNS 名称。
