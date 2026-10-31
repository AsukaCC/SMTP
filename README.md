# BunMail RS

个人自用的 Rust 邮件发送服务，参考 [BunMail](https://github.com/mohamedboukari/bunmail) 的域名管理、API 发信和控制台流程重新实现。参考基线为 BunMail `0.11.0`，提交 `14fac2560ec5708e01f6e41d88d7fc5a37f84644`；本项目版本独立从 `0.1.0` 开始，不是上游的 Rust 分支，也不保证 API 或数据库兼容。

## 功能

- 简体中文默认界面，支持英文切换并记住偏好。
- 单管理员登录；独立 API 密钥生成、撤销，密钥只允许发信。
- 发信域名、RSA 2048 DKIM 密钥、SPF / DKIM / DMARC / PTR / A 或 AAAA 检查。
- rDNS 为可选告警，缺失时仍可发信；支持发件人显示名称，支持 Cloudflare DNS 预览及创建缺失记录。
- HTTP API 发信、经过 STARTTLS 与认证的 SMTP 提交、纯文本及 HTML 模板。
- 内置欢迎、验证码、事务通知、投递测试四类模板，各有简体中文和英文版本；支持模板筛选、示例变量、桌面/手机预览。
- PostgreSQL 持久队列、发送记录、指数式间隔重试、手动重试和取消排队邮件。
- Rust 直接连接收件方 MX；无需 SES、Resend、Redis 或 Postfix。
- Docker 部署，以及同目录防重复安装、更新前备份、健康检查失败恢复原版本、手动回退。
- 后台查看版本、检查本项目 GitHub 正式发布、复制受管更新与回退命令；在服务器执行更新。

服务采用 Axum、Tokio、SQLx 和 PostgreSQL；Lettre 处理 MIME、DKIM 和出站 SMTP，Mailin 处理提交协议状态，Hickory 负责 DNS。前端静态资源编译进同一个 Rust 二进制文件。

## 本地运行

需要 Docker Compose v2 和 Python 3.11+：

```bash
python3 scripts/setup.py --development
docker compose up -d --build --wait
```

打开 <http://localhost:3000>。管理员邮箱及首次密码位于本地 `.env` 的 `OWNER_EMAIL` / `BOOTSTRAP_PASSWORD`；脚本不输出凭据，不覆盖现有 `.env`。默认关闭对外发信。已有配置时跳过 `setup.py`。

新实例首次创建管理员时自动初始化 8 个邮件模板。已有实例在“邮件模板”点击“添加初始模板”；重复执行只补齐缺少的预设，不覆盖编辑内容。预览中的示例验证码只用于展示，正式验证码必须由业务系统生成和校验。后台语言切换不会更改模板及邮件正文语言。

“发送测试邮件”入口只有在配置公网 IP、启用投递、并存在最近 24 小时内通过全部 DNS 检查的发信域名后才显示。可填写发件地址、收件地址及邮件语言，提交后显示实际队列状态和错误；后端仍会校验发件域名。未配置时不会显示测试入口，也不能通过直接请求绕过限制。

生产部署须配置固定公网 IP、出站 TCP 25、域名 DNS，以及管理页面 HTTPS。建议在服务器供应商处配置 PTR；缺失或不匹配时本应用只提示，不阻止发送，但收件方仍可能拒收或归入垃圾邮件。**rDNS 是接收方验证服务器身份的一环，并不是发信协议，也不保证进入收件箱。**

- [服务器与 Docker 部署](docs/deployment.zh-CN.md)
- [API 与 SMTP 使用](docs/api.zh-CN.md)
- [Cloudflare DNS 自动配置](docs/cloudflare.zh-CN.md)
- [版本打包、更新及回退](docs/releases.zh-CN.md)
- [维护约定](AGENTS.md)

## 开发验证

需要 Rust 1.91.1、Node.js（仅检查前端语法）、Docker、Python 3.11+、OpenSSL（SMTP 集成测试生成临时证书）。

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
python3 -m unittest discover -s tests -p 'test_*.py'
node --check static/app.js
docker compose -f docker-compose.yml -f docker-compose.dev.yml up -d db
python3 scripts/test-integration.py
```

集成测试会在本地 PostgreSQL 中新建随机名称的独立数据库，结束后删除该测试数据库；测试账号须有建库权限。测试使用隔离数据模拟已验证域名，验证本机 SMTP 提交，不启动对外投递 Worker，也不等同于真实 DNS 或公网投递测试。`cargo run` 使用 `.env` 的本地数据库连接；避免同时启动两个开发进程争用 HTTP 端口。

浏览器验证：安装 Playwright 及 Chromium 后运行 `node scripts/test-ui.cjs`（或通过 `PLAYWRIGHT_MODULE` 指定现有安装路径）。需要本地开发实例和关闭的投递开关；会创建临时域名、模板及密钥，完成后删除域名和模板、撤销密钥，截图保存在忽略提交的 `artifacts/`。

配置完成状态的浏览器测试：`BUNMAIL_BROWSER_TEST=true python3 scripts/test-integration.py`，可同样指定 `PLAYWRIGHT_MODULE`。测试启动独立临时 HTTP 服务，使用隔离的已验证域名数据，验证测试按钮、真实 API 入队、模板预览及 CSP 隔离；不启动公网投递 Worker，不代表邮件已进入真实收件箱。

发布流程验证：先生成 `0.1.0` 包，再运行 `python3 scripts/test-release.py /tmp/bunmail-packages/0.1.0`。它创建一次性 Docker 项目和合成版本镜像，验证真实备份恢复、更新、回退和失败恢复，最终只清理自己创建的测试容器、卷与镜像。

## 当前边界

- 每封邮件仅一个收件人、正文总计最多 512 KiB；不支持附件、抄送、密送、营销活动、自动化、计费及收件箱。
- 不接收退信或 DSN，不处理投诉、打开率或点击跟踪；`sent` 只表示对方 SMTP 接受，不代表最终入箱。
- SMTP 提交会提取主题和正文，重新构建邮件、Message-ID 和 DKIM；不保留任意自定义头部或第三方签名。
- SPF 校验只接受界面提供的显式单 IP 策略，不支持 `include`、`redirect`、CIDR 或多个 IP。配置的 IP 协议族决定出站 IPv4 / IPv6；实际出口地址由服务器路由或 NAT 决定，须与 `PUBLIC_IP` 相符。
- 出站采用机会性 STARTTLS；支持 TLS 的服务器必须通过证书验证，未提供 STARTTLS 的服务器可能使用明文传输。尚未实现 MTA-STS、DANE、投递 Webhook。
- 队列为至少一次投递语义：对方已接收但连接断开时，重试可能重复。API 幂等键只防止重复入队。
- 本版适合个人低流量使用；尚未在用户的公网邮件服务器上完成投递和信誉验证。

## 许可

MIT。上游参考来源及前端图标许可见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
