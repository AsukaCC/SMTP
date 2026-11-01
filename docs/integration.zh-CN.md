# 第三方网站接入

## 推荐架构

网站浏览器调用自己的业务后端，由业务后端调用 BunMail RS。API 密钥只保存在后端环境变量或密钥存储中，不放入 HTML、前端构建变量、URL、日志或 localStorage。网站后端调用不需要配置 CORS，也不需要管理后台 Cookie 或 `Origin`。

使用独立 API 密钥区分不同网站。密钥可发信、查询自己提交的邮件，不能修改域名、模板、投递开关或其他管理设置。撤销后立即失效。自己的业务接口仍需校验登录、业务权限和发送频率；不要直接公开一个可指定任意收件人的代理接口。

## 接口规范

- 基础路径：`https://你的邮件服务域名/api/v1`。
- OpenAPI 3.1：`GET /api/v1/openapi.json`，无需登录。后台“服务设置”可下载；支持将文件导入 Postman、Apifox 等客户端。只包含第三方公开 API，管理端点另见 [API 与 SMTP](api.zh-CN.md)。
- 请求：`Authorization: Bearer <API_KEY>`；POST 使用 `Content-Type: application/json`。
- 每个响应包含服务器生成的 `X-Request-ID`。排障时保留此编号、HTTP 状态和错误码，不记录密钥和邮件正文。
- `GET /auth/check`：验证密钥，返回 `{"authenticated":true,"scope":"send"}`，不发送邮件也不读取正文。
- `POST /emails/send`：每次一个收件人，支持 `from_name`、HTML/纯文本或模板变量。
- `GET /emails/{id}`：查询本密钥的邮件状态与尝试记录，其他密钥的记录返回 404。
- `GET /emails?page=0&status=queued&search=hello`：按本密钥范围分页查询，每页 50 条，`status` 和 `search` 可省略。

请求体最多 1 MiB；正文 HTML 与纯文本合计最多 512 KiB。字段类型、参数和示例以 OpenAPI 与 [详细说明](api.zh-CN.md) 为准。JSON 不接受未声明字段。JSON 语法、字段、UUID、查询参数、方法和体积错误统一返回 JSON；管理用 HTML 预览端点继续返回隔离的 HTML。

```json
{"error":{"code":"DOMAIN_NOT_VERIFIED","message":"Verify the sender domain DNS within the last 24 hours before sending"}}
```

| 状态 | 错误码 / 处理 |
| --- | --- |
| 200 | 查询成功，或相同幂等请求已存在，`created=false` |
| 202 | 已持久化入队，`created=true`；不表示已送达 |
| 400 | `BAD_REQUEST`：JSON 语法、UUID 或查询参数格式错误 |
| 401 | `UNAUTHORIZED`：缺失、无效或已撤销的密钥；带 `WWW-Authenticate` |
| 403 | `FORBIDDEN` / `ORIGIN_NOT_ALLOWED`：权限或来源不符 |
| 404 | `NOT_FOUND`：记录不存在或不属于此密钥 |
| 405 | `METHOD_NOT_ALLOWED`：HTTP 方法不支持 |
| 409 | `DELIVERY_PAUSED` / `IDEMPOTENCY_CONFLICT`：检查投递配置或幂等键使用 |
| 413 / 415 | `PAYLOAD_TOO_LARGE` / `UNSUPPORTED_MEDIA_TYPE`：请求过大或不是 JSON |
| 422 | `INVALID_INPUT` / `DOMAIN_NOT_VERIFIED`：修正字段或验证发件域名 |
| 429 | `RATE_LIMITED`：按响应 `Retry-After` 秒数等待；每密钥每分钟 60 次新发送尝试 |
| 500 | `INTERNAL_ERROR`：记录请求编号排查，不要换新幂等键盲目重发 |

429 的 `Retry-After` 为保守等待秒数（发信 60 秒；登录 300 秒；DNS 检查返回剩余冷却时间）。网络断开、超时、429、502、503、504 可按退避重试；建议等待 1、2、4 秒并最多重试 3 次，429 优先使用 `Retry-After`。所有重试必须使用原幂等键和相同请求内容，业务事件 ID 应在业务数据库中保存，不能每次 HTTP 重试都新建。

幂等键是 1-128 字节可打印 ASCII，按 API 密钥隔离，随邮件记录保留。它只保证不重复入队；SMTP 仍是至少一次语义。`sent` 仅表示收件服务器已接受，不保证进入收件箱。浏览器 CORS 错误、网络错误或超时不能证明服务端未接收请求，应使用原键重试或查询已知邮件 ID。

## 服务端调用

环境变量：`BUNMAIL_API_URL` 使用带 `/api/v1` 的完整基础路径，`BUNMAIL_API_KEY` 为独立发信密钥。下面的 cURL 不使用 `-v`，避免输出认证头：

```bash
curl --fail-with-body -sS \
  -H "Authorization: Bearer $BUNMAIL_API_KEY" \
  "$BUNMAIL_API_URL/auth/check"

curl --fail-with-body -sS \
  -H "Authorization: Bearer $BUNMAIL_API_KEY" \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: order-20261101-001' \
  --data '{"from":"hello@example.com","from_name":"订单通知","to":"recipient@example.net","subject":"订单已确认","text":"感谢您的购买。"}' \
  "$BUNMAIL_API_URL/emails/send"
```

Node.js 22+ 无额外依赖的客户端位于 [`examples/client.mjs`](../examples/client.mjs)：

```javascript
import {BunMailClient} from './examples/client.mjs';
const mail = new BunMailClient({
  baseUrl: process.env.BUNMAIL_API_URL,
  apiKey: process.env.BUNMAIL_API_KEY,
  timeoutMs: 10000
});
await mail.check();
const {data, requestId} = await mail.send({
  from: 'hello@example.com', from_name: '通知中心',
  to: 'recipient@example.net', subject: '订单已确认', text: '感谢您的购买。'
}, 'order-20261101-001');
const status = await mail.get(data.id);
// 只记录 data.id、requestId 和状态，不记录认证头或正文。
```

客户端返回 `{data,status,requestId}`；失败抛出 `BunMailError`，含 `status`、`code`、`requestId`、`retryAfter`。默认 10 秒超时，禁止自动重定向，避免密钥被转发到错误站点；客户端不自动重发，由业务系统根据上述规则控制重试。

## 可运行的网站示例

[`examples/website`](../examples/website) 是本机网站：浏览器不接触密钥，Node 后端固定发件人与收件人，调用 BunMail API 并查询结果。不会提供任意收件人的开放代理。

先将该目录的 `.env.example` 作为配置参考，在项目根目录创建已被 Git 忽略的 `.env.integration.local`，填入 `BUNMAIL_API_URL`、`BUNMAIL_API_KEY`、`MAIL_FROM`、`MAIL_TO`、`PORT`。不要覆盖邮件服务原有 `.env`。

```bash
node --env-file=.env.integration.local examples/website/server.mjs
```

打开命令输出的地址，默认 `http://127.0.0.1:4000`。相同表单内容重复提交会复用幂等键，可观察 `created` 从 true 变为 false；修改内容会生成新事件。刷新状态查看真实队列结果。示例只绑定本机，内存中的事件键不用于生产持久化；公网业务上线前应接入自身登录、权限、频率限制以及持久化业务事件 ID。

无配置也可运行 `node examples/website/server.mjs` 查看页面，发送按钮保持禁用。

## 可选浏览器跨域

仅受信任的操作者工具适合手动持有密钥直接调用。普通公开网站仍采用后端代理。CORS 不是身份认证，非浏览器客户端可不发送或伪造 Origin；它也不会把密钥限制为只能在某个网站使用。

在邮件服务器 `.env` 配置精确来源，默认留空关闭：

```dotenv
API_ALLOWED_ORIGINS=https://console.example.com,https://tools.example.com
```

不支持 `*`、通配子域、路径、查询参数和 `null`；生产要求 HTTPS，开发模式可用 HTTP。协议、主机、端口共同决定来源。修改后源码部署执行 `docker compose up -d --wait app`，受管部署使用 `reconfigure`。

仅 `/auth/check`、`/emails`、`/emails/{id}`、`/emails/send` 开放白名单 CORS。允许 GET/POST，允许 `Authorization`、`Content-Type`、`Idempotency-Key` 请求头，暴露 `X-Request-ID`、`Retry-After`。预检缓存 600 秒，不允许携带 Cookie，调用必须设置 `credentials:'omit'` 并显式提供 Bearer 密钥。管理端点不开放跨域。

如果第三方网站设置了 CSP，其 `connect-src` 也需要允许邮件 API 的来源；浏览器控制台可区分 CSP 拦截、CORS 预检失败与 API 返回的 JSON 错误。

```javascript
// token 由受信任操作者临时提供，不写入网页源码或浏览器存储。
async function checkConnection(apiBase, token) {
  const response = await fetch(apiBase + '/auth/check', {
    credentials: 'omit', headers: {Authorization: 'Bearer ' + token}
  });
  const result = await response.json();
  if (!response.ok) throw new Error(result.error.code);
  return result;
}
```

## 集成测试

测试启动独立 API、两个不同浏览器来源和真实 Node 网站后端，覆盖认证、JSON 错误、限流头、幂等冲突、查询、预检、来源拒绝、非法请求头、管理接口隔离和网页手机布局。

```bash
PLAYWRIGHT_MODULE=/绝对路径/node_modules/playwright \
BUNMAIL_THIRD_PARTY_TEST=true python3 scripts/test-integration.py
```

使用隔离数据库与模拟域名检查，不启动公网投递 Worker；测试邮件只入队，不代表公网投递成功。也可同时设置 `BUNMAIL_BROWSER_TEST=true` 验证原有中英文后台流程。
