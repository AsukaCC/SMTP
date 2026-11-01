# API 与 SMTP

基础路径为 `/api/v1`。后台登录创建会话 Cookie；管理写入需要与 `APP_URL` 一致的 `Origin`。发信 API 使用 `Authorization: Bearer <API_KEY>`，密钥仅创建时显示一次，不可读取管理数据。

第三方网站的服务端示例、可选 CORS、幂等重试规则与自动测试见 [第三方网站接入](integration.zh-CN.md)。公开 API 的 OpenAPI 3.1 文件位于 `GET /api/v1/openapi.json`，可直接导入客户端；`GET /api/v1/auth/check` 可验证凭据而不发送邮件。所有 API 响应包含服务器生成的 `X-Request-ID`。

## 发信

```http
POST /api/v1/emails/send
Authorization: Bearer <API_KEY>
Content-Type: application/json
Idempotency-Key: order-20261031-001

{
  "from": "hello@example.com",
  "from_name": "通知中心",
  "to": "recipient@example.net",
  "subject": "测试邮件",
  "html": "<p>你好</p>",
  "text": "你好"
}
```

首次成功入队返回 HTTP `202`：`{"id":"UUID","created":true}`。相同 API 密钥、相同幂等键和相同请求重试返回同一 ID、HTTP `200`、`created:false`。同键不同内容返回 `409`。更换 API 密钥即改变幂等范围。

`from` 和 `to` 各接受一个裸邮箱地址，不支持地址数组或 SMTPUTF8；国际化域名需先转换成 punycode。可选 `from_name` 为发件人显示名称，支持中文，最多 256 字节，不能包含控制字符；省略或空值表示只显示地址。邮件头编码和转义由 Lettre 处理，SMTP 信封仍使用裸地址。发送记录中的 `from_address` 保存完整 From 邮箱表示（有名称时为 `名称 <邮箱>`）；旧记录兼容，无须增加数据库字段。未传名称时保持原有幂等请求摘要格式。

主题最长 998 字节，正文最多 512 KiB。至少提供 `html` 或 `text` 之一。发件域名的 SPF、DKIM、DMARC、A/AAAA 必须已验证且发送开关启用。PTR/rDNS 为告警项，不阻止发送；`GET /config` 的 `rdns_warning` 提示最近保存的检查中存在 PTR 未通过的情况。每个 API 密钥每分钟最多 60 次新发送尝试。

## 模板

在后台创建模板后，使用其 UUID：

```json
{
  "from": "hello@example.com",
  "to": "recipient@example.net",
  "template_id": "模板 UUID",
  "variables": {"name": "张三"}
}
```

模板只支持 `{{ name }}` 形式的变量，变量名由字母、数字和下划线组成。HTML 变量自动转义；缺少变量会报错。不支持循环、条件、函数、过滤器或原始 HTML 绕过。模板内容优先于请求中的主题和正文。

## 初始化与测试

`POST /templates/initialize` 仅允许管理员会话，补齐 8 个中英文初始模板并返回 `{"inserted":数量}`，不覆盖同 ID 已有内容。新实例会自动初始化，删除后的模板只会在手动初始化时补回。

`POST /templates/preview` 使用 `application/x-www-form-urlencoded`：提供 `template_id` 或 `subject`、`html`、`text`，以及 JSON 字符串形式的 `variables`。返回隔离的 HTML 文档，仅供后台 iframe 预览；脚本、外部资源和表单提交均由 CSP 禁止，不提供远程图片预览。预览不入队，不要求启用投递。

`GET /config` 中的 `ready_to_test` 同时检查公网 IP、投递开关和有效发信域名。后台据此显示测试入口。管理员可通过 `PUT /config/delivery` 提交 `{"enabled":true}` 或 `{"enabled":false}` 切换投递，需要管理员会话和匹配的 `Origin`，发信 API 密钥无权修改。开启要求公网 IP 和最近 24 小时内有效的发信域名，PTR/rDNS 仅提示。开关保存在数据库并覆盖环境变量的初始值；暂停保留队列，正在投递的邮件仍可能完成。发送测试仅允许管理员会话，并需要 `Origin`：

```http
POST /api/v1/emails/test
Content-Type: application/json
Origin: https://mail-admin.example.com
Idempotency-Key: delivery-test-001

{"from":"test@example.com","from_name":"测试发件人","to":"your-mailbox@example.net","language":"zh-CN"}
```

`language` 为 `zh-CN` 或 `en`，独立于后台语言。测试邮件使用内置投递测试正文，经相同的域名验证、限流、幂等和 DKIM 队列投递流程；返回格式与发信 API 相同。未配置时返回 `409`，发件域名未验证或验证过期时返回 `422`，API 密钥访问此管理端点返回 `403`。

## 后台端点

以下仅允许管理员会话访问：

| 端点 | 方法与用途 |
| --- | --- |
| `/login`、`/logout` | POST 登录、退出 |
| `/me`、`/config`、`/stats` | GET 当前账号、配置、统计 |
| `/domains` | GET 列表、POST 添加 `{name}` |
| `/domains/{id}/verify` | POST 验证 DNS |
| `/domains/{id}/cloudflare/preview` | POST 预览 Cloudflare DNS 变更，无外部写入 |
| `/domains/{id}/cloudflare/apply` | POST `{fingerprint}`，核对预览后创建缺失记录 |
| `/domains/{id}` | DELETE 删除无邮件引用的域名 |
| `/templates` | GET 列表（含 UUID）、POST 新建 |
| `/templates/{id}` | PUT 更新、DELETE 删除 |
| `/api-keys` | GET 列表、POST 创建 `{name}` |
| `/api-keys/{id}` | DELETE 撤销 |
| `/emails?page=0&status=queued&search=hello` | GET 记录，页码从 0 起，每页 50 条；状态与搜索可选 |
| `/emails/{id}` | GET 邮件及投递尝试详情 |
| `/emails/{id}/retry` | POST 重试失败邮件 |
| `/emails/{id}/cancel` | POST 取消尚未发送的队列邮件 |

`GET /health` 无需登录，仅返回服务版本及数据库健康状态。错误格式统一为 `{"error":{"code":"…","message":"…"}}`，包含框架级 JSON、UUID、查询参数、体积和方法错误；管理用 HTML 预览端点保持隔离的 HTML 错误页面。429 包含 `Retry-After`；401 包含 Bearer 认证挑战。发信端点区分 `DELIVERY_PAUSED`、`DOMAIN_NOT_VERIFIED`、`IDEMPOTENCY_CONFLICT`，客户端应按错误码处理，不依赖英文消息文本。

邮件列表和单封详情也允许发信 API 密钥读取，但仅返回该密钥提交的记录。`search` 对主题、完整发件人、收件人做不区分大小写的字面子串搜索，最多 200 个字符；`%`、`_` 不作为通配符。支持与 `status` 同时筛选，状态仅限 `queued`、`sending`、`sent`、`failed`、`cancelled`。列表按创建时间和 ID 倒序稳定分页，返回 `has_more` 与当前 `page`。

Cloudflare 凭据仅配置在服务器 `.env`，不会由 API 返回。`GET /config` 的 `cloudflare_configured` 只表示是否配置 Token，不代表权限已验证。预览返回 `records`、`fingerprint` 和 `can_apply`；记录状态为 `create`、`unchanged`、`conflict`、`unavailable`。应用变更时重新读取 Cloudflare 并校验 fingerprint，冲突或过期预览返回 `409`。执行结果返回 `complete` 和各记录的 `created`、`unchanged`、`failed`、`skipped`；部分失败也可能返回 `200`，必须检查 `complete`。应用后清除该域名的旧验证状态，需等 DNS 生效后重新检查。

SMTP 提交共用相同校验与队列，必须 STARTTLS 后认证。`From` 头只能包含一个发件人，其邮箱必须与 `MAIL FROM` 一致；显示名称会保留。一次只能接受一个 `RCPT TO`。SMTP `250 Queued as …` 表示已持久化入队，不表示最终送达。SMTP 本身不支持此 HTTP API 的幂等键。
