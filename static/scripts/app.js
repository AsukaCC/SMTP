"use strict";
const $ = (s, root = document) => root.querySelector(s);
let cloudflarePlan=null, cloudflareBusy=false, cloudflareRequest=0;
function rdnsNotice(){return `<p class="notice warning">${icon("triangle-alert")}<span>${tr("rDNS is missing, mismatched or unavailable. Sending is allowed, but recipients may reject the message or mark it as spam.")}</span></p>`;}
function domainView({domain:d,records}){
  const expired=d.checked_at&&Date.now()-Date.parse(d.checked_at)>=86400000;
  const status=d.verified?"Verified":!d.checked_at?"Not checked":expired?"Verification expired":"Required checks incomplete";
  const check=(key,label,optional=false)=>`<span class="check ${d.checks[key]?"ok":d.checked_at?(optional?"warn":"failed"):""}">${icon(d.checks[key]?"circle-check":d.checked_at?(optional?"triangle-alert":"circle-x"):"circle-dashed")}${tr(label)}</span>`;
  return `<section class="domain"><div class="domain-head"><div class="domain-identity"><h2>${icon("globe")}${esc(d.name)}</h2><span class="badge ${d.verified?"verified":"pending"}">${tr(status)}</span></div><div class="row"><button class="btn" data-action="cloudflare" data-id="${d.id}">${icon("cloud-cog")}${tr("Configure Cloudflare")}</button><button class="btn" data-action="verify" data-id="${d.id}">${icon("refresh-cw")}${tr("Check DNS")}</button>${button("delete-domain","trash-2","Delete",`data-id="${d.id}"`)}</div></div><div class="dns-status"><div><span class="group-label">${tr("Required DNS")}</span><div class="checks">${[["spf","SPF"],["dkim","DKIM"],["dmarc","DMARC"],["forward","A / AAAA"]].map(([key,label])=>check(key,label)).join("")}</div></div><div><span class="group-label">${tr("Optional DNS")}</span><div class="checks">${check("ptr","PTR / rDNS (optional)",true)}</div></div><div class="dns-checked"><span class="group-label">${tr("Last checked")}</span><time datetime="${esc(d.checked_at||"")}" title="${tr("DNS verification is valid for 24 hours")}">${date(d.checked_at)}</time></div></div>${d.checked_at&&!d.checks.ptr?rdnsNotice():""}<details><summary>${tr("DNS records")}</summary><div class="table-wrap"><table class="dns-table"><thead><tr><th>${tr("Type")}</th><th>${tr("Name")}</th><th>${tr("Value")}</th><th></th></tr></thead><tbody>${records.map((r,index)=>`<tr><td><span class="record-type">${esc(r.kind)}</span></td><td><code>${esc(r.name)}</code></td><td><code>${esc(r.value)}</code></td><td>${button("copy-dns","copy","Copy",`data-id="${d.id}" data-index="${index}"`)}</td></tr>`).join("")}</tbody></table></div></details></section>`;
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
const esc = value => String(value ?? "").replaceAll("&","&amp;").replaceAll("<","&lt;").replaceAll(">","&gt;").replaceAll(String.fromCharCode(34),"&quot;").replaceAll("'","&#39;");
const verifyCooldowns = new Map(), verifyPending = new Set();
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
let emailSearch="", dialogDirty=false, renderedSection="", renderedLocale="", renderedHTML="", activeHash=location.hash, bodyMode="html", composeDraft=null;
const hasUnsavedChanges=()=>dirty||dialogDirty;
const captureForm=form=>form?Object.fromEntries(new FormData(form)):null;
function restoreForm(form,values){if(form&&values)for(const [key,value]of Object.entries(values)){const input=form.elements.namedItem(key);if(input&&"value"in input)input.value=value;}}
function formError(form,message){let error=$(".form-error",form);if(!error){error=document.createElement("p");error.className="error form-error";error.setAttribute("role","alert");error.tabIndex=-1;form.append(error);}error.textContent=t(message);error.hidden=false;error.focus();}
function closeDialog(){if(dialogDirty&&!confirm(t("Changes not saved. Leave this page?")))return false;dialogDirty=false;$("#dialog").close();return true;}
const sections = [["dashboard","layout-dashboard","Dashboard"],["send","send","Send email"],["emails","mail","Emails"],["domains","globe","Domains"],["templates","file-text","Templates"],["keys","key-round","API keys"],["settings","settings-2","Settings"]];
let section = () => sections.some(s => s[0] === location.hash.slice(1)) ? location.hash.slice(1) : "dashboard";
function icons() { window.lucide?.createIcons({attrs:{"stroke-width":1.7}}); }
function language() { return `<div class="language" role="group" aria-label="Language"><button type="button" data-locale="zh-CN" class="${locale==="zh-CN"?"selected":""}" aria-pressed="${locale==="zh-CN"}">简体中文</button><button type="button" data-locale="en" class="${locale==="en"?"selected":""}" aria-pressed="${locale==="en"}">English</button></div>`; }
function themeControl() {
  const choice = window.bunmailTheme?.choice?.() || "system";
  const options = [["light","sun","Light"],["dark","moon","Dark"],["system","monitor","System"]];
  return `<div class="theme" role="group" aria-label="${tr("Appearance")}">${options.map(([value,name,label])=>`<button type="button" data-theme-choice="${value}" class="${choice===value?"selected":""}" aria-pressed="${choice===value}" title="${tr(label)}" aria-label="${tr(label)}">${icon(name)}</button>`).join("")}</div>`;
}
function preferences() { return `<div class="prefs">${language()}${themeControl()}</div>`; }
function toast(message) { const el=$("#toast"); el.textContent=t(message); el.hidden=false; clearTimeout(toast.timer); toast.timer=setTimeout(()=>el.hidden=true,4500); }
async function api(path, options={}) {
  let response;
  try { response=await fetch(`/api/v1${path}`, {credentials:"same-origin",...options,headers:{"Content-Type":"application/json",...options.headers}}); }
  catch (_) { throw new Error("Connection failed"); }
  const data=await response.json().catch(()=>({}));
  if (!response.ok) {
    if (response.status===401 && path!=="/login") {sequence++;user=null;dirty=false;dialogDirty=false;$("#dialog").close();renderLogin("Session expired. Please sign in again");}
    const error = new Error(data.error?.message || "Request failed");
    error.status = response.status;
    error.code = data.error?.code;
    error.retryAfter = Number(data.error?.retry_after_seconds || response.headers.get("Retry-After")) || 0;
    throw error;
  }
  return data;
}
function empty(label, name="inbox") {return `<div class="empty">${icon(name)}<p>${tr(label)}</p></div>`;}
function heading(title, actions="", subtitle="") {return `<div class="heading"><div><h1>${tr(title)}</h1>${subtitle?`<p>${tr(subtitle)}</p>`:""}</div><div class="row">${actions}</div></div>${config?.rdns_warning&&["dashboard","send","settings"].includes(section())?rdnsNotice():""}`;}
function renderLogin(error="") {
  renderedSection="";renderedHTML="";
  document.documentElement.lang=locale;
  document.title=`${t("Sign in")} · BunMail RS`;
  $("#app").innerHTML=`<main class="login"><div class="login-inner"><div class="brand"><img src="/assets/logo.svg" alt="">BunMail <small>RS</small></div><h1>${tr("Sign in")}</h1><p class="muted">${tr("Personal instance")}</p><form id="login-form" class="form"><div class="field"><label for="email">${tr("Email")}</label><input id="email" name="email" type="email" autocomplete="username" required></div><div class="field"><label for="password">${tr("Password")}</label><input id="password" name="password" type="password" autocomplete="current-password" maxlength="256" required></div><div class="error" id="login-error" ${error?"":"hidden"}>${tr(error)}</div><button class="btn primary" type="submit">${icon("log-in")}${tr("Sign in")}</button></form>${preferences()}</div></main>`;
  icons();
}
function shell() {
  document.documentElement.lang=locale;
  const current=sections.find(s=>s[0]===section());
  $("#app").innerHTML=`<div class="app"><aside class="sidebar"><a href="#dashboard" class="brand"><img src="/assets/logo.svg" alt="">BunMail <small>RS</small></a><div class="workspace-label">${icon("server")}${tr("Personal instance")}</div><nav aria-label="${tr("Mail operations")}">${sections.map(([id,name,label])=>`<a href="#${id}" class="${section()===id?"active":""}" ${section()===id?'aria-current="page"':""}>${icon(name)}<span>${tr(label)}</span></a>`).join("")}</nav><div class="sidebar-foot">${preferences()}<div class="row account-row"><span class="avatar">${icon("user-round")}</span><span class="account" title="${esc(user.email)}">${esc(user.email)}</span>${button("logout","log-out","Sign out")}</div></div></aside><button class="menu-backdrop" data-action="menu" aria-label="${tr("Close")}" hidden></button><div class="main"><header class="topbar"><div class="row"><button class="btn icon mobile-menu" data-action="menu" title="${tr("Workspace")}" aria-label="${tr("Workspace")}">${icon("menu")}</button><span class="crumb">${tr("Workspace")}${icon("chevron-right")}<strong>${tr(current[2])}</strong></span></div><span class="badge ${config.delivery_enabled?"verified":"paused"}"><span class="status-dot"></span>${tr(config.delivery_enabled?"Delivery enabled":"Delivery paused")}</span></header><main class="content" id="content"></main></div></div>`;
  icons();
}
function updateShell(){
  const status=$(".topbar > .badge");
  if(status){status.className=`badge ${config.delivery_enabled?"verified":"paused"}`;status.innerHTML=`<span class="status-dot"></span>${tr(config.delivery_enabled?"Delivery enabled":"Delivery paused")}`;}
  document.title=`${t(sections.find(s=>s[0]===section())[2])} · BunMail RS`;
}
function mailTable(rows) {
  if(!rows.length)return empty(section()==="emails"&&(filter||emailSearch)?"No matching emails":"No emails yet");
  return `<div class="table-wrap mail-table"><table><thead><tr><th>${tr("Subject")}</th><th>${tr("Recipient")}</th><th>${tr("Status")}</th><th class="attempts-column">${tr("Attempts")}</th><th class="created-column">${tr("Created")}</th><th></th></tr></thead><tbody>${rows.map(row=>`<tr data-open="${esc(row.id)}"><td class="subject"><button class="subject-link" data-action="email" data-id="${esc(row.id)}">${esc(row.subject||t("No subject"))}</button><span class="sender-line" title="${esc(row.from_address)}">${esc(row.from_address)}</span></td><td class="recipient-column">${esc(row.to_address)}</td><td>${badge(row.status)}${row.last_error?`<span class="row-error" title="${tr(row.last_error)}">${tr(row.last_error)}</span>`:""}</td><td class="attempts-column">${row.attempts}</td><td class="muted created-column">${date(row.created_at)}</td><td>${button("email","arrow-up-right","Email details",`data-id="${row.id}"`)}</td></tr>`).join("")}</tbody></table></div>`;
}
async function render({background=false}={}) {
  if(!user) return renderLogin();
  const request=++sequence;const current=section();
  if(current==="send"&&$("#send-form"))composeDraft=captureForm($("#send-form"));
  const draft=current==="send"?composeDraft:null,wasDirty=dirty;
  if(!$("#content")||renderedSection!==current||renderedLocale!==locale){shell();renderedSection=current;renderedLocale=locale;renderedHTML="";}
  if(!background)$("#content").innerHTML=`<div class="loading" role="status">${icon("loader-circle")}${tr("Loading")}</div>`;
  $("#content").setAttribute("aria-busy","true");icons();
  try {
    const nextConfig=await api("/config");
    if(request!==sequence||!user)return;
    config=nextConfig;updateShell();
    let html="";
    if(current==="dashboard") {
      const [stats,emails]=await Promise.all([api("/stats"),api("/emails")]);
      html=heading("Dashboard",`<a class="btn primary" href="#send">${icon("plus")}${tr("New email")}</a>`)+
        readiness()+
        `<div class="stats">${[["queued","clock-3","Queued"],["sending","loader-circle","Sending"],["sent","circle-check","Sent"],["failed","circle-alert","Failed"]].map(([key,name,label])=>`<div class="stat"><span class="label">${icon(name)}${tr(label)}</span><strong>${Number(stats[key]).toLocaleString(locale)}</strong></div>`).join("")}</div><section><div class="section-head"><h2>${tr("Server identity")}</h2><a class="btn link" href="#domains">${tr("Domain checks")}${icon("arrow-right")}</a></div><dl class="details"><dt>${tr("Mail hostname")}</dt><dd class="mono">${esc(config.mail_hostname)}</dd><dt>${tr("Public IP")}</dt><dd class="mono">${esc(config.public_ip||t("Not configured"))}</dd><dt>${tr("Transport")}</dt><dd>${tr("Direct to recipient MX")}</dd></dl></section><section class="section"><div class="section-head"><h2>${tr("Recent emails")}</h2><a class="btn link" href="#emails">${tr("View all")}${icon("arrow-right")}</a></div>${mailTable(emails.data.slice(0,8))}</section>`;
    } else if(current==="emails") {
      const result=await api(`/emails?${new URLSearchParams({page:String(page),...(filter?{status:filter}:{}),...(emailSearch?{search:emailSearch}:{})})}`);
      html=heading("Emails",`<a class="btn primary" href="#send">${icon("plus")}${tr("New email")}</a>`)+`<div class="toolbar mail-toolbar"><form id="email-search-form" role="search"><label class="search-field">${icon("search")}<input type="search" name="search" maxlength="200" value="${esc(emailSearch)}" placeholder="${tr("Search emails")}" aria-label="${tr("Search emails")}"></label><button class="btn icon" type="submit" title="${tr("Search")}" aria-label="${tr("Search")}">${icon("arrow-right")}</button></form><div class="row"><select class="filter" id="status-filter" aria-label="${tr("Status")}"><option value="">${tr("All statuses")}</option>${Object.entries(statuses).map(([value,label])=>`<option value="${value}" ${filter===value?"selected":""}>${tr(label)}</option>`).join("")}</select>${filter||emailSearch?button("clear-filters","filter-x","Clear filters"):""}${button("refresh","refresh-cw","Refresh")}</div></div>${mailTable(result.data)}<div class="pagination">${button("previous","chevron-left","Previous page",page===0?"disabled":"")}<span>${tr("Page")} ${page+1}</span>${button("next","chevron-right","Next page",!result.has_more?"disabled":"")}</div>`;
    } else if(current==="send") {
      await loadTemplates();
      html=heading("Send email",config.ready_to_test?testButton():"","Only one recipient per message")+(!config.ready_to_test?readiness():"")+`<div class="compose-layout"><form class="form" id="send-form"><div class="form-section"><h2>${icon("user-round")}${tr("Sender")}</h2><div class="grid2">${field("from_name","Sender name")}${field("from","From","email",true)}</div>${field("to","To","email",true)}</div><div class="form-section"><h2>${icon("mail")}${tr("Message content")}</h2><div class="field"><label for="template_id">${tr("Template")}</label><select id="template_id" name="template_id"><option value="">${tr("No template")}</option>${templates.map(row=>`<option value="${row.id}">${esc(row.name)}</option>`).join("")}</select></div><div id="manual-body" class="form">${field("subject","Subject","text",true)}<div class="body-tabs" role="group" aria-label="${tr("Message content")}">${[["html","code-2","HTML"],["text","align-left","Plain text"]].map(([value,symbol,label])=>`<button type="button" class="btn ${bodyMode===value?'selected':''}" data-action="body-mode" data-mode="${value}" aria-pressed="${bodyMode===value}">${icon(symbol)}${tr(label)}</button>`).join("")}</div><div data-body="html" ${bodyMode!=="html"?'hidden':''}>${area("html","HTML body",12)}</div><div data-body="text" ${bodyMode!=="text"?'hidden':''}>${area("text","Text body",12)}</div></div><div id="template-variables" hidden>${area("variables","Variables (JSON)",9,"{}")}</div></div><div class="form-foot"><span class="draft-status" ${dirty?'':'hidden'}>${icon("circle-small")}${tr("Draft")}</span><button class="btn" type="button" data-action="preview-compose">${icon("eye")}${tr("Preview")}</button><button class="btn primary" type="submit" ${!config.ready_to_test?"disabled":""}>${icon("send")}${tr("Send")}</button></div></form>${previewPane("compose-preview")}</div>`;
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
      html+=`<div class="row version-actions"><a class="btn" href="/api/v1/openapi.json" download="bunmail-openapi.json">${icon("download")}${tr("Download OpenAPI")}</a></div>`+versionPanel();
    }
    if(request!==sequence)return;
    if(!background||html!==renderedHTML){$("#content").innerHTML=html;renderedHTML=html;document.querySelectorAll(".recipient-column").forEach(el=>el.dataset.label=t("To"));icons();}
    $("#content").removeAttribute("aria-busy");$(".refresh-error")?.remove();updateVerifyButtons();updateCheckButton();
    if(draft){restoreForm($("#send-form"),draft);syncTemplate(false);dirty=wasDirty;}
    else if(current!=="send")dirty=false;
    if(current==="send" && chosenTemplate && templates.some(row=>row.id===chosenTemplate)){
      $("#template_id").value=chosenTemplate;syncTemplate();chosenTemplate="";
    }
  } catch(error) {
    if(request!==sequence||!$("#content"))return;
    $("#content").removeAttribute("aria-busy");
    const failure=`<div class="error refresh-error" role="alert"><span>${tr(error.message)}</span><button class="btn" data-action="refresh">${icon("refresh-cw")}${tr("Try again")}</button></div>`;
    if(background){$(".refresh-error")?.remove();$("#content").insertAdjacentHTML("afterbegin",failure);}else $("#content").innerHTML=failure;
    icons();
  }
}
function testButton(){return `<button class="btn primary" data-action="test-email">${icon("send-horizontal")}${tr("Send test email")}</button>`;}
function readiness(){
  const checks=[[!!config.public_ip,"Public IP",config.public_ip||t("Not configured"),"settings"],[!!config.verified_domains?.length,"Verified sender",config.verified_domains?.join(", ")||t("Pending"),"domains"],[config.delivery_enabled,"Delivery switch",t(config.delivery_enabled?"Enabled":"Disabled"),"settings"]];
  const canEnable=!!config.public_ip&&!!config.verified_domains?.length;
  const next=!config.public_ip?["settings","Configure public IP"]:!config.verified_domains?.length?["domains","Verify domain"]:["settings","Open settings"];
  return `<section class="readiness" aria-label="${tr("Configuration")}"><div class="section-head"><div class="row"><h2>${tr("Configuration")}</h2><span class="badge ${config.ready_to_test?"verified":"pending"}">${tr(config.ready_to_test?"Ready":"Incomplete")}</span></div>${config.ready_to_test?testButton():section()!==next[0]?`<a class="btn link" href="#${next[0]}">${tr(next[1])}${icon("arrow-up-right")}</a>`:""}</div><div class="readiness-steps">${checks.map(([ok,label,value,target])=>`<div class="readiness-step ${ok?"complete":""}">${icon(ok?"circle-check":"circle-dashed")}<div><span class="step-label">${tr(label)}</span>${label==="Delivery switch"&&section()==="settings"?`<button type="button" class="delivery-toggle" role="switch" aria-checked="${config.delivery_enabled}" aria-label="${tr("Delivery switch")}" data-action="toggle-delivery" ${!config.delivery_enabled&&!canEnable?'disabled':''} title="${tr(canEnable?"Delivery switch":"Please configure a public IP and DNS before enabling delivery.")}"><span class="switch-track" aria-hidden="true"></span><span>${esc(value)}</span></button>`:`<strong>${!ok&&section()!==target?`<a class="step-link" href="#${target}">${esc(value)}${icon("arrow-up-right")}</a>`:esc(value)}</strong>`}</div></div>`).join("")}</div>${section()==="settings"?`<p class="muted delivery-note">${tr(!canEnable&&!config.delivery_enabled?"Please configure a public IP and DNS before enabling delivery.":"Queued messages are preserved. In-flight delivery may still complete.")}</p>`:""}</section>`;
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
function field(name,label,type="text",required=false,value="") {return `<div class="field"><label for="${name}">${tr(label)}${required?`<span class="required-mark" aria-label="${tr("Required")}">*</span>`:""}</label><input id="${name}" name="${name}" type="${type}" ${required?"required":""} value="${esc(value)}" maxlength="${name==="subject"?998:254}"></div>`;}
function area(name,label,rows=6,value="") {return `<div class="field"><label for="${name}">${tr(label)}</label><textarea id="${name}" name="${name}" rows="${rows}" ${name==="html"?'class="mono"':""}>${esc(value)}</textarea></div>`;}
function modal(title,html,wide=false) {
  clearTimeout(testTimer);dialogDirty=false;const d=$("#dialog");if(d.open)d.close();d.dataset.emailId="";d.dataset.cloudflareId="";d.classList.toggle("wide",wide);d.setAttribute("aria-labelledby","dialog-title");d.innerHTML=`<div class="dialog-head"><h2 id="dialog-title">${tr(title)}</h2>${button("close","x","Close")}</div>${html}`;d.showModal();icons();$("input,textarea",d)?.focus();
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
  const submit=form.querySelector('[type="submit"]');if(form.dataset.busy||submit?.disabled)return;
  form.dataset.busy="true";if(submit)submit.disabled=true;form.setAttribute("aria-busy","true");
  $(".form-error",form)?.remove();
  const values=Object.fromEntries(new FormData(form));
  try {
    if(form.id==="email-search-form"){emailSearch=values.search.trim();page=0;await render();return;}
    if(form.id==="login-form") {user=await api("/login",{method:"POST",body:JSON.stringify(values)});config=await api("/config");await render();return;}
    if(form.id==="domain-form") {await api("/domains",{method:"POST",body:JSON.stringify(values)});$("#dialog").close();toast("Saved");await render();}
    if(form.id==="key-form") {const key=await api("/api-keys",{method:"POST",body:JSON.stringify(values)});await render();modal("Key created",`<p class="muted">${tr("This key is shown only once.")}</p><pre class="reveal" id="new-key">${esc(key.token)}</pre><div class="form-foot"><button class="btn primary" data-action="copy-key">${icon("copy")}${tr("Copy")}</button></div>`);}
    if(form.id==="template-form") {const {preview_variables,...input}=values;await api(`/templates${form.dataset.id?`/${form.dataset.id}`:""}`,{method:form.dataset.id?"PUT":"POST",body:JSON.stringify(input)});dialogDirty=false;$("#dialog").close();toast("Saved");await render();}
    if(form.id==="preview-form")showPreview({template_id:form.dataset.id,variables:values.preview_variables},"template-preview");
    if(form.id==="test-form"){
      const body=JSON.stringify({from:values.test_from,from_name:values.test_from_name,to:values.test_to,language:values.test_language});if(testBody!==body){testKey=crypto.randomUUID();testBody=body;}
      const result=await api("/emails/test",{method:"POST",headers:{"Idempotency-Key":testKey},body});
      modal("Delivery test status",`<div id="test-result">${tr("Loading")}</div><div class="form-foot">${button("refresh-test","refresh-cw","Refresh status",`data-id="${result.id}"`)}<button class="btn" data-action="email" data-id="${result.id}">${icon("arrow-up-right")}${tr("Open record")}</button></div>`);$("#dialog").dataset.emailId=result.id;await pollTest(result.id);
    }
    if(form.id==="send-form") {
      const input={from:values.from,from_name:values.from_name,to:values.to,subject:values.subject,html:values.html,text:values.text};
      if(values.template_id){input.template_id=values.template_id;input.variables=JSON.parse(values.variables);if(!input.variables||Array.isArray(input.variables)||typeof input.variables!=="object")throw new Error("Use valid JSON object for variables");}
      else if(!values.html.trim()&&!values.text.trim())throw new Error("Message body is required");
      const body=JSON.stringify(input);if(body!==sendBody){sendKey=crypto.randomUUID();sendBody=body;}
      await api("/emails/send",{method:"POST",headers:{"Idempotency-Key":sendKey},body});sendKey=crypto.randomUUID();sendBody="";dirty=false;composeDraft=null;toast("Message queued");location.hash="emails";
    }
  }catch(error){const message=error instanceof SyntaxError?"Invalid JSON":error.message;if(form.id==="login-form"&&$("#login-error")){$("#login-error").textContent=t(message);$("#login-error").hidden=false;}else if(form.isConnected)formError(form,message);}
  finally{delete form.dataset.busy;form.removeAttribute("aria-busy");if(submit)submit.disabled=false;}
});
document.addEventListener("click",async event=>{
  const themeButton=event.target.closest("button[data-theme-choice]");
  if(themeButton){
    window.bunmailTheme?.set(themeButton.dataset.themeChoice);
    document.querySelectorAll("button[data-theme-choice]").forEach(button=>{
      const selected=button.dataset.themeChoice===themeButton.dataset.themeChoice;
      button.classList.toggle("selected",selected);
      button.setAttribute("aria-pressed",String(selected));
    });
    return;
  }
  const languageButton=event.target.closest("[data-locale]");
  if(languageButton){
    if(dialogDirty&&!closeDialog())return;
    const values=[...document.querySelectorAll("form")].map(f=>[f.id,Object.fromEntries(new FormData(f))]);const wasDirty=dirty;
    locale=languageButton.dataset.locale;try{localStorage.setItem("bunmail.dashboard.locale",locale);}catch(_){}
    $("#dialog").close();await render();for(const [id,data]of values){const form=document.getElementById(id);if(form)for(const [key,value]of Object.entries(data)){const input=form.elements.namedItem(key);if(input&&"value"in input)input.value=value;}}
    if($("#template_id"))syncTemplate(false);dirty=wasDirty;return;
  }
  const nav=event.target.closest('a[href^="#"]');
  if(nav){
    if(nav.hash!==location.hash&&hasUnsavedChanges()&&!confirm(t("Changes not saved. Leave this page?"))){event.preventDefault();return;}
    if(nav.hash!==location.hash){dirty=false;dialogDirty=false;composeDraft=null;$("#dialog").close();}
    $(".sidebar")?.classList.remove("open");if($(".menu-backdrop"))$(".menu-backdrop").hidden=true;$(".mobile-menu")?.setAttribute("aria-expanded","false");
  }
  const action=event.target.closest("[data-action]");
  if(!action){const row=event.target.closest("[data-open]");if(row)try{await emailDetails(row.dataset.open);}catch(e){toast(e.message);}return;}
  const id=action.dataset.id;if(action.disabled)return;action.disabled=true;
  try {
    switch(action.dataset.action){
      case "clear-filters":emailSearch="";filter="";page=0;await render();break;
      case "body-mode":bodyMode=action.dataset.mode;document.querySelectorAll("[data-body]").forEach(el=>el.hidden=el.dataset.body!==bodyMode);document.querySelectorAll('[data-action="body-mode"]').forEach(el=>{const active=el.dataset.mode===bodyMode;el.classList.toggle("selected",active);el.setAttribute("aria-pressed",String(active));});break;
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
      case "menu":$(".sidebar").classList.toggle("open");$(".menu-backdrop").hidden=!$(".sidebar").classList.contains("open");$(".mobile-menu").setAttribute("aria-expanded",String(!$(".menu-backdrop").hidden));break;
      case "logout":if(hasUnsavedChanges()&&!confirm(t("Changes not saved. Leave this page?")))break;await api("/logout",{method:"POST"});sequence++;user=null;dirty=false;dialogDirty=false;composeDraft=null;$("#dialog").close();renderLogin();break;
      case "refresh":await render();break;
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
      case "close":closeDialog();break;
      case "previous":page=Math.max(0,page-1);await render();break;
      case "next":page++;await render();break;
      case "email":await emailDetails(id);break;
      case "retry":await api(`/emails/${id}/retry`,{method:"POST"});$("#dialog").close();toast("Retry scheduled");await render();break;
      case "cancel-email":if(confirm(t("Cancel queued email?"))){await api(`/emails/${id}/cancel`,{method:"POST"});$("#dialog").close();await render();}break;
    }
  }catch(error){if(action.dataset.action==="verify" && error.retryAfter>0)setVerifyCooldown(id,error.retryAfter);const form=action.closest("form");if(form)formError(form,error instanceof SyntaxError?"Invalid JSON":error.message);else toast(error.message);}finally{if(action.dataset.action==="verify")verifyPending.delete(id);action.disabled=false;updateVerifyButtons();cloudflareButtons();}
});
function syncTemplate(fill=true){
  const select=$("#template_id");const use=!!select.value;$("#manual-body").hidden=use;$("#template-variables").hidden=!use;$("#subject").required=!use;
  if(use&&fill){const template=templates.find(t=>t.id===select.value);$("#variables").value=JSON.stringify(sampleVariables(template),null,2);}
  if(use){try{composePreview();}catch(_){}}
}
document.addEventListener("change",event=>{
  if(event.target.id==="status-filter"){filter=event.target.value;page=0;void render();}
  if(event.target.id==="template_id"){dirty=true;$(".draft-status").hidden=false;syncTemplate();}
  if(event.target.id==="template-language"){templateLanguage=event.target.value;$("#template-list").innerHTML=templateCards();icons();}
});
document.addEventListener("input",event=>{if(event.target.closest("#send-form")){dirty=true;$(".draft-status").hidden=false;}if(event.target.closest("#template-form"))dialogDirty=true;if(event.target.id==="template-search"){templateSearch=event.target.value;$("#template-list").innerHTML=templateCards();icons();}});
document.addEventListener("input",event=>{if(["install-directory","package-directory"].includes(event.target.id))updateCommands();});
$("#dialog").addEventListener("close",()=>{clearTimeout(testTimer);dialogDirty=false;});
$("#dialog").addEventListener("cancel",event=>{event.preventDefault();closeDialog();});
window.addEventListener("beforeunload",event=>{if(hasUnsavedChanges()){event.preventDefault();event.returnValue="";}});
window.addEventListener("hashchange",()=>{
  if(hasUnsavedChanges()&&!confirm(t("Changes not saved. Leave this page?"))){history.replaceState(null,"",location.pathname+location.search+activeHash);return;}
  activeHash=location.hash;dirty=false;dialogDirty=false;composeDraft=null;$("#dialog").close();page=0;void render();
});
setInterval(()=>{if(user&&!$("#dialog").open&&!hasUnsavedChanges()&&!document.activeElement.closest("#content")&&["dashboard","emails"].includes(section())&&document.visibilityState==="visible")void render({background:true});},30000);
setInterval(updateVerifyButtons,1000);
setInterval(updateCheckButton,1000);
(async()=>{try{user=await api("/me");config=await api("/config");await render();}catch(error){renderLogin(error.status===401?"":error.message);}})();
