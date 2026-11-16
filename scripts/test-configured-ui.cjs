// Called only by the isolated Rust integration test. Its server has no outbound worker.
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
(async()=>{
  const browser=await chromium.launch({headless:true});
  const page=await browser.newPage({viewport:{width:1440,height:1000}});
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.addInitScript(()=>{
    window.dashboardPolicyViolations=[];
    document.addEventListener('securitypolicyviolation',event=>window.dashboardPolicyViolations.push(event.violatedDirective));
  });
  page.on('dialog',dialog=>dialog.accept());
  try{
    await page.goto(process.env.TEST_UI_URL);
    fs.mkdirSync('artifacts',{recursive:true});
    for(const theme of ['light','dark']) {
      await page.locator(`button[data-theme-choice=${theme}]`).click();
      for(const locale of ['zh-CN','en']) {
        await page.locator(`[data-locale=${locale}]`).click();
        await page.locator(`#login-form ~ .prefs [data-locale=${locale}].selected`).waitFor();
        assert.equal(await page.locator('[data-action=login-mode], #register-form, #verify-form, #forgot-form, #reset-form, .auth-links').count(),0);
        await page.locator('#password').fill('visibility-fixture');
        const toggle=page.locator('[aria-controls=password]');
        await page.locator('#password').evaluate(input=>{input.focus();input.setSelectionRange(2,5);});
        await toggle.click();
        assert.deepEqual(await page.locator('#password').evaluate(input=>[input.selectionStart,input.selectionEnd,document.activeElement===input]),[2,5,true]);
        assert.equal(await page.locator('#password').getAttribute('type'),'text');
        assert.equal(await toggle.getAttribute('aria-pressed'),'true');
        await toggle.click();
        assert.equal(await page.locator('#password').getAttribute('type'),'password');
        assert.equal(await page.locator('#password').inputValue(),'visibility-fixture');
        await page.locator('#password').dispatchEvent('keydown',{key:'A',modifierCapsLock:true});
        assert.equal(await page.locator('#login-form .caps-warning').isVisible(),true);
        await page.locator('#password').dispatchEvent('keyup',{key:'a',modifierCapsLock:false});
        assert.equal(await page.locator('#login-form .caps-warning').isVisible(),false);
        await page.locator('#password').fill('');
        await page.locator('.login-inner .glass__warp').waitFor({state:'attached'});
        await page.setViewportSize({width:390,height:844});
        assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
        await page.screenshot({path:`artifacts/glass-login-${theme}-${locale}.png`});
      }
    }
    await page.goto(process.env.TEST_UI_URL+'/dashboard/login?reset='+'a'.repeat(64));
    await page.locator('#login-form').waitFor();
    assert.equal(await page.locator('#reset-form, #forgot-form').count(),0);
    await page.locator('[data-locale=zh-CN]').click();
    await page.locator('[data-locale=zh-CN].selected').waitFor();
    await page.locator('button[data-theme-choice=system]').click();
    await page.setViewportSize({width:1440,height:1000});
    await page.emulateMedia({colorScheme:'dark'});
    await page.waitForFunction(()=>document.documentElement.dataset.theme==='dark');
    await page.locator('button[data-theme-choice=light]').click();
    assert.equal(await page.locator('html').getAttribute('data-theme'),'light');
    await page.screenshot({path:'artifacts/liquid-login-desktop.png'});
    await page.reload();await page.locator('#login-form').waitFor();
    assert.equal(await page.locator('html').getAttribute('data-theme'),'light');
    await page.locator('button[data-theme-choice=dark]').click();
    assert.equal(await page.locator('html').getAttribute('data-theme'),'dark');
    await page.locator('button[data-theme-choice=system]').click();
    await page.emulateMedia({colorScheme:'light'});
    await page.waitForFunction(()=>document.documentElement.dataset.theme==='light');
    assert.equal(await page.locator('html').getAttribute('aria-pressed'),null);
    await page.locator('#email').fill('owner@example.test');
    await page.locator('#password').fill('correct-horse-battery-staple-123');
    await page.locator('#login-form button[type=submit]').click();
    await page.locator('nav a[href="#settings"]').click();
    await page.getByRole('heading',{name:'服务设置',exact:true}).waitFor();
    assert.equal(await page.evaluate(()=>window.bunmailGlass.version),'1.1.1');
    await page.locator('.topbar .glass__warp').waitFor({state:'attached'});
    assert.notEqual(await page.locator('.topbar .glass__warp').evaluate(el=>getComputedStyle(el).filter),'none');
    assert.equal(await page.locator('[data-action=toggle-registration]').count(),0);
    const packageFixture={name:'bunmail-rs-0.1.1-linux-amd64.tar.gz',platform:'linux/amd64',size:123456};
    let updateFixture={current_version:'0.1.0',repository:'AsukaCC/SMTP',releases_url:'https://github.com/AsukaCC/SMTP/releases',status:'update_available',retry_after_seconds:0,check:{latest_version:'0.1.1',checked_at:new Date().toISOString()},installer:{status:'ready',repository:'AsukaCC/SMTP',platform:'linux/amd64',current_version:'0.1.0',job:null},can_install:true,package:packageFixture};
    let updateCalls=0;
    await page.route('**/api/v1/updates',route=>route.fulfill({json:updateFixture}));
    await page.route('**/api/v1/updates/install',route=>{
      const body=route.request().postDataJSON();assert.equal(body.version,'0.1.1');assert.match(body.request_id,/^[0-9a-f-]{36}$/);
      updateCalls++;updateFixture={...updateFixture,can_install:false,installer:{...updateFixture.installer,job:{...body,status:'downloading',error:null,updated_at:new Date().toISOString()}}};
      return route.fulfill({status:202,json:updateFixture.installer.job});
    });
    await page.evaluate(()=>render());await page.locator('[data-action=install-update]').click();
    await page.getByText('0.1.1 · 正在下载构建包',{exact:true}).waitFor();
    assert.equal(updateCalls,1);assert(await page.locator('[data-action=install-update]').isDisabled());
    for(const locale of ['zh-CN','en']){
      await page.locator(`[data-locale=${locale}]`).click();
      for(const width of [1440,390,320]){
        await page.setViewportSize({width,height:844});
        await page.locator('.version-section').scrollIntoViewIfNeeded();
        assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
        await page.screenshot({path:`artifacts/update-progress-${locale}-${width}.png`});
      }
      await page.setViewportSize({width:1440,height:1000});
    }
    updateFixture.installer.job.status='rolled_back';updateFixture.installer.job.error='update_failed';
    await page.evaluate(()=>pollUpdateStatus());await page.getByText(/previous version restored/).waitFor();
    updateFixture.installer.job=null;updateFixture.package=null;
    await page.evaluate(()=>pollUpdateStatus());assert(await page.locator('[data-action=install-update]').isDisabled());
    await page.getByText('No compatible build package',{exact:true}).waitFor();
    await page.unroute('**/api/v1/updates');await page.unroute('**/api/v1/updates/install');
    await page.locator('[data-locale=zh-CN]').click();await page.locator('[data-action=toggle-delivery]').waitFor();
    const delivery=page.locator('[data-action=toggle-delivery]');
    await delivery.waitFor();
    assert.equal(await delivery.getAttribute('aria-checked'),'false');
    assert.equal(await page.locator('[data-action=test-email]').count(),0);
    assert(!(await delivery.isDisabled()));
    await delivery.click();
    await page.locator('[data-action=test-email]').waitFor();
    assert.equal(await delivery.getAttribute('aria-checked'),'true');
    assert((await page.locator('.topbar').textContent()).includes('投递已启用'));
    await page.reload();await page.locator('[data-action=test-email]').waitFor();
    assert.equal(new URL(page.url()).hash,'#settings','refresh preserves the current management page');
    assert.equal(await delivery.getAttribute('aria-checked'),'true');
    assert((await page.locator('#content .notice.warning').textContent()).includes('仍可发送'));
    fs.mkdirSync('artifacts',{recursive:true});
    await page.screenshot({path:'artifacts/settings-configured-fixture.png'});
    for(const locale of ['zh-CN','en']){
      await page.locator(`[data-locale=${locale}]`).click();
      await page.locator('[data-action=test-email]').waitFor();
      await page.setViewportSize({width:390,height:844});
      assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
      await page.screenshot({path:`artifacts/delivery-switch-${locale}-mobile.png`});
      await delivery.click();
      await page.waitForFunction(()=>document.querySelector('[data-action=toggle-delivery]')?.getAttribute('aria-checked')==='false');
      assert.equal(await page.locator('[data-action=test-email]').count(),0);
      await page.reload();await delivery.waitFor();
      assert.equal(await delivery.getAttribute('aria-checked'),'false');
      await delivery.click();await page.locator('[data-action=test-email]').waitFor();
      await page.setViewportSize({width:1440,height:1000});
      await page.screenshot({path:`artifacts/delivery-switch-${locale}-desktop.png`});
    }
    await page.locator('[data-locale=zh-CN]').click();
    await page.locator('[data-action=test-email]').waitFor();
    await page.locator('[data-action=test-email]').click();
    await page.locator('#test_from_name').fill('中文测试发件人');
    await page.locator('#test_to').fill('recipient@example.net');
    await page.locator('#test_language').selectOption('en');
    await page.locator('#test-form button[type=submit]').click();
    await page.locator('#test-result .badge.queued').waitFor();
    assert((await page.locator('#test-result').textContent()).includes('BunMail RS email delivery test'));
    assert((await page.locator('#test-result').textContent()).includes('中文测试发件人'));
    await page.screenshot({path:'artifacts/test-send-queued-fixture.png'});
    await page.locator('#dialog .dialog-head [data-action=close]').click();
    await page.locator('nav a[href="#send"]').click();
    await page.locator('#from_name').fill('自用通知');
    await page.locator('#from').fill('hello@example.test');
    await page.locator('#to').fill('recipient@example.net');
    await page.locator('#subject').fill('Sender UI fixture');
    await page.locator('[data-action=body-mode][data-mode=text]').click();
    await page.locator('#text').fill('Sender display name fixture');
    await page.locator('[data-locale=en]').click();
    await page.locator('#text').waitFor();
    assert.equal(await page.locator('#text').inputValue(),'Sender display name fixture');
    assert.equal(await page.locator('#from_name').inputValue(),'自用通知');
    await page.locator('[data-action=body-mode][data-mode=html]').click();
    await page.locator('#html').fill('<p>HTML draft</p>');
    await page.locator('[data-action=body-mode][data-mode=text]').click();
    assert.equal(await page.locator('#text').inputValue(),'Sender display name fixture');
    await page.locator('[data-locale=zh-CN]').click();await page.locator('#text').waitFor();
    await page.evaluate(()=>render());
    assert.equal(await page.locator('#text').inputValue(),'Sender display name fixture');
    assert.equal(await page.locator('#html').inputValue(),'<p>HTML draft</p>');
    // Browser back/hash navigation must not discard the composition when the user cancels.
    page.removeAllListeners('dialog');page.once('dialog',dialog=>dialog.dismiss());
    await page.evaluate(()=>{location.hash='domains';});
    await page.waitForFunction(()=>location.hash==='#send');
    assert.equal(await page.locator('#html').inputValue(),'<p>HTML draft</p>');
    page.on('dialog',dialog=>dialog.accept());
    assert(!(await page.locator('#send-form button[type=submit]').isDisabled()));
    await page.screenshot({path:'artifacts/sender-rdns-warning-desktop.png'});
    await page.route('**/api/v1/emails/send',route=>route.fulfill({status:503,json:{error:{message:'Connection failed'}}}));
    await page.locator('#send-form button[type=submit]').click();
    await page.locator('#send-form .form-error').waitFor();
    assert.equal(await page.locator('#send-form .form-error').textContent(),'无法连接服务');
    assert.equal(await page.locator('#text').inputValue(),'Sender display name fixture');
    await page.screenshot({path:'artifacts/compose-inline-error-zh.png'});
    await page.unroute('**/api/v1/emails/send');
    await page.locator('#send-form button[type=submit]').click();
    await page.locator('#email-search-form input').fill('Sender UI fixture');
    await page.locator('#email-search-form button').click();
    await page.waitForFunction(()=>document.querySelectorAll('tr[data-open]').length===1);
    await page.screenshot({path:'artifacts/email-search-zh-desktop.png'});
    await page.locator('tr[data-open]').filter({hasText:'Sender UI fixture'}).click();
    await page.locator('#dialog[open]').waitFor();
    assert((await page.locator('#dialog').textContent()).includes('自用通知'));
    await page.locator('#dialog .dialog-head [data-action=close]').click();
    await page.locator('#email-search-form input').fill('no-matching-message');
    await page.locator('#email-search-form button').click();
    await page.getByText('没有符合条件的邮件',{exact:true}).waitFor();
    await page.locator('[data-action=clear-filters]').click();
    await page.locator('tr[data-open]').first().waitFor();
    const stableRow=await page.locator('tr[data-open]').first().elementHandle();
    await page.evaluate(()=>render({background:true}));
    assert(await stableRow.evaluate(row=>row.isConnected),'unchanged background refresh must preserve the rendered list');
    await page.locator('nav a[href="#domains"]').click();
    await page.locator('.domain .badge.verified').waitFor();
    await page.locator('[data-action=cloudflare]').first().click();
    await page.locator('#cloudflare-content .error').waitFor();
    assert(await page.locator('[data-action=cloudflare-apply]').isDisabled());
    await page.locator('#dialog .dialog-head [data-action=close]').click();
    // Only the provider endpoints/config flag are mocked; no Cloudflare account is modified.
    const domains=await (await page.request.get(process.env.TEST_UI_URL+'/api/v1/domains')).json();
    const recordFixture=domains.data[0].records.filter(r=>r.kind!=='PTR').map(r=>({...r,existing:[],zone_id:'a'.repeat(32),action:'create'}));
    let mode='conflict',applied=0;
    await page.route('**/api/v1/config',async route=>{const response=await route.fetch();await route.fulfill({response,json:{...await response.json(),cloudflare_configured:true}});});
    await page.route('**/cloudflare/preview',route=>route.fulfill({json:{fingerprint:'a'.repeat(64),can_apply:mode!=='conflict',records:recordFixture.map((r,i)=>i===0&&mode==='conflict'?{...r,action:'conflict',existing:[{type:'TXT',content:'v=spf1 include:other.example -all',proxied:false}]}:i===0&&mode==='partial'?{...r,action:'unchanged'}:r)}}));
    await page.route('**/cloudflare/apply',route=>{assert.equal(route.request().postDataJSON().fingerprint,'a'.repeat(64));applied++;mode='partial';return route.fulfill({json:{complete:applied>1,records:recordFixture.map((r,i)=>({kind:r.kind,name:r.name,status:applied>1?(i===0?'unchanged':'created'):i===0?'created':i===1?'failed':'skipped'}))}});});
    await page.reload();await page.locator('[data-action=cloudflare]').first().click();
    await page.locator('.dns-change .badge.failed').waitFor();
    assert(await page.locator('[data-action=cloudflare-apply]').isDisabled());assert.equal(applied,0);
    await page.setViewportSize({width:390,height:844});
    assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
    await page.screenshot({path:'artifacts/cloudflare-conflict-zh-mobile.png'});
    mode='create';await page.locator('[data-action=cloudflare-preview]').click();
    await page.waitForFunction(()=>!document.querySelector('[data-action=cloudflare-apply]').disabled);
    await page.locator('[data-action=cloudflare-apply]').click();
    await page.locator('#cloudflare-content .error').waitFor();
    assert((await page.locator('#cloudflare-content').textContent()).includes('部分记录'));
    assert(await page.locator('[data-action=cloudflare-apply]').isDisabled());
    await page.locator('[data-action=cloudflare-preview]').click();
    await page.locator('.dns-change .badge.verified').waitFor();
    await page.locator('[data-action=cloudflare-apply]').click();
    await page.locator('#cloudflare-content .notice').waitFor();
    assert.equal(applied,2);
    await page.locator('#dialog .dialog-head [data-action=close]').click();
    await page.setViewportSize({width:1440,height:1000});
    await page.locator('[data-locale=en]').click();
    await page.locator('[data-action=cloudflare]').first().click();
    await page.locator('.dns-change').first().waitFor();
    await page.screenshot({path:'artifacts/cloudflare-plan-en-desktop.png'});
    await page.locator('#dialog .dialog-head [data-action=close]').click();
    await page.unroute('**/api/v1/config');await page.unroute('**/cloudflare/preview');await page.unroute('**/cloudflare/apply');
    await page.locator('[data-locale=zh-CN]').click();
    await page.locator('nav a[href="#templates"]').click();
    await page.getByRole('heading',{name:'邮件模板',exact:true}).waitFor();
    await page.locator('[data-template="bee00000-0000-4000-8000-000000000002"] [data-action=preview-template]').click();
    await page.frameLocator('#template-preview').getByRole('heading',{name:'验证你的邮箱'}).waitFor();
    assert.equal(await page.frameLocator('#template-preview').locator('body').evaluate(body=>getComputedStyle(body).backgroundColor),'rgb(243, 245, 246)');
    await page.screenshot({path:'artifacts/template-preview-fixture.png'});
    await page.locator('#dialog .dialog-head [data-action=close]').click();
    // Render active HTML through the real form POST preview; sandbox and CSP must block it.
    await page.locator('[data-action=add-template]').click();
    await page.locator('#name').fill('Unsaved template');
    page.removeAllListeners('dialog');page.once('dialog',dialog=>dialog.dismiss());
    await page.keyboard.press('Escape');
    assert(await page.locator('#dialog').evaluate(dialog=>dialog.open));
    assert.equal(await page.locator('#name').inputValue(),'Unsaved template');
    page.on('dialog',dialog=>dialog.accept());
    await page.keyboard.press('Escape');
    await page.waitForFunction(()=>!document.querySelector('#dialog').open);
    await page.locator('[data-action=add-template]').click();
    await page.locator('#html').fill('<script>window.previewUnsafe=true;parent.previewUnsafe=true</script><p style="color:rgb(10,20,30)">Safe preview</p><img src="https://tracking.invalid/pixel">');
    const blockedRequest=page.waitForEvent('requestfailed',{predicate:request=>request.url().includes('tracking.invalid'),timeout:5000});
    await page.locator('[data-action=preview-editor]').click();
    const preview=page.frameLocator('#editor-preview');
    await preview.getByText('Safe preview').waitFor();
    assert.equal(await preview.locator('p').evaluate(p=>getComputedStyle(p).color),'rgb(10, 20, 30)');
    assert.equal(await preview.locator('body').evaluate(()=>window.previewUnsafe),undefined);
    assert.equal(await page.evaluate(()=>window.previewUnsafe),undefined);
    assert.match((await blockedRequest).failure().errorText,/^(csp|net::ERR_BLOCKED_BY_CSP)$/i);
    await page.locator('#dialog .dialog-head [data-action=close]').click();
    // A failed list request has a working recovery action.
    await page.route('**/api/v1/emails?*',route=>route.fulfill({status:503,json:{error:{message:'Connection failed'}}}));
    await page.locator('nav a[href="#emails"]').click();
    await page.locator('.refresh-error').waitFor();
    await page.unroute('**/api/v1/emails?*');
    await page.locator('.refresh-error [data-action=refresh]').click();
    await page.locator('tr[data-open]').first().waitFor();
    // Expired required checks are distinguished from missing PTR.
    await page.route('**/api/v1/domains',async route=>{const response=await route.fetch();const data=await response.json();data.data[0].domain.verified=false;data.data[0].domain.checked_at=new Date(Date.now()-25*3600000).toISOString();await route.fulfill({response,json:data});});
    await page.locator('nav a[href="#domains"]').click();
    await page.getByText('验证已过期',{exact:true}).waitFor();
    await page.screenshot({path:'artifacts/domain-expired-zh-desktop.png'});
    await page.unroute('**/api/v1/domains');
    for(const theme of ['dark','light']){
      await page.locator(`button[data-theme-choice=${theme}]`).click();
      for(const locale of ['en','zh-CN']){
      await page.locator(`[data-locale=${locale}]`).click();await page.waitForFunction(()=>!document.querySelector('.loading')&&document.querySelector('#content')?.getAttribute('aria-busy')!=='true');
      for(const viewport of [{width:1440,height:1000},{width:390,height:844},{width:320,height:740}]){
        await page.setViewportSize(viewport);
        for(const section of ['dashboard','send','emails','domains','templates','keys','settings']){
          await page.evaluate(section=>{location.hash=section;},section);
          await page.waitForFunction(section=>document.querySelector('nav a[aria-current=page]')?.getAttribute('href')==='#'+section&&!document.querySelector('.loading')&&document.querySelector('#content')?.getAttribute('aria-busy')!=='true',section);
          assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),`${locale} ${section} overflows ${viewport.width}`);
          if(viewport.width<=390)assert(await page.locator('.mobile-menu').evaluate(el=>{const a=el.getBoundingClientRect(),b=el.closest('.topbar').getBoundingClientRect();return a.top>=b.top&&a.bottom<=b.bottom;}),'mobile navigation must fit inside the top bar');
          if(viewport.width<=390&&section==='emails')assert(await page.locator('.mail-table').evaluate(el=>el.scrollWidth<=el.clientWidth+1),'mobile mail list must not clip status/actions');
          await page.screenshot({path:`artifacts/ui-${section}-${locale}-${viewport.width}-${theme}.png`});
        }
      }
      await page.setViewportSize({width:1440,height:1000});
      }
    }
    await page.setViewportSize({width:390,height:844});
    await page.locator('.mobile-menu').click();
    await page.locator('.sidebar.open').waitFor();
    assert(await page.locator('.sidebar').evaluate(el=>{const r=el.getBoundingClientRect();return r.left>=0&&r.right<=innerWidth&&r.bottom<=innerHeight;}));
    await page.screenshot({path:'artifacts/glass-navigation-mobile.png'});
    await page.locator('nav a[href="#dashboard"]').click();
    await page.waitForFunction(()=>!document.querySelector('.sidebar.open')&&document.querySelector('nav a[aria-current=page]')?.getAttribute('href')==='#dashboard'&&document.querySelector('#content')?.getAttribute('aria-busy')!=='true'&&!document.querySelector('.loading'));
    await page.emulateMedia({reducedMotion:'reduce'});
    assert.equal(await page.locator('.mobile-menu').evaluate(el=>getComputedStyle(el).transitionDuration),'0s');
    await page.emulateMedia({reducedMotion:'no-preference',contrast:'more'});
    assert.equal(await page.locator('.topbar').evaluate(el=>getComputedStyle(el).backdropFilter),'none');
    await page.emulateMedia({contrast:'no-preference'});
    await page.setViewportSize({width:1440,height:1000});
    // Authentication loss closes privileged modals and cannot be overwritten by stale rendering.
    await page.locator('nav a[href="#templates"]').click();await page.locator('[data-action=add-template]').click();
    await page.route('**/api/v1/config',route=>route.fulfill({status:401,json:{error:{message:'Please sign in or provide a valid API key'}}}));
    await page.evaluate(()=>render());
    await page.locator('#login-form').waitFor();
    assert(!(await page.locator('#dialog').evaluate(dialog=>dialog.open)));
    assert((await page.locator('#login-error').textContent()).includes('登录已过期'));
    await page.unroute('**/api/v1/config');
    // Exercise the actual password endpoint and verify another device is signed out.
    await page.locator('#email').fill('owner@example.test');
    await page.locator('#password').fill('correct-horse-battery-staple-123');
    await page.locator('#login-form button[type=submit]').click();
    await page.locator('nav a[href="#settings"]').click();
    await page.locator('[data-action=change-password]').waitFor();
    const otherDevice=await browser.newContext({storageState:await page.context().storageState()});
    assert.equal((await otherDevice.request.get(process.env.TEST_UI_URL+'/api/v1/me')).status(),200);
    for(const locale of ['en','zh-CN']){
      await page.locator(`[data-locale=${locale}]`).click();
      await page.locator('[data-action=change-password]').click();
      for(const width of [1440,390,320]){
        await page.setViewportSize({width,height:900});
        assert(await page.locator('#dialog').evaluate(el=>el.scrollWidth<=el.clientWidth+1));
        await page.screenshot({path:`artifacts/password-dialog-${locale}-${width}.png`});
      }
      await page.locator('#dialog .dialog-head [data-action=close]').click();
      await page.setViewportSize({width:1440,height:1000});
    }
    await page.locator('[data-action=change-password]').click();
    await page.locator('#current_password').fill('incorrect-password');
    await page.locator('#new_password').fill('a-new-ui-password-456');
    await page.locator('#confirm_password').fill('a-different-confirmation');
    assert.equal(await page.locator('#confirm_password').evaluate(el=>el.checkValidity()),false);
    await page.locator('#confirm_password').fill('a-new-ui-password-456');
    assert.equal(await page.locator('[data-password-rule].complete').count(),3);
    await page.locator('#password-form button[type=submit]').click();
    await page.locator('#password-form .form-error').waitFor();
    assert((await page.locator('#password-form .form-error').textContent()).includes('当前密码不正确'));
    await page.locator('#current_password').fill('correct-horse-battery-staple-123');
    await page.locator('#password-form button[type=submit]').click();
    await page.locator('#login-form').waitFor();
    assert.equal(await page.locator('#password').inputValue(),'');
    assert.equal((await otherDevice.request.get(process.env.TEST_UI_URL+'/api/v1/me')).status(),401);
    await otherDevice.close();
    await page.locator('#email').fill('owner@example.test');
    await page.locator('#password').fill('a-new-ui-password-456');
    await page.locator('#login-form button[type=submit]').click();
    await page.locator('#content').waitFor();
    // Restore only the isolated test fixture's password for the remaining Rust checks.
    const restored=await page.request.put(process.env.TEST_UI_URL+'/api/v1/admin/password',{
      headers:{Origin:process.env.TEST_UI_URL},
      data:{current_password:'a-new-ui-password-456',new_password:'correct-horse-battery-staple-123'}
    });
    assert.equal(restored.status(),200);
    assert.deepEqual(await page.evaluate(()=>window.dashboardPolicyViolations),[]);
    assert.deepEqual(errors,[]);
    console.log('PASS: liquid-glass-react, password visibility/reset/session revocation, delivery switch, search, draft/locale preservation, error recovery, seven views in both themes/languages at 1440/390/320px, reduced motion/contrast, CSP and sandboxed previews (isolated fixtures; no public delivery)');
  }finally{await browser.close();}
})().catch(error=>{console.error(error.stack);process.exitCode=1;});
