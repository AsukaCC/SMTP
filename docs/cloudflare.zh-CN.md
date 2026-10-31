# Cloudflare DNS 自动配置

后台“发信域名 → Cloudflare 配置”可预览并创建 SPF、DKIM、DMARC、邮件主机的 A 或 AAAA。PTR/rDNS 不通过此功能配置，需联系公网 IP 所属服务器供应商；本应用不再要求 PTR 通过才能发送。

## 服务器配置

1. 在 [Cloudflare API Tokens](https://dash.cloudflare.com/profile/api-tokens) 创建 API Token，授予 **Zone / Zone / Read** 和 **Zone / DNS / Edit**，资源范围只选择需要配置的域名区域。不要使用 Global API Key。
2. 在服务器原有 `.env` 中添加 `CLOUDFLARE_API_TOKEN=你的Token`。Token 不填入网页、不提交仓库、不作为镜像构建参数，也不会返回给浏览器。
3. 确认 `PUBLIC_IP` 为实际出站 IP，`MAIL_HOSTNAME` 为邮件主机名。源码部署重新创建应用容器；受管部署使用 `reconfigure`，见部署文档。
4. 在后台添加发信域名，打开“Cloudflare 配置”。应用根据 DNS 名称选择 Token 可访问的最长匹配区域；发信域名与邮件主机分属不同区域时，两者都需要授权。

## 预览与执行

预览不修改 Cloudflare。确认记录名称和值后点击“创建缺失记录”。系统会重新读取记录，预览过期或存在冲突时拒绝写入；同一实例同一时间只执行一个配置操作。

- 已有完全一致的记录保持不变，重复执行不重复添加。TXT 分段由现有 DNS 文本解析库处理。
- 已有不同的 SPF、DKIM、DMARC、A/AAAA 或冲突的 CNAME/NS 时显示冲突。应用不覆盖、不删除这些记录，也不覆盖其他用途的 TXT。
- 邮件主机的新 A/AAAA 固定为 DNS only（`proxied=false`）。已有代理记录会显示冲突，避免改变正在使用的管理后台或网站代理配置。推荐管理后台与邮件主机使用不同名称。
- 每条创建结果单独显示。若发生网络或权限错误，保留已创建的记录，停止后续创建。重新预览后只补缺失记录；不会通过删除已创建记录实现回滚。
- Cloudflare 不提供跨多个区域的原子变更。执行期间应避免其他客户端同时编辑相同记录；本应用在预览后以及每次创建前都会重新检查，但无法锁住 Cloudflare 控制台中的操作。
- 创建记录不等于公共 DNS 已生效。操作后清除该域名旧的验证状态，待传播完成后点击“检查 DNS”；只有必要的四项验证通过且投递开关开启才能发信。

无可用区域时检查 Token 资源范围、域名是否已在 Cloudflare 激活。请求失败时检查 Zone Read / DNS Edit 权限、Token 有效期及服务器是否能访问 `api.cloudflare.com`。单个查询最多读取 20 页、每页 50 条，超过限制会提示缩小范围，不会以不完整数据生成写入计划。

API 实现依据：[列出 DNS 记录](https://developers.cloudflare.com/api/resources/dns/subresources/records/methods/list/)、[创建 DNS 记录](https://developers.cloudflare.com/api/resources/dns/subresources/records/methods/create/)。自动化测试使用本机模拟 Cloudflare API，不代表已在真实账号中创建记录。
