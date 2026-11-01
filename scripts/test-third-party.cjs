// Run only through the isolated integration fixture, which has no outbound worker.
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const {spawn}=require('node:child_process');
const {createInterface}=require('node:readline');
const {once}=require('node:events');
const {randomUUID}=require('node:crypto');
const fs=require('node:fs');
const assert=require('node:assert/strict');
(async()=>{
  const {BunMailClient}=await import('../examples/client.mjs');
  const baseUrl=process.env.TEST_API_URL,apiKey=process.env.TEST_API_KEY;
  const client=new BunMailClient({baseUrl,apiKey});
  assert.equal((await client.check()).data.scope,'send');
  const body={from:'hello@example.test',to:'recipient@example.net',subject:'Third-party server fixture',text:'No public delivery worker'};
  const key=randomUUID(),first=await client.send(body,key),second=await client.send(body,key);
  assert.equal(first.status,202);assert.equal(second.status,200);assert.equal(first.data.id,second.data.id);
  await assert.rejects(client.send({...body,subject:'changed'},key),error=>error.code==='IDEMPOTENCY_CONFLICT'&&!!error.requestId);
  assert.equal((await client.get(first.data.id)).data.email.status,'queued');
  await assert.rejects(new BunMailClient({baseUrl,apiKey:'invalid'}).check(),error=>error.status===401&&!!error.requestId);
  const website=spawn(process.execPath,['examples/website/server.mjs'],{env:{...process.env,BUNMAIL_API_URL:baseUrl,BUNMAIL_API_KEY:apiKey,MAIL_FROM:body.from,MAIL_TO:body.to,PORT:'0'},stdio:['ignore','pipe','inherit']});
  let browser;
  try{
    const lines=createInterface({input:website.stdout});
    const [line]=await once(lines,'line',{signal:AbortSignal.timeout(10000)});
    const websiteUrl=JSON.parse(line).url;
    browser=await chromium.launch({headless:true});
    const page=await browser.newPage({viewport:{width:1280,height:900}});
    const errors=[];page.on('pageerror',error=>errors.push(error.message));
    page.on('request',request=>{if(request.url().startsWith(websiteUrl))assert(!request.headers().authorization,'website frontend must not receive the API key');});
    await page.goto(websiteUrl);await page.getByText('连接正常',{exact:true}).waitFor();
    await page.locator('[name=subject]').fill('Third-party website fixture');
    await page.locator('[name=text]').fill('网站后端通过 Bearer API 发信');
    await page.locator('#send-form button').click();
    await page.waitForFunction(()=>document.querySelector('#result-text').textContent.includes('"created": true'));
    const result=JSON.parse(await page.locator('#result-text').textContent());
    assert.equal(result.status,202);
    await page.locator('#send-form button').click();
    await page.waitForFunction(()=>document.querySelector('#result-text').textContent.includes('"created": false'));
    assert.equal(JSON.parse(await page.locator('#result-text').textContent()).data.id,result.data.id);
    await page.locator('#refresh').click();
    await page.waitForFunction(()=>document.querySelector('#result-text').textContent.includes('"status": "queued"'));
    assert(!(await page.content()).includes(apiKey));
    fs.mkdirSync('artifacts',{recursive:true});
    await page.screenshot({path:'artifacts/third-party-website-desktop.png',fullPage:true});
    await page.setViewportSize({width:390,height:844});
    assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
    await page.screenshot({path:'artifacts/third-party-website-mobile.png',fullPage:true});
    await page.goto(process.env.TEST_ORIGIN_URL);
    const cross=await page.evaluate(async({baseUrl,apiKey,body,key})=>{
      const response=await fetch(baseUrl+'/emails/send',{method:'POST',credentials:'omit',headers:{Authorization:'Bearer '+apiKey,'Content-Type':'application/json','Idempotency-Key':key},body:JSON.stringify(body)});
      return {status:response.status,data:await response.json(),requestId:response.headers.get('x-request-id')};
    },{baseUrl,apiKey,body:{...body,subject:'Allowed browser CORS fixture'},key:randomUUID()});
    assert.equal(cross.status,202);assert(cross.requestId);
    const invalid=await page.evaluate(async({baseUrl,apiKey})=>{const r=await fetch(baseUrl+'/emails/send',{method:'POST',headers:{Authorization:'Bearer '+apiKey,'Content-Type':'application/json'},body:'{'});return {status:r.status,body:await r.json(),requestId:r.headers.get('x-request-id')};},{baseUrl,apiKey});
    assert.equal(invalid.status,400);assert.equal(invalid.body.error.code,'BAD_REQUEST');assert(invalid.requestId);
    const anonymous=await page.evaluate(async baseUrl=>{const r=await fetch(baseUrl+'/auth/check',{credentials:'omit'});return r.status;},baseUrl);
    assert.equal(anonymous,401);
    const forbiddenHeader=await page.evaluate(async({baseUrl,apiKey,body})=>{try{await fetch(baseUrl+'/emails/send',{method:'POST',headers:{Authorization:'Bearer '+apiKey,'Content-Type':'application/json','X-Not-Allowed':'1'},body:JSON.stringify(body)});return false;}catch{return true;}},{baseUrl,apiKey,body});
    assert(forbiddenHeader);
    const adminBlocked=await page.evaluate(async({baseUrl,apiKey})=>{try{await fetch(baseUrl+'/config',{headers:{Authorization:'Bearer '+apiKey}});return false;}catch{return true;}},{baseUrl,apiKey});
    assert(adminBlocked);
    // Same fixture under a different host: no CSP, but this origin is not allowed.
    await page.goto(process.env.TEST_ORIGIN_URL.replace('127.0.0.1','localhost'));
    const blocked=await page.evaluate(async({baseUrl,apiKey,body})=>{try{await fetch(baseUrl+'/emails/send',{method:'POST',headers:{Authorization:'Bearer '+apiKey,'Content-Type':'application/json'},body:JSON.stringify({...body,subject:'Blocked browser CORS fixture'})});return false;}catch{return true;}},{baseUrl,apiKey,body});
    assert(blocked);
    assert.equal((await client.list({search:'Blocked browser CORS fixture'})).data.data.length,0);
    assert.deepEqual(errors,[]);
    console.log('PASS: website backend proxy, server SDK, authentication, same-key retry, message status, browser CORS preflight/errors, denied origins/headers/admin routes; no public delivery');
  }finally{
    if(browser)await browser.close();
    const exited=once(website,'exit');website.kill('SIGTERM');await exited;
  }
})().catch(error=>{console.error(error.message);process.exitCode=1;});
