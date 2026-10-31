// Requires a running local instance and Playwright; all created fixtures are removed or revoked.
const {chromium}=require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const fs=require('node:fs');
const assert=require('node:assert/strict');
const values=Object.fromEntries(fs.readFileSync('.env','utf8').split('\n').filter(l=>l && !l.startsWith('#')).map(l=>{const i=l.indexOf('=');return [l.slice(0,i),l.slice(i+1)];}));
assert.equal(values.APP_ENV,'development','UI fixture tests require development mode');
assert.equal(values.DELIVERY_ENABLED,'false','UI fixture tests require paused delivery');
const stamp=Date.now();
const domain=`ui-${stamp}.test`, template=`UI fixture ${stamp}`, key=`UI fixture ${stamp}`;
(async()=>{
  fs.mkdirSync('artifacts',{recursive:true});
  const browser=await chromium.launch({headless:true});
  const page=await browser.newPage({viewport:{width:1440,height:1000}});
  const errors=[];
  page.on('pageerror',error=>errors.push(error.message));
  page.on('dialog',dialog=>dialog.accept());
  async function idle(){await page.waitForFunction(()=>!document.querySelector('.loading'));}
  async function nav(section){await page.locator(`nav a[href="#${section}"]`).click();await page.locator(`nav a[href="#${section}"][aria-current=page]`).waitFor({state:'attached'});await idle();}
  async function fit(){assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),'page overflows viewport');}
  try {
    await page.goto(values.APP_URL);
    await page.locator('#login-form').waitFor();
    assert.equal(await page.locator('html').getAttribute('lang'),'zh-CN');
    await page.screenshot({path:'artifacts/login-desktop.png'});
    await page.locator('#email').fill(values.OWNER_EMAIL);
    await page.locator('#password').fill(values.BOOTSTRAP_PASSWORD);
    await page.locator('#login-form button[type=submit]').click();
    await page.locator('nav').waitFor();await idle();
    assert.equal(await page.locator('[data-action=test-email]').count(),0);
    await page.screenshot({path:'artifacts/dashboard-zh-desktop.png'});
    await nav('domains');
    await page.locator('[data-action=add-domain]').click();
    await page.locator('#domain-form #name').fill(domain);
    await page.locator('#domain-form button[type=submit]').click();
    await page.locator('.domain h2').filter({hasText:domain}).waitFor();
    const domainRow=page.locator('.domain').filter({hasText:domain});
    await domainRow.locator('details summary').click();
    assert((await domainRow.locator('.dns-table').textContent()).includes('v=DKIM1'));
    await domainRow.locator('[data-action=verify]').click();await idle();
    await page.screenshot({path:'artifacts/domains-zh-desktop.png'});
    await nav('templates');
    await page.locator('[data-action=initialize-templates]').click();
    await page.locator('[data-template="bee00000-0000-4000-8000-000000000002"]').waitFor();
    assert.equal(await page.locator('.template-card[data-template^="bee00000"]').count(),4);
    await page.screenshot({path:'artifacts/templates-zh-desktop.png'});
    await page.locator('#template-language').selectOption('en');
    assert.equal(await page.locator('.template-card[data-template^="bee00000"]').count(),4);
    await page.locator('#template-language').selectOption('zh-CN');
    await page.locator('[data-template="bee00000-0000-4000-8000-000000000002"] [data-action=preview-template]').click();
    await page.frameLocator('#template-preview').getByRole('heading',{name:'验证你的邮箱'}).waitFor();
    await page.screenshot({path:'artifacts/verification-preview-desktop.png'});
    await page.locator('#dialog [data-action=close]').click();
    await page.locator('[data-action=add-template]').click();
    await page.locator('#template-form #name').fill(template);
    await page.locator('#template-form #subject').fill('你好 {{ name }} <script>');
    await page.locator('#template-form #html').fill('<p>Hello {{ name }}</p>');
    await page.locator('#template-form button[type=submit]').click();
    await page.locator('.template-card').filter({hasText:template}).waitFor();
    assert.equal(await page.locator('#content script').count(),0);
    await page.locator('.template-card').filter({hasText:template}).locator('[data-action=edit-template]').click();
    await page.locator('#template-form #text').fill('Hello {{ name }}');
    await page.locator('#template-form button[type=submit]').click();
    await page.waitForFunction(()=>!document.querySelector('#dialog').open);await idle();
    await nav('send');
    await page.locator('#from').fill('hello@example.com');
    await page.locator('#template_id').selectOption({label:template});
    await page.locator('#variables').fill('{"name":"中文变量"}');
    await page.locator('[data-locale=en]').click();await idle();
    assert.equal(await page.locator('#from').inputValue(),'hello@example.com');
    assert.equal(await page.locator('#variables').inputValue(),'{"name":"中文变量"}');
    assert(await page.locator('#manual-body').isHidden());
    assert(await page.locator('#send-form button[type=submit]').isDisabled());
    await page.screenshot({path:'artifacts/send-en-desktop.png'});
    await page.reload();await page.locator('nav').waitFor();await idle();
    assert.equal(await page.locator('html').getAttribute('lang'),'en');
    await nav('keys');
    await page.locator('[data-action=add-key]').click();
    await page.locator('#key-form #name').fill(key);
    await page.locator('#key-form button[type=submit]').click();
    await page.locator('#new-key').waitFor();
    assert((await page.locator('#new-key').textContent()).startsWith('bm_live_'));
    await page.locator('#dialog [data-action=close]').click();
    await page.locator('tr').filter({hasText:key}).locator('[data-action=revoke-key]').click();
    await page.locator('tr').filter({hasText:key}).getByText('Revoked',{exact:true}).waitFor();
    for(const locale of ['en','zh-CN']) {
      await page.locator(`[data-locale="${locale}"]`).click();await idle();
      for(const section of ['dashboard','send','emails','domains','templates','keys','settings']){
        await nav(section);await fit();
      }
    }
    await nav('dashboard');
    await page.setViewportSize({width:390,height:844});await fit();
    await page.screenshot({path:'artifacts/dashboard-zh-mobile.png'});
    for(const section of ['send','emails','domains','templates','keys','settings']){
      await page.locator('.mobile-menu').click();await nav(section);await fit();
    }
    await page.locator('.mobile-menu').click();
    await page.locator('[data-locale=en]').click();await idle();await fit();
    await page.screenshot({path:'artifacts/settings-en-mobile.png'});
    assert.equal(await page.locator('[data-action=test-email]').count(),0);
    await page.locator('.mobile-menu').click();await nav('templates');
    await page.screenshot({path:'artifacts/templates-en-mobile.png'});
    await page.setViewportSize({width:320,height:740});await fit();
    await page.setViewportSize({width:1440,height:1000});
    await nav('templates');
    await page.locator('.template-card').filter({hasText:template}).locator('[data-action=delete-template]').click();await idle();
    await nav('domains');
    await domainRow.locator('[data-action=delete-domain]').click();await idle();
    await page.locator('[data-locale=zh-CN]').click();await idle();
    await page.locator('[data-action=logout]').click();await page.locator('#login-form').waitFor();
    await page.reload();await page.locator('#login-form').waitFor();
    await page.setViewportSize({width:390,height:844});await fit();
    await page.screenshot({path:'artifacts/login-zh-mobile.png'});
    assert.deepEqual(errors,[]);
    console.log('PASS: real API login, domain/template/key lifecycle, locale persistence, form preservation, seven views, mobile layout, logout');
  } finally {
    for(const [path,name]of [["templates",template],["domains",domain],["api-keys",key]]){
      try{const response=await page.request.get(`${values.APP_URL}/api/v1/${path}`);if(response.ok())for(const item of (await response.json()).data){const row=item.domain||item;if(row.name===name)await page.request.delete(`${values.APP_URL}/api/v1/${path}/${row.id}`,{headers:{Origin:values.APP_URL}});}}catch(_){console.error('Fixture cleanup could not connect');}
    }
    await browser.close();
  }
})().catch(error=>{console.error(error.message);process.exitCode=1;});
