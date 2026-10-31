// Called only by the isolated Rust integration test. Its server has no outbound worker.
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
(async()=>{
  const browser=await chromium.launch({headless:true});
  const page=await browser.newPage({viewport:{width:1440,height:1000}});
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  try{
    await page.goto(process.env.TEST_UI_URL);
    await page.locator('#email').fill('owner@example.test');
    await page.locator('#password').fill('correct-horse-battery-staple-123');
    await page.locator('#login-form button[type=submit]').click();
    await page.locator('nav a[href="#settings"]').click();
    await page.getByRole('heading',{name:'服务设置',exact:true}).waitFor();
    await page.locator('[data-action=test-email]').waitFor();
    fs.mkdirSync('artifacts',{recursive:true});
    await page.screenshot({path:'artifacts/settings-configured-fixture.png'});
    await page.locator('[data-action=test-email]').click();
    await page.locator('#test_to').fill('recipient@example.net');
    await page.locator('#test_language').selectOption('en');
    await page.locator('#test-form button[type=submit]').click();
    await page.locator('#test-result .badge.queued').waitFor();
    assert((await page.locator('#test-result').textContent()).includes('BunMail RS email delivery test'));
    await page.screenshot({path:'artifacts/test-send-queued-fixture.png'});
    await page.locator('#dialog [data-action=close]').click();
    await page.locator('nav a[href="#templates"]').click();
    await page.getByRole('heading',{name:'邮件模板',exact:true}).waitFor();
    await page.locator('[data-template="bee00000-0000-4000-8000-000000000002"] [data-action=preview-template]').click();
    await page.frameLocator('#template-preview').getByRole('heading',{name:'验证你的邮箱'}).waitFor();
    assert.equal(await page.frameLocator('#template-preview').locator('body').evaluate(body=>getComputedStyle(body).backgroundColor),'rgb(243, 245, 246)');
    await page.screenshot({path:'artifacts/template-preview-fixture.png'});
    await page.locator('#dialog [data-action=close]').click();
    // Render active HTML through the real form POST preview; sandbox and CSP must block it.
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
    assert.deepEqual(errors,[]);
    console.log('PASS: configured UI exposes test, real API enqueues English test, preview renders styles and blocks scripts/network (isolated DNS fixture; no public delivery)');
  }finally{await browser.close();}
})().catch(error=>{console.error(error.stack);process.exitCode=1;});
