import fs from 'node:fs/promises';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { chromium } = require('/Users/gfhfyjbr/.npm/_npx/e058441c325e062a/node_modules/playwright-core');
const browser = await chromium.launch({executablePath:'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless:true});
try {
  const page = await browser.newPage();
  await page.goto('https://www.microsoft.com/en-us/software-download/windows11arm64', {waitUntil:'networkidle', timeout:90000});
  const form = await page.evaluate(() => ({
    selects: [...document.querySelectorAll('select')].map(s => ({id:s.id, options:[...s.options].map(o=>({value:o.value,text:o.text}))})),
    buttons: [...document.querySelectorAll('button,input[type=submit],a')].map(b=>({id:b.id,text:b.textContent.trim(),href:b.getAttribute('href')})).filter(b=>/download|confirm/i.test(b.text)),
  }));
  await fs.writeFile('glimm-windows-download-form.json',JSON.stringify(form,null,2));
  await page.screenshot({path:'glimm-windows-download-page.png',fullPage:true});
  await page.locator('#product-edition').selectOption('3816');
  await page.locator('#submit-product-edition').click();
  await page.waitForFunction(() => document.querySelector('#product-languages')?.options.length > 1, null, {timeout:120000});
  const language = await page.locator('#product-languages').evaluate(s => [...s.options].find(o => ['English','English (United States)'].includes(o.text.trim()))?.value);
  if (!language) throw new Error('No US English option: '+await page.locator('#product-languages').innerText());
  await page.locator('#product-languages').selectOption(language);
  await page.locator('#submit-sku').click();
  await page.waitForFunction(() => [...document.querySelectorAll('a[href]')].some(a => a.href.toLowerCase().includes('.iso')), null, {timeout:120000});
  const url = await page.evaluate(() => [...document.querySelectorAll('a[href]')].find(a => a.href.toLowerCase().includes('.iso')).href);
  const rows = await page.locator('tr').allInnerTexts();
  await fs.writeFile('glimm-windows-iso-details.json',JSON.stringify({url,rows},null,2));
  const sha256 = rows.filter(text => text.includes('English') && !text.includes('International') && !text.includes('Australia')).map(text=>text.match(/[A-Fa-f0-9]{64}/)?.[0]).find(Boolean);
  if (!sha256 || !/^[A-Fa-f0-9]{64}$/.test(sha256)) throw new Error('Missing official English ISO checksum');
  if (!new URL(url).hostname.endsWith('.microsoft.com')) throw new Error('Unexpected download host');
  await fs.writeFile('glimm-windows-iso-download.json',JSON.stringify({url,language,sha256},null,2));
} finally { await browser.close(); }
