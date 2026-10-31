# API 与 SMTP

基础路径为 `/api/v1`。后台登录创建会话 Cookie；管理写入需要与 `APP_URL` 一致的 `Origin`。发信 API 使用 `Authorization: Bearer <API_KEY>`，密钥仅创建时显示一次，不可读取管理数据。

## 发信

```http
POST /api/v1/emails/send
Authorization: Bearer <API_KEY>
Content-Type: application/json
Idempotency-Key: order-20261031-001

{
  "from": "hello@example.com",
  "to": "recipient@example.net",
  "subject": "测试邮件",
  "html": "<p>你好</p>",
  "text": "你好"
}
```

首次成功入队返回 HTTP `202`：`{"id":"UUID","created":true}`。相同 API 密钥、相同幂等键和相同请求重试返回同一 ID、HTTP `200`、`created:false`。同键不同内容返回 `409`。更换 API 密钥即改变幂等范围。

只接受一个裸邮箱地址，不支持显示名称、地址数组或 SMTPUTF8；国际化域名需先转换成 punycode。主题最长 998 字节，正文最多 512 KiB。至少提供 `html` 或 `text` 之一。发件域名必须已验证且发送开关启用。每个 API 密钥每分钟最多 60 次新发送尝试。

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

`GET /config` 中的 `ready_to_test` 同时检查公网 IP、投递开关和有效发信域名。后台据此显示测试入口。发送测试仅允许管理员会话，并需要 `Origin`：

```http
POST /api/v1/emails/test
Content-Type: application/json
Origin: https://mail-admin.example.com
Idempotency-Key: delivery-test-001

{"from":"test@example.com","to":"your-mailbox@example.net","language":"zh-CN"}
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
| `/domains/{id}` | DELETE 删除无邮件引用的域名 |
| `/templates` | GET 列表（含 UUID）、POST 新建 |
| `/templates/{id}` | PUT 更新、DELETE 删除 |
| `/api-keys` | GET 列表、POST 创建 `{name}` |
| `/api-keys/{id}` | DELETE 撤销 |
| `/emails?page=0&status=queued` | GET 记录，页码从 0 起 |
| `/emails/{id}` | GET 邮件及投递尝试详情 |
| `/emails/{id}/retry` | POST 重试失败邮件 |
| `/emails/{id}/cancel` | POST 取消尚未发送的队列邮件 |

`GET /health` 无需登录，仅返回服务版本及数据库健康状态。常规错误格式为 `{"error":{"code":"…","message":"…"}}`，框架级 JSON 或路径解析错误可能为文本。

SMTP 提交共用相同校验与队列，必须 STARTTLS 后认证。`From` 头必须与 `MAIL FROM` 一致；一次只能接受一个 `RCPT TO`。SMTP `250 Queued as …` 表示已持久化入队，不表示最终送达。SMTP 本身不支持此 HTTP API 的幂等键。
