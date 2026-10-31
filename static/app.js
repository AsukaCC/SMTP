"use strict";
const ZH = {
  "Dashboard":"概览","Send email":"发送邮件","Emails":"发送记录","Domains":"发信域名","Templates":"邮件模板","API keys":"API 密钥","Settings":"服务设置","Sign in":"登录","Sign out":"退出登录","Email":"邮箱","Password":"密码","Administrator":"管理员","Personal instance":"个人实例","Refresh":"刷新","New email":"写邮件","Queued":"待发送","Sending":"发送中","Sent":"对方已接收","Failed":"发送失败","Cancelled":"已取消","Pending":"待验证","Verified":"已验证","Delivery paused":"投递已暂停","Delivery enabled":"投递已启用","Recent emails":"最近邮件","View all":"查看全部","No emails yet":"暂无邮件记录","No domains yet":"尚未添加发信域名","No templates yet":"尚未创建模板","No API keys yet":"尚未创建 API 密钥","Subject":"主题","Recipient":"收件人","Status":"状态","Created":"创建时间","Attempts":"尝试次数","Actions":"操作","From":"发件地址","To":"收件地址","HTML body":"HTML 正文","Text body":"纯文本正文","Template":"模板","No template":"不使用模板","Variables (JSON)":"模板变量（JSON）","Send":"发送","Add domain":"添加域名","Domain name":"域名","Cancel":"取消","Save":"保存","Delete":"删除","Delete this record?":"确定删除此记录？","Check DNS":"检查 DNS","DNS records":"DNS 记录","Type":"类型","Name":"名称","Value":"值","Copy":"复制","Copied":"已复制","New template":"创建模板","Edit template":"编辑模板","Edit":"编辑","New API key":"创建 API 密钥","Key name":"密钥名称","Create":"创建","Prefix":"前缀","Active":"有效","Revoked":"已撤销","Revoke":"撤销","Revoke this API key?":"确定撤销此 API 密钥？","Key created":"密钥已创建","This key is shown only once.":"此密钥仅显示一次。","Close":"关闭","Message queued":"邮件已加入队列","Saved":"已保存","DNS check completed":"DNS 检查完成","All statuses":"全部状态","Previous page":"上一页","Next page":"下一页","Page":"页","Email details":"邮件详情","Message ID":"邮件标识","Accepted at":"对方接收时间","Next attempt":"下次尝试","Last error":"最近错误","Retry":"重新发送","Cancel queued email?":"确定取消这封待发送邮件？","Delivery attempts":"投递尝试","No delivery attempts":"尚无投递尝试","Message":"邮件正文","Service version":"服务版本","Mail hostname":"邮件主机名","Public IP":"公网 IP","Not configured":"未配置","SMTP submission":"SMTP 提交","Enabled":"已启用","Disabled":"未启用","Transport":"传输方式","Direct to recipient MX":"直连收件方 MX","STARTTLS required":"必须使用 STARTTLS","SMTP username":"SMTP 用户名","SMTP password":"SMTP 密码","An active API key":"有效的 API 密钥","HTTP API":"HTTP API","API endpoint":"API 地址","Authentication":"认证方式","Bearer API key":"Bearer API 密钥","Loading":"正在加载","Request failed":"请求失败","Connection failed":"无法连接服务","Invalid email or password":"邮箱或密码错误","Please sign in or provide a valid API key":"请登录或提供有效的 API 密钥","Administrator access required":"此操作需要管理员权限","The record already exists":"记录已存在","The record is still in use":"记录仍在使用，不能删除","The operation could not be completed":"操作未能完成，请查看服务日志","Record not found":"记录不存在","Invalid domain name":"域名格式不正确","Invalid email address":"邮箱格式不正确","A template variable is missing or invalid":"模板变量缺失或格式不正确","Delivery is paused. Configure DNS and enable DELIVERY_ENABLED":"投递已暂停，请完成 DNS 配置后启用 DELIVERY_ENABLED","Verify the sender domain DNS within the last 24 hours before sending":"发送前请先完成发信域名 DNS 验证，验证有效期为 24 小时","Delivery is paused":"投递已暂停","Only failed messages can be retried":"只有发送失败的邮件可以重试","Only queued messages can be cancelled":"只有待发送的邮件可以取消","Please wait before checking DNS again":"请稍后再检查 DNS","DNS lookup failed. Try again later; saved verification was not changed":"DNS 查询失败，请稍后重试；已有验证记录未更改","Invalid JSON":"JSON 格式不正确","SMTP port":"SMTP 端口","Server identity":"服务器身份","Domain checks":"域名检查","Version & deployment":"版本与部署","Please configure a public IP and DNS before enabling delivery.":"启用投递前请先配置公网 IP 和 DNS。","Accepted by recipient SMTP server; inbox placement is not confirmed.":"收件方 SMTP 服务器已接受邮件，不代表已进入收件箱。","Only one recipient per message":"每封邮件一个收件人","Too many attempts. Try again in five minutes":"尝试过于频繁，请在五分钟后重试","Sending rate exceeded":"发送频率超过限制","Name must contain 1 to 100 bytes":"名称长度须为 1 至 100 字节","Subject is required, at most 998 bytes, without newlines":"请填写主题，最多 998 字节，不能包含换行","Message body is required and must be at most 512 KiB":"请填写邮件正文，正文总大小不能超过 512 KiB","Changes not saved. Leave this page?":"更改尚未保存，确定离开？","Retry scheduled":"已安排重试","Use valid JSON object for variables":"请使用 JSON 对象填写模板变量","Sender domain is unavailable":"发信域名暂时不可用","Sender DNS verification is missing or expired":"发信域名 DNS 验证缺失或已过期","Could not build or sign the message":"无法构建邮件或完成 DKIM 签名","Recipient domain declares null MX":"收件域名明确表示不接收邮件（null MX）","Recipient domain does not exist":"收件域名不存在","Temporary MX lookup failure":"MX 查询暂时失败","No reachable public MX address":"没有可连接的公网 MX 地址","Delivery attempt timed out":"投递尝试超时","Recipient SMTP server accepted the message":"收件方 SMTP 服务器已接受邮件","Request origin does not match APP_URL":"当前访问地址与 APP_URL 配置不匹配"
};
const $ = (s, root = document) => root.querySelector(s);
Object.assign(ZH,{
  "Sender name":"发件人名称","Sender name must be at most 256 bytes without control characters":"发件人名称最多 256 字节，不能包含控制字符","rDNS is missing, mismatched or unavailable. Sending is allowed, but recipients may reject the message or mark it as spam.":"rDNS 缺失、不匹配或暂时无法查询。仍可发送，但收件方可能拒收或将邮件归入垃圾邮件。","PTR / rDNS (optional)":"PTR / rDNS（可选）","Cloudflare DNS":"Cloudflare DNS","Configure Cloudflare":"Cloudflare 配置","Preview DNS changes":"预览 DNS 变更","Create missing records":"创建缺失记录","Existing records":"现有记录","Create record":"待创建","Already configured":"已配置","DNS conflict":"记录冲突","Zone unavailable":"无可用域名区域","DNS records created. Check DNS after propagation.":"DNS 记录已创建，生效后请检查 DNS。","Some DNS changes failed. Preview again before retrying.":"部分记录未能创建，请重新预览后重试。","Cloudflare does not configure PTR. Set it with your server provider.":"Cloudflare 不配置 PTR，需在服务器供应商处设置。","Configure CLOUDFLARE_API_TOKEN on the server first":"请先在服务器 .env 中配置 CLOUDFLARE_API_TOKEN","Configure PUBLIC_IP before creating DNS records":"创建 DNS 记录前请先配置 PUBLIC_IP","Cloudflare request failed. Check token permissions and connectivity":"Cloudflare 请求失败，请检查 Token 权限和服务器网络","Too many Cloudflare records. Narrow the token scope":"Cloudflare 记录过多，请缩小 Token 的域名授权范围","A Cloudflare change is already in progress":"已有 Cloudflare 配置操作正在执行","DNS records changed or contain conflicts. Preview again":"DNS 记录已变化或存在冲突，请重新预览","Resolve conflicts or zone access before applying":"请先处理冲突记录或域名区域权限，再重新预览","DNS created":"已创建","DNS unchanged":"无需修改","DNS failed":"创建失败","DNS skipped":"未执行"
});
let cloudflarePlan=null, cloudflareBusy=false, cloudflareRequest=0;
function rdnsNotice(){return `<p class="notice warning">${icon("triangle-alert")}<span>${tr("rDNS is missing, mismatched or unavailable. Sending is allowed, but recipients may reject the message or mark it as spam.")}</span></p>`;}
function domainView({domain:d,records}){
  return `<section class="domain"><div class="domain-head"><div class="row"><h2>${esc(d.name)}</h2><span class="badge ${d.verified?"verified":"pending"}">${tr(d.verified?"Verified":"Pending")}</span></div><div class="row"><button class="btn" data-action="cloudflare" data-id="${d.id}">${icon("cloud-cog")}${tr("Configure Cloudflare")}</button><button class="btn" data-action="verify" data-id="${d.id}">${icon("refresh-cw")}${tr("Check DNS")}</button>${button("delete-domain","trash-2","Delete",`data-id="${d.id}"`)}</div></div><div class="checks">${[["spf","SPF"],["dkim","DKIM"],["dmarc","DMARC"],["ptr","PTR / rDNS (optional)"],["forward","A / AAAA"]].map(([key,label])=>`<span class="check ${d.checks[key]?"ok":key==="ptr"&&d.checked_at?"warn":""}">${icon(d.checks[key]?"circle-check":key==="ptr"&&d.checked_at?"triangle-alert":"circle-dashed")}${tr(label)}</span>`).join("")}<span class="muted">${date(d.checked_at)}</span></div>${d.checked_at&&!d.checks.ptr?rdnsNotice():""}<details><summary>${tr("DNS records")}</summary><div class="table-wrap"><table class="dns-table"><thead><tr><th>${tr("Type")}</th><th>${tr("Name")}</th><th>${tr("Value")}</th><th></th></tr></thead><tbody>${records.map((r,index)=>`<tr><td>${esc(r.kind)}</td><td><code>${esc(r.name)}</code></td><td><code>${esc(r.value)}</code></td><td>${button("copy-dns","copy","Copy",`data-id="${d.id}" data-index="${index}"`)}</td></tr>`).join("")}</tbody></table></div></details></section>`;
}
function openCloudflare(id){
  if(cloudflareBusy){toast("A Cloudflare change is already in progress");return;}
  cloudflarePlan=null;cloudflareRequest++;
  modal("Cloudflare DNS",`<div id="cloudflare-content">${config.cloudflare_configured?"":`<p class="error">${tr("Configure CLOUDFLARE_API_TOKEN on the server first")}</p>`}</div><p class="muted section">${tr("Cloudflare does not configure PTR. Set it with your server provider.")}</p><div class="form-foot"><button class="btn" data-action="cloudflare-preview" data-id="${id}" ${!config.cloudflare_configured?"disabled":""}>${icon("refresh-cw")}${tr("Preview DNS changes")}</button><button class="btn primary" data-action="cloudflare-apply" data-id="${id}" disabled>${icon("cloud-upload")}${tr("Create missing records")}</button></div>`,true);
  $("#dialog").dataset.cloudflareId=id;
  if(config.cloudflare_configured)void previewCloudflare(id);
}
function cloudflareButtons(){
  const preview=$('[data-action="cloudflare-preview"]'),apply=$('[data-action="cloudflare-apply"]');
  if(preview)preview.disabled=cloudflareBusy||!config.cloudflare_configured;
  if(apply)apply.disabled=cloudflareBusy||!cloudflarePlan?.can_apply;
}
function cloudflareRecords(plan){
  const labels={create:"Create record",unchanged:"Already configured",conflict:"DNS conflict",unavailable:"Zone unavailable"};
  return `${plan.records.map(r=>`<section class="dns-change"><div class="row"><strong>${esc(r.kind)}</strong><code>${esc(r.name)}</code><span class="badge ${r.action==="conflict"||r.action==="unavailable"?"failed":r.action==="unchanged"?"verified":"pending"}">${tr(labels[r.action])}</span></div><pre>${esc(r.value)}</pre>${r.existing.length?`<details ${r.action==="conflict"?"open":""}><summary>${tr("Existing records")}</summary><pre>${esc(r.existing.map(e=>`${e.type} ${e.content}${e.proxied?" (proxied)":""}`).join("\n"))}</pre></details>`:""}</section>`).join("")}${plan.records.some(r=>["conflict","unavailable"].includes(r.action))?`<p class="notice warning">${tr("Resolve conflicts or zone access before applying")}</p>`:""}`;
}
async function previewCloudflare(id){
  if(cloudflareBusy)return;
  cloudflareBusy=true;cloudflarePlan=null;cloudflareButtons();const request=cloudflareRequest;
  $("#cloudflare-content").innerHTML=`<p>${tr("Loading")}</p>`;
  try{const plan=await api(`/domains/${id}/cloudflare/preview`,{method:"POST"});if(request===cloudflareRequest&&$("#dialog").open&&$("#dialog").dataset.cloudflareId===id){cloudflarePlan=plan;$("#cloudflare-content").innerHTML=cloudflareRecords(plan);}}
  catch(error){if($("#cloudflare-content"))$("#cloudflare-content").innerHTML=`<p class="error">${tr(error.message)}</p>`;}
  finally{cloudflareBusy=false;cloudflareButtons();}
}
async function applyCloudflare(id){
  if(cloudflareBusy||!cloudflarePlan?.can_apply)return;
  cloudflareBusy=true;cloudflareButtons();const fingerprint=cloudflarePlan.fingerprint;cloudflarePlan=null;
  try{const result=await api(`/domains/${id}/cloudflare/apply`,{method:"POST",body:JSON.stringify({fingerprint})});
    if($("#cloudflare-content")){$("#cloudflare-content").innerHTML=`<p class="${result.complete?"notice":"error"}">${tr(result.complete?"DNS records created. Check DNS after propagation.":"Some DNS changes failed. Preview again before retrying.")}</p><dl class="details">${result.records.map(r=>detail(`${r.kind} ${r.name}`,t({created:"DNS created",unchanged:"DNS unchanged",failed:"DNS failed",skipped:"DNS skipped"}[r.status]))).join("")}</dl>`;}
    await render();
  }catch(error){if($("#cloudflare-content"))$("#cloudflare-content").innerHTML=`<p class="error">${tr(error.message)}</p>`;}
  finally{cloudflareBusy=false;cloudflareButtons();}
}
Object.assign(ZH,{
  "Workspace":"工作空间","Mail operations":"邮件管理","Configuration":"配置状态","Ready":"已就绪","Incomplete":"待完成","Verified sender":"已验证的发信域名","Delivery switch":"投递开关","Send test email":"发送测试邮件","Email language":"邮件语言","Delivery test":"投递测试","Test submitted":"测试已提交","Refresh status":"刷新状态","Open record":"查看记录","Preview":"预览","Refresh preview":"刷新预览","Desktop preview":"桌面预览","Mobile preview":"手机预览","Use template":"使用模板","Initial templates":"添加初始模板","Initial templates added":"初始模板已添加，已有内容保持不变","All languages":"全部语言","Search templates":"搜索模板","Custom template":"自定义模板","Welcome":"欢迎邮件","Verification code":"验证码","Notification":"事务通知","Variables":"变量","Updated":"更新时间","Copy template ID":"复制模板 ID","No matching templates":"没有匹配的模板","Template preview":"模板预览","No preview yet":"暂无预览","Configuration incomplete":"发信配置未完成","Domain verification":"域名验证","Complete delivery configuration before sending a test":"完成发信配置后才能发送测试邮件","Unsupported email language":"不支持的邮件语言","Delivery test status":"测试投递状态","Message preview":"邮件预览","Edit content":"编辑内容","Sender domains":"发信域名","Template library":"模板库","Restore initial templates?":"添加缺少的初始模板？已有模板不会被覆盖。","Template requires a name, subject and message body within the size limits":"请填写名称、主题和正文，并确认内容未超过大小限制"
});
const esc = value => String(value ?? "").replaceAll("&","&amp;").replaceAll("<","&lt;").replaceAll(">","&gt;").replaceAll(String.fromCharCode(34),"&quot;").replaceAll("'","&#39;");
Object.assign(ZH,{
  "Delivery is paused. Enable delivery in Settings":"投递已暂停，请在服务设置中开启投递",
  "Enable delivery and resume queued messages?":"开启投递并继续发送队列中的邮件？",
  "Queued messages are preserved. In-flight delivery may still complete.":"待发送邮件会保留，正在投递的邮件仍可能完成。"
});
Object.assign(ZH,{"Check again in {seconds}s":"{seconds} 秒后可重新检查","DNS verification passed":"DNS 验证已通过","DNS records are not fully verified":"DNS 记录尚未全部通过验证"});
const verifyCooldowns = new Map(), verifyPending = new Set();
Object.assign(ZH,{
  "Enter absolute directories":"请填写绝对目录",
  "Version updates":"版本更新","Current version":"当前版本","Latest release":"最新正式版","Release repository":"发布仓库","Last checked":"上次检查","Check for updates":"检查更新","Checking for updates":"正在检查更新","Not checked":"尚未检查","Update available":"有新版本","Up to date":"已是最新正式版","Ahead of latest release":"当前版本高于最新正式版","No public release found":"未找到可访问的正式发布","Update check failed":"检查失败，请确认服务器能访问 GitHub 后重试","Update checks disabled":"更新检查已关闭","Release page":"发布页面","Server commands":"服务器命令","Managed installation":"受管安装","Installation directory":"安装目录","Package directory":"已解压的安装包目录","Update command":"更新命令","Rollback command":"回退命令","Copy command":"复制命令","Source deployment":"源码部署","Deployment guide":"部署文档","Update and rollback guide":"更新与回退文档","Target version":"目标版本","Enter an absolute directory and a stable version":"请填写绝对目录和 major.minor.patch 格式的版本号","No release selected":"尚未选择发布版本"
});
let updateStatus=null, updateChecking=false, updateDeadline=0;
const updateLabels={not_checked:"Not checked",update_available:"Update available",up_to_date:"Up to date",ahead:"Ahead of latest release",no_release:"No public release found",check_failed:"Update check failed",disabled:"Update checks disabled"};
function versionPanel(){
  const data=updateStatus;
  const link=(url,label)=>`<a class="btn" href="${esc(url)}" target="_blank" rel="noopener noreferrer">${icon("external-link")}${tr(label)}</a>`;
  return `<section class="version-section"><div class="section-head"><h2>${tr("Version updates")}</h2><button class="btn" data-action="check-updates">${icon("refresh-cw")}${tr("Check for updates")}</button></div><p class="update-status ${data.status==="check_failed"?"error":"muted"}" role="status">${tr(updateLabels[data.status]||"Update check failed")}</p><dl class="details">${detail("Current version",data.current_version)}${detail("Latest release",data.check?.latest_version||"-")}${detail("Release repository",data.repository||t("Update checks disabled"))}${detail("Last checked",date(data.check?.checked_at))}</dl><div class="row version-actions">${data.releases_url?link(data.check?.release_url||data.releases_url,"Release page"):""}<button class="btn" data-action="update-commands">${icon("terminal")}${tr("Server commands")}</button></div></section>`;
}
function updateCheckButton(){
  const button=$('[data-action="check-updates"]');if(!button)return;
  const seconds=Math.max(0,Math.ceil((updateDeadline-Date.now())/1000));
  button.disabled=updateChecking||updateStatus?.status==="disabled"||seconds>0;
  const label=updateChecking?t("Checking for updates"):seconds>0?t("Check again in {seconds}s").replace("{seconds}",seconds):t("Check for updates");
  if(button.dataset.label!==label){button.innerHTML=icon("refresh-cw")+esc(label);button.dataset.label=label;icons();}
}
const shellQuote=value=>"'"+String(value).replaceAll("'","'\\''")+"'";
function commandDialog(){
  const number=updateStatus?.check?.latest_version||config.version;
  modal("Server commands",`<div class="form"><h3>${tr("Managed installation")}</h3>${field("install-directory","Installation directory","text",true,"/srv/bunmail")}${field("package-directory","Package directory","text",true,`/tmp/bunmail-packages/${number}`)}<div><div class="section-head"><h2>${tr("Update command")}</h2>${button("copy-update-command","copy","Copy command")}</div><pre id="update-command"></pre></div><div><div class="section-head"><h2>${tr("Rollback command")}</h2>${button("copy-rollback-command","copy","Copy command")}</div><pre id="rollback-command"></pre></div><div class="row"><a class="btn" href="https://github.com/AsukaCC/SMTP/blob/main/docs/releases.zh-CN.md" target="_blank" rel="noopener noreferrer">${icon("external-link")}${tr("Update and rollback guide")}</a><a class="btn" href="https://github.com/AsukaCC/SMTP/blob/main/docs/deployment.zh-CN.md" target="_blank" rel="noopener noreferrer">${icon("external-link")}${tr("Source deployment")}</a></div></div>`);
  updateCommands();
}
function updateCommands(){
  const directory=$("#install-directory")?.value.trim(),pkg=$("#package-directory")?.value.trim();
  if(directory===undefined)return;
  const valid=directory.startsWith("/")&&pkg.startsWith("/")&&!/[\r\n\0]/.test(directory+pkg);
  $("#update-command").textContent=valid?`python3 ${shellQuote(pkg+"/deploy.py")} --directory ${shellQuote(directory)} update ${shellQuote(pkg)}`:t("Enter absolute directories");
  $("#rollback-command").textContent=valid?`python3 ${shellQuote(pkg+"/deploy.py")} --directory ${shellQuote(directory)} rollback`:t("Enter absolute directories");
  for(const action of ["copy-update-command","copy-rollback-command"])$(`[data-action="${action}"]`).disabled=!valid;
}
function setVerifyCooldown(id, seconds) {
  const delay = Number(seconds);
  if (Number.isFinite(delay) && delay > 0) verifyCooldowns.set(id, Date.now() + Math.ceil(delay) * 1000);
  else verifyCooldowns.delete(id);
}
function updateVerifyButtons() {
  let refreshIcons = false;
  document.querySelectorAll('[data-action="verify"]').forEach(button => {
    const seconds = Math.max(0, Math.ceil(((verifyCooldowns.get(button.dataset.id)||0) - Date.now()) / 1000));
    button.disabled = seconds > 0 || verifyPending.has(button.dataset.id);
    if (seconds > 0) {
      button.textContent = t("Check again in {seconds}s").replace("{seconds}", String(seconds));
      button.dataset.cooldown = "true";
    } else if (button.dataset.cooldown) {
      delete button.dataset.cooldown;
      verifyCooldowns.delete(button.dataset.id);
      button.innerHTML = icon("refresh-cw") + tr("Check DNS");
      refreshIcons = true;
    }
  });
  if (refreshIcons) icons();
}
let locale = "zh-CN";
try { locale = localStorage.getItem("bunmail.dashboard.locale") === "en" ? "en" : "zh-CN"; } catch (_) {}
const t = text => locale === "zh-CN" ? (Object.hasOwn(ZH, text) ? ZH[text] : text) : text;
const icon = name => `<i data-lucide="${name}" aria-hidden="true"></i>`;
const tr = text => esc(t(text));
const date = value => value ? new Date(value).toLocaleString(locale, {month:"2-digit",day:"2-digit",hour:"2-digit",minute:"2-digit"}) : "-";
const statuses = {queued:"Queued",sending:"Sending",sent:"Sent",failed:"Failed",cancelled:"Cancelled"};
const badge = status => `<span class="badge ${esc(status)}"><span class="status-dot"></span>${tr(statuses[status] || status)}</span>`;
const button = (action, name, label, extra = "") => `<button type="button" class="btn icon" data-action="${action}" title="${tr(label)}" aria-label="${tr(label)}" ${extra}>${icon(name)}</button>`;
let user, config, domains = [], templates = [], presets = [], keys = [], page = 0, filter = "", sequence = 0, dirty = false, sendKey = crypto.randomUUID(), sendBody = "", chosenTemplate = "", templateLanguage = "zh-CN", templateSearch = "", testKey = "", testBody = "", testTimer;
const sections = [["dashboard","layout-dashboard","Dashboard"],["send","send","Send email"],["emails","mail","Emails"],["domains","globe","Domains"],["templates","file-text","Templates"],["keys","key-round","API keys"],["settings","settings-2","Settings"]];
let section = () => sections.some(s => s[0] === location.hash.slice(1)) ? location.hash.slice(1) : "dashboard";
function icons() { window.lucide?.createIcons({attrs:{"stroke-width":1.7}}); }
function language() { return `<div class="language" role="group" aria-label="Language"><button type="button" data-locale="zh-CN" class="${locale==="zh-CN"?"selected":""}" aria-pressed="${locale==="zh-CN"}">简体中文</button><button type="button" data-locale="en" class="${locale==="en"?"selected":""}" aria-pressed="${locale==="en"}">English</button></div>`; }
function toast(message) { const el=$("#toast"); el.textContent=t(message); el.hidden=false; clearTimeout(toast.timer); toast.timer=setTimeout(()=>el.hidden=true,4500); }
async function api(path, options={}) {
  let response;
  try { response=await fetch(`/api/v1${path}`, {credentials:"same-origin",...options,headers:{"Content-Type":"application/json",...options.headers}}); }
  catch (_) { throw new Error("Connection failed"); }
  const data=await response.json().catch(()=>({}));
  if (!response.ok) {
    if (response.status===401 && path!=="/login") {user=null;renderLogin();}
    const error = new Error(data.error?.message || "Request failed");
    error.status = response.status;
    error.code = data.error?.code;
    error.retryAfter = Number(data.error?.retry_after_seconds || response.headers.get("Retry-After")) || 0;
    throw error;
  }
  return data;
}
function empty(label, name="inbox") {return `<div class="empty">${icon(name)}<p>${tr(label)}</p></div>`;}
function heading(title, actions="", subtitle="") {return `<div class="heading"><div><h1>${tr(title)}</h1>${subtitle?`<p>${tr(subtitle)}</p>`:""}</div><div class="row">${actions}</div></div>`;}
function renderLogin(error="") {
  document.documentElement.lang=locale;
  $("#app").innerHTML=`<main class="login"><div class="login-inner"><div class="brand"><img src="/assets/logo.svg" alt="">BunMail <small>RS</small></div><h1>${tr("Sign in")}</h1><p class="muted">${tr("Personal instance")}</p><form id="login-form" class="form"><div class="field"><label for="email">${tr("Email")}</label><input id="email" name="email" type="email" autocomplete="username" required></div><div class="field"><label for="password">${tr("Password")}</label><input id="password" name="password" type="password" autocomplete="current-password" maxlength="256" required></div><div class="error" id="login-error" ${error?"":"hidden"}>${tr(error)}</div><button class="btn primary" type="submit">${icon("log-in")}${tr("Sign in")}</button></form>${language()}</div></main>`;
  icons();
}
function shell() {
  document.documentElement.lang=locale;
  const current=sections.find(s=>s[0]===section());
  $("#app").innerHTML=`<div class="app"><aside class="sidebar"><a href="#dashboard" class="brand"><img src="/assets/logo.svg" alt="">BunMail <small>RS</small></a><div class="workspace-label">${icon("server")}${tr("Personal instance")}</div><nav aria-label="${tr("Mail operations")}">${sections.map(([id,name,label])=>`<a href="#${id}" class="${section()===id?"active":""}" ${section()===id?'aria-current="page"':""}>${icon(name)}<span>${tr(label)}</span></a>`).join("")}</nav><div class="sidebar-foot">${language()}<div class="row account-row"><span class="avatar">${icon("user-round")}</span><span class="account" title="${esc(user.email)}">${esc(user.email)}</span>${button("logout","log-out","Sign out")}</div></div></aside><button class="menu-backdrop" data-action="menu" aria-label="${tr("Close")}" hidden></button><div class="main"><header class="topbar"><div class="row"><button class="btn icon mobile-menu" data-action="menu" title="${tr("Workspace")}" aria-label="${tr("Workspace")}">${icon("menu")}</button><span class="crumb">${tr("Workspace")}${icon("chevron-right")}<strong>${tr(current[2])}</strong></span></div><span class="badge ${config.delivery_enabled?"verified":"paused"}"><span class="status-dot"></span>${tr(config.delivery_enabled?"Delivery enabled":"Delivery paused")}</span></header><main class="content" id="content"></main></div></div>`;
  icons();
}
function mailTable(rows) {
  if(!rows.length)return empty("No emails yet");
  return `<div class="table-wrap"><table><thead><tr><th>${tr("Subject")}</th><th>${tr("Recipient")}</th><th>${tr("Status")}</th><th>${tr("Attempts")}</th><th>${tr("Created")}</th><th></th></tr></thead><tbody>${rows.map(row=>`<tr data-open="${esc(row.id)}"><td class="subject">${esc(row.subject)}</td><td>${esc(row.to_address)}</td><td>${badge(row.status)}</td><td>${row.attempts}</td><td class="muted">${date(row.created_at)}</td><td>${button("email","arrow-up-right","Email details",`data-id="${row.id}"`)}</td></tr>`).join("")}</tbody></table></div>`;
}
async function render() {
  if(!user) return renderLogin();
  const request=++sequence;const current=section();
  shell();$("#content").innerHTML=`<div class="loading">${tr("Loading")}…</div>`;
  try {
    config=await api("/config");
    if(request!==sequence)return;
    shell();
    let html="";
    if(current==="dashboard") {
      const [stats,emails]=await Promise.all([api("/stats"),api("/emails")]);
      html=heading("Dashboard",`<a class="btn primary" href="#send">${icon("plus")}${tr("New email")}</a>`)+
        readiness()+
        `<div class="stats">${[["queued","clock-3","Queued"],["sending","loader-circle","Sending"],["sent","circle-check","Sent"],["failed","circle-alert","Failed"]].map(([key,name,label])=>`<div class="stat"><span class="label">${icon(name)}${tr(label)}</span><strong>${Number(stats[key]).toLocaleString(locale)}</strong></div>`).join("")}</div><section><div class="section-head"><h2>${tr("Server identity")}</h2><a class="btn link" href="#domains">${tr("Domain checks")}${icon("arrow-right")}</a></div><dl class="details"><dt>${tr("Mail hostname")}</dt><dd class="mono">${esc(config.mail_hostname)}</dd><dt>${tr("Public IP")}</dt><dd class="mono">${esc(config.public_ip||t("Not configured"))}</dd><dt>${tr("Transport")}</dt><dd>${tr("Direct to recipient MX")}</dd></dl></section><section class="section"><div class="section-head"><h2>${tr("Recent emails")}</h2><a class="btn link" href="#emails">${tr("View all")}${icon("arrow-right")}</a></div>${mailTable(emails.data.slice(0,8))}</section>`;
    } else if(current==="emails") {
      const result=await api(`/emails?page=${page}${filter?`&status=${encodeURIComponent(filter)}`:""}`);
      html=heading("Emails",`<a class="btn primary" href="#send">${icon("plus")}${tr("New email")}</a>`)+`<div class="toolbar"><select class="filter" id="status-filter" aria-label="${tr("Status")}"><option value="">${tr("All statuses")}</option>${Object.entries(statuses).map(([value,label])=>`<option value="${value}" ${filter===value?"selected":""}>${tr(label)}</option>`).join("")}</select>${button("refresh","refresh-cw","Refresh")}</div>${mailTable(result.data)}<div class="pagination">${button("previous","chevron-left","Previous page",page===0?"disabled":"")}<span>${tr("Page")} ${page+1}</span>${button("next","chevron-right","Next page",!result.has_more?"disabled":"")}</div>`;
    } else if(current==="send") {
      await loadTemplates();
      html=heading("Send email",config.ready_to_test?testButton():"","Only one recipient per message")+(!config.ready_to_test?readiness():"")+`<div class="compose-layout"><form class="form" id="send-form"><div class="grid2">${field("from_name","Sender name")}${field("from","From","email",true)}</div>${field("to","To","email",true)}<div class="field"><label for="template_id">${tr("Template")}</label><select id="template_id" name="template_id"><option value="">${tr("No template")}</option>${templates.map(row=>`<option value="${row.id}">${esc(row.name)}</option>`).join("")}</select></div><div id="manual-body" class="form">${field("subject","Subject","text",true)}${area("html","HTML body",9)}${area("text","Text body",4)}</div><div id="template-variables" hidden>${area("variables","Variables (JSON)",9,"{}")}</div><div class="form-foot"><button class="btn" type="button" data-action="preview-compose">${icon("eye")}${tr("Preview")}</button><button class="btn primary" type="submit" ${!config.ready_to_test?"disabled":""}>${icon("send")}${tr("Send")}</button></div></form>${previewPane("compose-preview")}</div>`;
    } else if(current==="domains") {
      domains=(await api("/domains")).data;
      for (const row of domains) setVerifyCooldown(row.domain.id, row.retry_after_seconds);
      html=heading("Domains",`<button class="btn primary" data-action="add-domain">${icon("plus")}${tr("Add domain")}</button>`)+(domains.length?domains.map(domainView).join(""):empty("No domains yet","globe"));
    } else if(current==="templates") {
      await loadTemplates();
      html=heading("Templates",`<button class="btn" data-action="initialize-templates">${icon("library")}${tr("Initial templates")}</button><button class="btn primary" data-action="add-template">${icon("plus")}${tr("New template")}</button>`)+`<div class="toolbar template-toolbar"><label class="search-field">${icon("search")}<input type="search" id="template-search" value="${esc(templateSearch)}" placeholder="${tr("Search templates")}" aria-label="${tr("Search templates")}"></label><select id="template-language" class="filter" aria-label="${tr("Email language")}">${[["all","All languages"],["zh-CN","简体中文"],["en","English"]].map(([value,label])=>`<option value="${value}" ${templateLanguage===value?"selected":""}>${tr(label)}</option>`).join("")}</select></div><div id="template-list">${templateCards()}</div>`;
    } else if(current==="keys") {
      keys=(await api("/api-keys")).data;
      html=heading("API keys",`<button class="btn primary" data-action="add-key">${icon("plus")}${tr("New API key")}</button>`)+(keys.length?`<div class="table-wrap"><table><thead><tr><th>${tr("Name")}</th><th>${tr("Prefix")}</th><th>${tr("Status")}</th><th>${tr("Created")}</th><th>${tr("Actions")}</th></tr></thead><tbody>${keys.map(row=>`<tr><td>${esc(row.name)}</td><td class="mono">${esc(row.prefix)}…</td><td><span class="badge ${row.revoked_at?"":"verified"}">${tr(row.revoked_at?"Revoked":"Active")}</span></td><td class="muted">${date(row.created_at)}</td><td>${row.revoked_at?"":button("revoke-key","key-round","Revoke",`data-id="${row.id}"`)}</td></tr>`).join("")}</tbody></table></div>`:empty("No API keys yet","key-round"));
    } else {
      updateStatus=await api("/updates");updateDeadline=Date.now()+updateStatus.retry_after_seconds*1000;
      html=heading("Settings",button("refresh","refresh-cw","Refresh"))+readiness()+`<div class="settings-grid"><section><h2>${icon("server")}${tr("Version & deployment")}</h2><dl class="details">${detail("Service version",`BunMail RS ${config.version}`)}${detail("Mail hostname",config.mail_hostname)}${detail("Public IP",config.public_ip||t("Not configured"))}${detail("Status",t(config.delivery_enabled?"Delivery enabled":"Delivery paused"))}</dl></section><section><h2>${icon("network")}${tr("SMTP submission")}</h2><dl class="details">${detail("Status",t(config.smtp_enabled?"Enabled":"Disabled"))}${detail("SMTP port",config.smtp_port||"-")}${detail("Transport",t("STARTTLS required"))}${detail("SMTP username","bunmail")}${detail("SMTP password",t("An active API key"))}</dl></section><section><h2>${icon("code-2")}${tr("HTTP API")}</h2><dl class="details">${detail("API endpoint",`${location.origin}/api/v1/emails/send`)}${detail("Authentication",t("Bearer API key"))}</dl></section></div>`;
      html+=versionPanel();
    }
    if(request!==sequence)return;
    if(config.rdns_warning&&current!=="domains")html=rdnsNotice()+html;
    $("#content").innerHTML=html;icons();updateVerifyButtons();updateCheckButton();dirty=false;
    if(current==="send" && chosenTemplate && templates.some(row=>row.id===chosenTemplate)){
      $("#template_id").value=chosenTemplate;syncTemplate();chosenTemplate="";
    }
  } catch(error) { if(request===sequence && $("#content"))$("#content").innerHTML=`<div class="error">${tr(error.message)}</div>`; }
}
function testButton(){return `<button class="btn primary" data-action="test-email">${icon("send-horizontal")}${tr("Send test email")}</button>`;}
function readiness(){
  const checks=[[!!config.public_ip,"Public IP",config.public_ip||t("Not configured")],[!!config.verified_domains?.length,"Verified sender",config.verified_domains?.join(", ")||t("Pending")],[config.delivery_enabled,"Delivery switch",t(config.delivery_enabled?"Enabled":"Disabled")]];
  const canEnable=!!config.public_ip&&!!config.verified_domains?.length;
  return `<section class="readiness" aria-label="${tr("Configuration")}"><div class="section-head"><div class="row"><h2>${tr("Configuration")}</h2><span class="badge ${config.ready_to_test?"verified":"pending"}">${tr(config.ready_to_test?"Ready":"Incomplete")}</span></div>${config.ready_to_test?testButton():`<a class="btn link" href="${canEnable?'#settings':'#domains'}">${tr(canEnable?"Settings":"Domain checks")}${icon("arrow-up-right")}</a>`}</div><div class="readiness-steps">${checks.map(([ok,label,value])=>`<div class="readiness-step ${ok?"complete":""}">${icon(ok?"circle-check":"circle-dashed")}<div><span>${tr(label)}</span>${label==="Delivery switch"&&section()==="settings"?`<button type="button" class="btn delivery-toggle" role="switch" aria-checked="${config.delivery_enabled}" aria-label="${tr("Delivery switch")}" data-action="toggle-delivery" ${!config.delivery_enabled&&!canEnable?'disabled':''} title="${tr(canEnable?"Delivery switch":"Please configure a public IP and DNS before enabling delivery.")}">${icon(config.delivery_enabled?"toggle-right":"toggle-left")}${esc(value)}</button>`:`<strong>${esc(value)}</strong>`}</div></div>`).join("")}</div>${section()==="settings"?`<p class="muted delivery-note">${tr(!canEnable&&!config.delivery_enabled?"Please configure a public IP and DNS before enabling delivery.":"Queued messages are preserved. In-flight delivery may still complete.")}</p>`:""}</section>`;
}
async function loadTemplates(){const result=await api("/templates");templates=result.data;presets=result.presets||[];}
function variableNames(row){return [...new Set([...`${row.subject} ${row.html} ${row.text}`.matchAll(/\{\{\s*(\w+)\s*\}\}/g)].map(match=>match[1]))];}
function sampleVariables(row){const preset=presets.find(p=>p.id===row.id);return Object.fromEntries(variableNames(row).map(name=>[name,preset?.variables[name]??""]));}
function templateCards(){
  const rows=templates.filter(row=>{const preset=presets.find(p=>p.id===row.id);return (!preset||templateLanguage==="all"||preset.language===templateLanguage)&&`${row.name} ${row.subject}`.toLowerCase().includes(templateSearch.toLowerCase());});
  if(!rows.length)return empty(templates.length?"No matching templates":"No templates yet","files");
  const categories={welcome:["hand","Welcome"],verification:["shield-check","Verification code"],notification:["bell","Notification"],test:["send-horizontal","Delivery test"]};
  return `<div class="template-grid">${rows.map(row=>{
    const preset=presets.find(p=>p.id===row.id), [symbol,label]=categories[preset?.category]||["file-text","Custom template"];
    return `<article class="template-card" data-template="${row.id}"><div class="template-card-top"><span class="template-symbol ${esc(preset?.category||"custom")}">${icon(symbol)}</span><span class="muted">${tr(label)}</span>${preset?`<span class="locale-tag">${preset.language==="en"?"EN":"中文"}</span>`:""}</div><h2>${esc(row.name)}</h2><p class="template-subject">${esc(row.subject)}</p><div class="template-excerpt">${esc(row.text||row.html.replace(/<[^>]*>/g," ")).slice(0,180)}</div><div class="variable-chips">${variableNames(row).slice(0,5).map(name=>`<code>{{ ${esc(name)} }}</code>`).join("")}</div><div class="template-card-foot"><button class="btn link" data-action="use-template" data-id="${row.id}">${tr("Use template")}${icon("arrow-right")}</button><div class="row compact">${button("preview-template","eye","Preview",`data-id="${row.id}"`)}${button("edit-template","pencil","Edit",`data-id="${row.id}"`)}${button("copy-template","copy","Copy template ID",`data-id="${row.id}"`)}${button("delete-template","trash-2","Delete",`data-id="${row.id}"`)}</div></div></article>`;
  }).join("")}</div>`;
}
function previewPane(id){return `<aside class="preview-tool" data-preview="${id}"><div class="preview-toolbar"><h2>${tr("Message preview")}</h2><div class="segmented" role="group" aria-label="${tr("Preview")}"><button type="button" class="selected" data-action="preview-width" data-width="desktop" aria-pressed="true" title="${tr("Desktop preview")}" aria-label="${tr("Desktop preview")}">${icon("monitor")}</button><button type="button" data-action="preview-width" data-width="mobile" aria-pressed="false" title="${tr("Mobile preview")}" aria-label="${tr("Mobile preview")}">${icon("smartphone")}</button></div></div><div class="preview-stage"><div class="preview-empty">${icon("mail-open")}<p>${tr("No preview yet")}</p></div><iframe class="preview" id="${id}" name="${id}" title="${tr("Message preview")}" sandbox="" referrerpolicy="no-referrer" hidden></iframe></div></aside>`;}
function showPreview(input,id){
  const iframe=document.getElementById(id);if(!iframe)return;
  JSON.parse(input.variables||"{}");
  const form=document.createElement("form");form.method="POST";form.action="/api/v1/templates/preview";form.target=id;form.hidden=true;
  for(const [name,value]of Object.entries(input)){if(value===undefined||value===null||name==="template_id"&&!value)continue;const field=document.createElement("input");field.name=name;field.value=String(value);form.append(field);}
  document.body.append(form);iframe.hidden=false;iframe.closest(".preview-stage").querySelector(".preview-empty").hidden=true;form.submit();form.remove();
}
function composePreview(){const values=Object.fromEntries(new FormData($("#send-form")));showPreview({subject:values.subject,html:values.html,text:values.text,template_id:values.template_id,variables:values.template_id?values.variables:"{}"},"compose-preview");}
function previewTemplate(id){
  const row=templates.find(row=>row.id===id);if(!row)return;
  modal("Template preview",`<div class="compose-layout"><form class="form" id="preview-form" data-id="${id}"><h3>${esc(row.name)}</h3><p class="muted">${esc(row.subject)}</p>${area("preview_variables","Variables (JSON)",8,JSON.stringify(sampleVariables(row),null,2))}<button class="btn" type="submit">${icon("refresh-cw")}${tr("Refresh preview")}</button></form>${previewPane("template-preview")}</div>`,true);
  showPreview({template_id:id,variables:JSON.stringify(sampleVariables(row))},"template-preview");
}
function openTest(){
  if(!config.ready_to_test)return;
  testKey=crypto.randomUUID();testBody="";
  modal("Send test email",`${config.rdns_warning?rdnsNotice():""}<form id="test-form" class="form">${field("test_from_name","Sender name")}${field("test_from","From","email",true,`test@${config.verified_domains[0]}`)}${field("test_to","To","email",true)}<div class="field"><label for="test_language">${tr("Email language")}</label><select id="test_language" name="test_language"><option value="zh-CN">简体中文</option><option value="en">English</option></select></div><div class="form-foot"><button class="btn primary" type="submit">${icon("send-horizontal")}${tr("Send test email")}</button></div></form>`);
}
async function pollTest(id){
  clearTimeout(testTimer);if(!$("#dialog").open||$("#dialog").dataset.emailId!==id)return;
  try{const {email}=await api(`/emails/${id}`);if(!$("#dialog").open||$("#dialog").dataset.emailId!==id)return;
    $("#test-result").innerHTML=`<div class="test-status">${badge(email.status)}</div><dl class="details">${detail("From",email.from_address)}${detail("To",email.to_address)}${detail("Subject",email.subject)}${detail("Attempts",email.attempts)}${email.last_error?detail("Last error",t(email.last_error)):""}${email.status==="queued"?detail("Next attempt",date(email.next_attempt_at)):""}</dl>${email.status==="sent"?`<p class="muted section">${tr("Accepted by recipient SMTP server; inbox placement is not confirmed.")}</p>`:""}`;
    if(["queued","sending"].includes(email.status))testTimer=setTimeout(()=>void pollTest(id),2500);
  }catch(error){toast(error.message);}
}
function detail(label,value) {return `<dt>${tr(label)}</dt><dd>${esc(value)}</dd>`;}
function field(name,label,type="text",required=false,value="") {return `<div class="field"><label for="${name}">${tr(label)}</label><input id="${name}" name="${name}" type="${type}" ${required?"required":""} value="${esc(value)}" maxlength="${name==="subject"?998:254}"></div>`;}
function area(name,label,rows=6,value="") {return `<div class="field"><label for="${name}">${tr(label)}</label><textarea id="${name}" name="${name}" rows="${rows}" ${name==="html"?'class="mono"':""}>${esc(value)}</textarea></div>`;}
function modal(title,html,wide=false) {
  clearTimeout(testTimer);const d=$("#dialog");if(d.open)d.close();d.dataset.emailId="";d.dataset.cloudflareId="";d.classList.toggle("wide",wide);d.innerHTML=`<div class="dialog-head"><h2>${tr(title)}</h2>${button("close","x","Close")}</div>${html}`;d.showModal();icons();$("input,textarea",d)?.focus();
}
function simpleForm(id,title,name,label) {modal(title,`<form id="${id}" class="form">${field(name,label,"text",true)}<div class="form-foot"><button type="button" class="btn" data-action="close">${tr("Cancel")}</button><button type="submit" class="btn primary">${icon("plus")}${tr("Create")}</button></div></form>`);}
function editTemplate(id) {
  const row=templates.find(t=>t.id===id)||{};
  modal(id?"Edit template":"New template",`<div class="compose-layout"><form id="template-form" class="form" data-id="${esc(id||"")}">${field("name","Name","text",true,row.name)}${field("subject","Subject","text",true,row.subject)}${area("html","HTML body",9,row.html)}${area("text","Text body",4,row.text)}<details><summary>${tr("Variables")}</summary>${area("preview_variables","Variables (JSON)",4,JSON.stringify(sampleVariables(row),null,2))}</details><div class="form-foot"><button type="button" class="btn" data-action="preview-editor">${icon("eye")}${tr("Preview")}</button><button class="btn primary" type="submit">${icon("save")}${tr("Save")}</button></div></form>${previewPane("editor-preview")}</div>`,true);
  if(id)showPreview({template_id:id,variables:JSON.stringify(sampleVariables(row))},"editor-preview");
}
async function emailDetails(id) {
  const {email:e,attempts}=await api(`/emails/${id}`);
  modal("Email details",`<h3>${esc(e.subject)}</h3><dl class="details">${detail("From",e.from_address)}${detail("To",e.to_address)}<dt>${tr("Status")}</dt><dd>${badge(e.status)}</dd>${detail("Message ID",e.message_id)}${detail("Created",date(e.created_at))}${detail("Accepted at",date(e.sent_at))}${e.status==="queued"?detail("Next attempt",date(e.next_attempt_at)):""}${e.last_error?detail("Last error",t(e.last_error)):""}</dl>${e.status==="sent"?`<p class="muted section">${tr("Accepted by recipient SMTP server; inbox placement is not confirmed.")}</p>`:""}<h3 class="section">${tr("Message")}</h3><pre>${esc(e.text||e.html)}</pre><h3 class="section">${tr("Delivery attempts")}</h3>${attempts.length?`<div class="table-wrap"><table><tbody>${attempts.map(a=>`<tr><td>${date(a.created_at)}</td><td>${badge(a.result)}</td><td>${tr(a.detail)}</td></tr>`).join("")}</tbody></table></div>`:`<p class="muted">${tr("No delivery attempts")}</p>`}<div class="form-foot">${e.status==="failed"?`<button class="btn primary" data-action="retry" data-id="${id}">${icon("rotate-ccw")}${tr("Retry")}</button>`:""}${e.status==="queued"?`<button class="btn danger" data-action="cancel-email" data-id="${id}">${icon("circle-x")}${tr("Cancel")}</button>`:""}</div>`);
}
document.addEventListener("submit",async event=>{
  const form=event.target;if(!(form instanceof HTMLFormElement))return;event.preventDefault();
  const submit=form.querySelector('[type="submit"]');submit.disabled=true;
  const values=Object.fromEntries(new FormData(form));
  try {
    if(form.id==="login-form") {user=await api("/login",{method:"POST",body:JSON.stringify(values)});config=await api("/config");await render();return;}
    if(form.id==="domain-form") {await api("/domains",{method:"POST",body:JSON.stringify(values)});$("#dialog").close();toast("Saved");await render();}
    if(form.id==="key-form") {const key=await api("/api-keys",{method:"POST",body:JSON.stringify(values)});await render();modal("Key created",`<p class="muted">${tr("This key is shown only once.")}</p><pre class="reveal" id="new-key">${esc(key.token)}</pre><div class="form-foot"><button class="btn primary" data-action="copy-key">${icon("copy")}${tr("Copy")}</button></div>`);}
    if(form.id==="template-form") {const {preview_variables,...input}=values;await api(`/templates${form.dataset.id?`/${form.dataset.id}`:""}`,{method:form.dataset.id?"PUT":"POST",body:JSON.stringify(input)});$("#dialog").close();toast("Saved");dirty=false;await render();}
    if(form.id==="preview-form")showPreview({template_id:form.dataset.id,variables:values.preview_variables},"template-preview");
    if(form.id==="test-form"){
      const body=JSON.stringify({from:values.test_from,from_name:values.test_from_name,to:values.test_to,language:values.test_language});if(testBody!==body){testKey=crypto.randomUUID();testBody=body;}
      const result=await api("/emails/test",{method:"POST",headers:{"Idempotency-Key":testKey},body});
      modal("Delivery test status",`<div id="test-result">${tr("Loading")}</div><div class="form-foot">${button("refresh-test","refresh-cw","Refresh status",`data-id="${result.id}"`)}<button class="btn" data-action="email" data-id="${result.id}">${icon("arrow-up-right")}${tr("Open record")}</button></div>`);$("#dialog").dataset.emailId=result.id;await pollTest(result.id);
    }
    if(form.id==="send-form") {
      const input={from:values.from,from_name:values.from_name,to:values.to,subject:values.subject,html:values.html,text:values.text};
      if(values.template_id){input.template_id=values.template_id;input.variables=JSON.parse(values.variables);if(!input.variables||Array.isArray(input.variables)||typeof input.variables!=="object")throw new Error("Use valid JSON object for variables");}
      const body=JSON.stringify(input);if(body!==sendBody){sendKey=crypto.randomUUID();sendBody=body;}
      await api("/emails/send",{method:"POST",headers:{"Idempotency-Key":sendKey},body});sendKey=crypto.randomUUID();sendBody="";dirty=false;toast("Message queued");location.hash="emails";
    }
  }catch(error){const message=error instanceof SyntaxError?"Invalid JSON":error.message;if(form.id==="login-form"){$("#login-error").textContent=t(message);$("#login-error").hidden=false;}else toast(message);}
  finally{submit.disabled=false;}
});
document.addEventListener("click",async event=>{
  const languageButton=event.target.closest("[data-locale]");
  if(languageButton){
    const values=[...document.querySelectorAll("form")].map(f=>[f.id,Object.fromEntries(new FormData(f))]);const wasDirty=dirty;
    locale=languageButton.dataset.locale;try{localStorage.setItem("bunmail.dashboard.locale",locale);}catch(_){}
    $("#dialog").close();await render();for(const [id,data]of values){const form=document.getElementById(id);if(form)for(const [key,value]of Object.entries(data)){const input=form.elements.namedItem(key);if(input&&"value"in input)input.value=value;}}
    if($("#template_id"))syncTemplate(false);dirty=wasDirty;return;
  }
  const nav=event.target.closest('nav a,.brand');if(nav){if(dirty&&!confirm(t("Changes not saved. Leave this page?"))){event.preventDefault();return;}dirty=false;$(".sidebar")?.classList.remove("open");if($(".menu-backdrop"))$(".menu-backdrop").hidden=true;}
  const action=event.target.closest("[data-action]");
  if(!action){const row=event.target.closest("[data-open]");if(row)try{await emailDetails(row.dataset.open);}catch(e){toast(e.message);}return;}
  const id=action.dataset.id;if(action.disabled)return;action.disabled=true;
  try {
    switch(action.dataset.action){
      case "toggle-delivery":{
        const enabled=action.getAttribute("aria-checked")!=="true";
        if(enabled&&!confirm(t("Enable delivery and resume queued messages?")))break;
        await api("/config/delivery",{method:"PUT",body:JSON.stringify({enabled})});
        toast(enabled?"Delivery enabled":"Delivery paused");await render();break;
      }
      case "cloudflare":openCloudflare(id);break;
      case "cloudflare-preview":await previewCloudflare(id);break;
      case "cloudflare-apply":await applyCloudflare(id);break;
      case "check-updates":
        updateChecking=true;updateCheckButton();
        try{updateStatus=await api("/updates/check",{method:"POST"});updateDeadline=Date.now()+updateStatus.retry_after_seconds*1000;if(section()==="settings")await render();}
        finally{updateChecking=false;updateCheckButton();}break;
      case "update-commands":commandDialog();break;
      case "copy-update-command":case "copy-rollback-command":await navigator.clipboard.writeText($(action.dataset.action==="copy-update-command"?"#update-command":"#rollback-command").textContent);toast("Copied");break;
      case "menu":$(".sidebar").classList.toggle("open");$(".menu-backdrop").hidden=!$(".sidebar").classList.contains("open");break;
      case "logout":await api("/logout",{method:"POST"});user=null;renderLogin();break;
      case "refresh":config=await api("/config");await render();break;
      case "add-domain":simpleForm("domain-form","Add domain","name","Domain name");break;
      case "add-key":simpleForm("key-form","New API key","name","Key name");break;
      case "verify":{
        verifyPending.add(id);updateVerifyButtons();
        const result=await api(`/domains/${id}/verify`,{method:"POST"});
        setVerifyCooldown(id,result.retry_after_seconds);
        toast(result.verified?"DNS verification passed":"DNS records are not fully verified");await render();break;
      }
      case "delete-domain":if(confirm(t("Delete this record?"))){await api(`/domains/${id}`,{method:"DELETE"});await render();}break;
      case "copy-dns":await navigator.clipboard.writeText(domains.find(r=>r.domain.id===id).records[Number(action.dataset.index)].value);toast("Copied");break;
      case "add-template":editTemplate();break;
      case "initialize-templates":if(confirm(t("Restore initial templates?"))){await api("/templates/initialize",{method:"POST"});toast("Initial templates added");await render();}break;
      case "preview-template":previewTemplate(id);break;
      case "copy-template":await navigator.clipboard.writeText(id);toast("Copied");break;
      case "use-template":chosenTemplate=id;dirty=false;location.hash="send";break;
      case "preview-compose":composePreview();break;
      case "preview-editor":{const values=Object.fromEntries(new FormData($("#template-form")));showPreview({subject:values.subject,html:values.html,text:values.text,variables:values.preview_variables},"editor-preview");break;}
      case "preview-width":{const tool=action.closest(".preview-tool");tool.classList.toggle("mobile-preview",action.dataset.width==="mobile");tool.querySelectorAll("[data-width]").forEach(button=>{const selected=button===action;button.classList.toggle("selected",selected);button.setAttribute("aria-pressed",String(selected));});break;}
      case "test-email":config=await api("/config");if(config.ready_to_test)openTest();else{toast("Configuration incomplete");await render();}break;
      case "refresh-test":await pollTest(id);break;
      case "edit-template":editTemplate(id);break;
      case "delete-template":if(confirm(t("Delete this record?"))){await api(`/templates/${id}`,{method:"DELETE"});await render();}break;
      case "revoke-key":if(confirm(t("Revoke this API key?"))){await api(`/api-keys/${id}`,{method:"DELETE"});await render();}break;
      case "copy-key":await navigator.clipboard.writeText($("#new-key").textContent);toast("Copied");break;
      case "close":if($("#template-form")&&dirty&&!confirm(t("Changes not saved. Leave this page?")))break;$("#dialog").close();if($("#template-form"))dirty=false;break;
      case "previous":page=Math.max(0,page-1);await render();break;
      case "next":page++;await render();break;
      case "email":await emailDetails(id);break;
      case "retry":await api(`/emails/${id}/retry`,{method:"POST"});$("#dialog").close();toast("Retry scheduled");await render();break;
      case "cancel-email":if(confirm(t("Cancel queued email?"))){await api(`/emails/${id}/cancel`,{method:"POST"});$("#dialog").close();await render();}break;
    }
  }catch(error){if(action.dataset.action==="verify" && error.retryAfter>0)setVerifyCooldown(id,error.retryAfter);toast(error.message);}finally{if(action.dataset.action==="verify")verifyPending.delete(id);action.disabled=false;updateVerifyButtons();cloudflareButtons();}
});
function syncTemplate(fill=true){
  const select=$("#template_id");const use=!!select.value;$("#manual-body").hidden=use;$("#template-variables").hidden=!use;$("#subject").required=!use;
  if(use&&fill){const template=templates.find(t=>t.id===select.value);$("#variables").value=JSON.stringify(sampleVariables(template),null,2);}
  if(use){try{composePreview();}catch(_){}}
}
document.addEventListener("change",event=>{
  if(event.target.id==="status-filter"){filter=event.target.value;page=0;void render();}
  if(event.target.id==="template_id")syncTemplate();
  if(event.target.id==="template-language"){templateLanguage=event.target.value;$("#template-list").innerHTML=templateCards();icons();}
});
document.addEventListener("input",event=>{if(event.target.closest("#send-form,#template-form"))dirty=true;if(event.target.id==="template-search"){templateSearch=event.target.value;$("#template-list").innerHTML=templateCards();icons();}});
document.addEventListener("input",event=>{if(["install-directory","package-directory"].includes(event.target.id))updateCommands();});
$("#dialog").addEventListener("close",()=>clearTimeout(testTimer));
window.addEventListener("beforeunload",event=>{if(dirty){event.preventDefault();event.returnValue="";}});
window.addEventListener("hashchange",()=>{page=0;void render();});
setInterval(()=>{if(user&&!$("#dialog").open&&!dirty&&["dashboard","emails"].includes(section())&&document.visibilityState==="visible")void render();},30000);
setInterval(updateVerifyButtons,1000);
setInterval(updateCheckButton,1000);
(async()=>{try{user=await api("/me");config=await api("/config");await render();}catch(_){renderLogin();}})();
