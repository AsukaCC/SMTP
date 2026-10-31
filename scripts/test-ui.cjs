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
    const verifyButton=domainRow.locator('[data-action=verify]');
    const domainId=await verifyButton.getAttribute('data-id');
    for(let attempt=0;attempt<3;attempt++){
      const response=page.waitForResponse(response=>response.url().endsWith(`/domains/${domainId}/verify`)&&response.request().method()==='POST');
      await verifyButton.click();
      assert.equal((await response).status(),200);
      if(attempt<2)await page.waitForFunction(id=>{const button=document.querySelector(`[data-action="verify"][data-id="${id}"]`);return button&&!button.disabled;},domainId);
    }
    await page.waitForFunction(id=>document.querySelector(`[data-action="verify"][data-id="${id}"]`)?.textContent.includes('秒后'),domainId);
    assert(await verifyButton.isDisabled());
    await page.reload();await page.locator('nav').waitFor();await idle();
    assert(await verifyButton.isDisabled());
    await page.locator('[data-locale=en]').click();await idle();
    assert.match(await verifyButton.textContent(),/Check again in \d+s/);
    await page.setViewportSize({width:390,height:844});await fit();
    await page.screenshot({path:'artifacts/domains-en-mobile.png'});
    await page.setViewportSize({width:1440,height:1000});
    await page.locator('[data-locale=zh-CN]').click();await idle();
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
    await page.locator('#from_name').fill('中文发件人');
    await page.locator('#template_id').selectOption({label:template});
    await page.locator('#variables').fill('{"name":"中文变量"}');
    await page.locator('[data-locale=en]').click();await idle();
    assert.equal(await page.locator('#from').inputValue(),'hello@example.com');
    assert.equal(await page.locator('#from_name').inputValue(),'中文发件人');
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
    await nav('settings');
    await page.locator('[data-action=check-updates]').waitFor();
    await page.locator('[data-action=update-commands]').click();
    await page.locator('#install-directory').fill("/srv/mail's instance");
    assert((await page.locator('#update-command').textContent()).includes("'/srv/mail'\\''s instance'"));
    await page.locator('#package-directory').fill('relative-directory');
    assert(await page.locator('[data-action=copy-update-command]').isDisabled());
    await page.locator('#package-directory').fill('/tmp/bunmail-packages/0.1.1');
    assert(!(await page.locator('[data-action=copy-update-command]').isDisabled()));
    await page.locator('#install-directory').fill('');
    assert(await page.locator('[data-action=copy-update-command]').isDisabled());
    await page.locator('#dialog [data-action=close]').click();
    // Fixture responses exercise update states independently of GitHub availability.
    let updateFixture={current_version:'0.1.0',repository:'AsukaCC/SMTP',releases_url:'https://github.com/AsukaCC/SMTP/releases',status:'not_checked',check:null,retry_after_seconds:0};
    await page.route('**/api/v1/updates',route=>route.fulfill({json:updateFixture}));
    await page.route('**/api/v1/updates/check',route=>{updateFixture={...updateFixture,status:'update_available',check:{latest_version:'0.1.1',checked_at:new Date().toISOString(),release_url:'https://github.com/AsukaCC/SMTP/releases/tag/v0.1.1'},retry_after_seconds:60};return route.fulfill({json:updateFixture});});
    await page.reload();await page.locator('nav').waitFor();await idle();
    await page.locator('[data-action=check-updates]').click();
    await page.locator('.update-status').filter({hasText:'Update available'}).waitFor();
    assert(await page.locator('[data-action=check-updates]').isDisabled());
    await page.reload();await page.locator('nav').waitFor();await idle();
    assert(await page.locator('[data-action=check-updates]').isDisabled());
    await page.screenshot({path:'artifacts/updates-en-desktop.png'});
    await page.locator('[data-locale=zh-CN]').click();await idle();
    assert.equal(await page.locator('.update-status').textContent(),'有新版本');
    await page.setViewportSize({width:390,height:844});await fit();
    await page.locator('.version-section').scrollIntoViewIfNeeded();
    await page.screenshot({path:'artifacts/updates-zh-mobile.png'});
    await page.locator('[data-action=update-commands]').click();await fit();
    await page.screenshot({path:'artifacts/update-commands-zh-mobile.png'});
    await page.locator('#dialog [data-action=close]').click();
    updateFixture={...updateFixture,status:'check_failed',check:null,retry_after_seconds:0};
    await page.reload();await page.locator('nav').waitFor({state:'attached'});await idle();
    assert((await page.locator('.update-status').textContent()).includes('检查失败'));
    await page.unroute('**/api/v1/updates');await page.unroute('**/api/v1/updates/check');
    await page.setViewportSize({width:1440,height:1000});
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
