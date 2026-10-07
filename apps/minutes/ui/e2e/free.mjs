// Free edition UI regression. No real audio recognition or purchases.
import { createRequire } from 'node:module';
import path from 'node:path';
import assert from 'node:assert/strict';
const require = createRequire(path.join(process.env.PLAYWRIGHT_DIR ?? process.cwd(), 'node_modules/'));
const { chromium } = require('playwright');
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  page.on('pageerror', e => errors.push(String(e)));
  await page.goto((process.env.URL ?? 'http://127.0.0.1:5175/') + '?free');
  await page.getByTestId('edition-badge').getByText('無料版', { exact: true }).waitFor();
  await page.getByRole('button', { name: '一覧', exact: true }).click();
  await page.getByTestId('plan-strip').getByText('残り 60 分', { exact: false }).waitFor();
  await page.getByRole('button', { name: '取り込みの設定 ▼', exact: true }).click();
  assert.equal(await page.getByTestId('options').getByLabel(/話者を判別/).count(), 0);
  assert.equal(await page.getByTestId('options').getByLabel(/ノイズ除去/).count(), 0);
  assert.equal(await page.getByRole('tab', { name: '用語辞書', exact: true }).count(), 0);
  await page.getByRole('button', { name: '一覧を閉じる', exact: true }).click();
  await page.getByRole('button', { name: '取り込む', exact: true }).click();
  await page.getByRole('button', { name: '確認しました', exact: true }).click();
  await page.getByTestId('reading').waitFor();
  await page.getByRole('button', { name: '確定', exact: true }).click();
  await page.getByRole('button', { name: '書き出し', exact: true }).click();
  const formats = await page.getByRole('menu', { name: '書き出す形式', exact: true })
    .getByRole('menuitem').allTextContents();
  assert.deepEqual(formats, ['テキスト']);
  await page.keyboard.press('Escape');
  await page.getByRole('tab', { name: '議事録を仕上げる', exact: true }).click();
  assert.equal(await page.getByTestId('summary').count(), 0);
  assert.equal(await page.getByRole('button', { name: '用語辞書を適用', exact: true }).count(), 0);
  await page.getByRole('button', { name: '一覧', exact: true }).click();
  await page.getByRole('tab', { name: '設定', exact: true }).click();
  assert.equal(await page.getByTestId('summary-model').count(), 0);
  assert.equal(await page.getByLabel(/ノイズ除去を既定/).count(), 0);
  await page.getByTestId('plan-card').getByRole('button', { name: 'Pro版の購入先を選ぶ ↗', exact: true }).waitFor();
  assert.deepEqual(errors, []);
  console.log('PASS Free: no paid controls, TXT only, purchase chooser');
} finally { await browser.close(); }
