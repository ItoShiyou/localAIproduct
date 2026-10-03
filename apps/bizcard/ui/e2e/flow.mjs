// 撮影 → 枠表示 → 編集 → 確定 → 検索 の流れを Playwright で確認し、スクリーンショットを docs/ui/ に保存する。
// 使い方: (ui/ で) npm run build && npx vite preview --port 4173 &  node e2e/flow.mjs
// playwright install は使わない。Chromium は executablePath で指定する(既定 /opt/pw-browsers/chromium)。
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
const require = createRequire("/opt/node22/lib/node_modules/");
const { chromium } = require("playwright");

const here = path.dirname(fileURLToPath(import.meta.url));
const shots = path.resolve(here, "../../docs/ui");
const testImage = path.resolve(here, "../../testset/images/01-a.png");
const URL = process.env.URL ?? "http://127.0.0.1:4173/";
const exe = process.env.CHROMIUM ?? "/opt/pw-browsers/chromium";
const fakeCam = ["--use-fake-ui-for-media-stream", "--use-fake-device-for-media-stream"];

async function run(label, ctxOpts, args) {
  const browser = await chromium.launch({ executablePath: exe, args });
  const ctx = await browser.newContext(ctxOpts);
  const page = await ctx.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  const shot = (n) => page.screenshot({ path: path.join(shots, `${label}-${n}.png`) });

  await page.goto(URL);
  await page.getByTestId("mock-banner").waitFor();
  // 1) カメラのプレビュー(fake device)
  await page.getByTestId("shutter").waitFor();
  await page.waitForFunction(() => document.querySelector("video")?.videoWidth > 0);
  await page.waitForFunction(() => !document.querySelector('[data-testid="shutter"]').disabled);
  await shot("01-camera");

  // 2) 撮影 → 画像表示 + 下部に項目枠
  await page.getByTestId("shutter").click();
  await page.getByTestId("photo").waitFor();
  await page.getByTestId("reading").waitFor();
  await shot("02-reading");
  await page.getByTestId("field-name").locator("input").waitFor();
  await page.waitForFunction(() => document.querySelector("#f-name")?.value === "青木 遥");
  assert.equal(await page.locator("#f-company").inputValue(), "株式会社ひなた工房");
  assert.equal(await page.locator("#f-email").inputValue(), "aoki@hinata-kobo.example.com");
  await shot("03-fields");

  // 3) その場で打ち替え → 元に戻す
  await page.locator("#f-name").fill("青木 はるか");
  await page.locator("#f-title").fill("部長");
  assert.equal(await page.locator("#f-name").inputValue(), "青木 はるか");
  await page.getByTestId("undo").click();
  assert.equal(await page.locator("#f-title").inputValue(), "課長"); // 直前の編集だけ戻る
  assert.equal(await page.locator("#f-name").inputValue(), "青木 はるか");
  await page.getByTestId("enc-place").fill("横浜の展示会");
  await page.getByTestId("enc-memo").fill("猫を飼っている。来月ロゴの相談");
  await shot("04-edited");

  // 4) 連続撮影: 確定の前に2枚目を撮る(キュー)
  await page.getByTestId("add-more").click();
  await page.getByTestId("shutter").waitFor();
  await page.waitForFunction(() => !document.querySelector('[data-testid="shutter"]').disabled);
  await page.getByTestId("shutter").click();
  await page.waitForFunction(() => document.querySelector("#f-name")?.value === "井上 健太");
  assert.equal(await page.getByTestId("queue-item").count(), 2);
  // 信頼度が低い項目に印
  assert.ok(await page.getByTestId("low-nameKana").isVisible());
  assert.ok(await page.getByTestId("low-address").isVisible());
  await shot("05-queue-low-confidence");

  // 5) 2枚目を確定(メモは空のまま = スキップ)→ 1枚目の確認へ戻る
  await page.getByTestId("confirm").click();
  await page.waitForFunction(() => document.querySelector("#f-name")?.value === "青木 はるか");
  await page.getByTestId("confirm").click(); // 1枚目を確定。待ちが無くなればカメラへ
  await page.getByTestId("camera").waitFor();
  await shot("06-after-confirm");

  // 6) 検索(2文字の氏名)→ 名刺画像とメモ
  await page.getByTestId("tab-search").click();
  await page.getByTestId("search-input").fill("青木");
  await page.getByTestId("hit").first().waitFor();
  assert.equal(await page.getByTestId("hit").count(), 1);
  assert.match(await page.getByTestId("hit-memo").innerText(), /ロゴの相談/);
  await shot("07-search");
  await page.getByTestId("search-input").fill("猫");
  await page.getByTestId("hit").first().waitFor();
  assert.equal(await page.getByTestId("hit").count(), 1);

  assert.deepEqual(errors, []);
  await browser.close();
}

async function runNoCamera(label, ctxOpts) {
  // カメラが無い環境(fake device なし)→ 取り込みへ
  const browser = await chromium.launch({ executablePath: exe, args: [] });
  const page = await (await browser.newContext(ctxOpts)).newPage();
  await page.goto(URL);
  await page.getByTestId("camera-unavailable").waitFor();
  await page.screenshot({ path: path.join(shots, `${label}-08-no-camera.png`) });
  await page.getByTestId("file-input").setInputFiles(testImage);
  await page.waitForFunction(() => document.querySelector("#f-name")?.value === "青木 遥");
  await page.screenshot({ path: path.join(shots, `${label}-09-imported-file.png`) });
  await browser.close();
}

const desktop = { viewport: { width: 1100, height: 800 } };
const phone = { viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true };
await run("desktop", desktop, fakeCam);
await run("phone", phone, fakeCam);
await runNoCamera("phone", phone);
console.log("OK");
