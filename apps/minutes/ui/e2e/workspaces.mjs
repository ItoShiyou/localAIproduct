// 新しい4画面の回帰テスト。ViteのモックAPIを使用する（実際の認識精度のテストではない）。
// ui/で開発サーバー起動後: PLAYWRIGHT_DIR=<インストール先> node e2e/workspaces.mjs
import { createRequire } from "node:module";
import path from "node:path";
import assert from "node:assert/strict";
const require = createRequire(path.join(process.env.PLAYWRIGHT_DIR ?? process.cwd(), "node_modules/"));
const { chromium } = require("playwright");
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto(process.env.URL ?? "http://127.0.0.1:5175/");
  await page.getByRole("button", { name: "取り込む", exact: true }).click();
  await page.getByRole("button", { name: "確認しました" }).click();
  await page.getByTestId("reading").waitFor();
  const switchTo = (name) => page.getByRole("tab", { name, exact: true }).click();
  await switchTo("一文ずつ直す");
  const review = page.getByTestId("sentence-review");
  assert.equal(await review.getByTestId("segment").count(), 1);
  await review.getByRole("textbox").fill("保存した発言です。");
  await review.getByRole("button", { name: "次の文", exact: true }).click();
  await review.getByRole("button", { name: "前の文", exact: true }).click();
  assert.equal(await review.getByRole("textbox").inputValue(), "保存した発言です。");
  await switchTo("読む");
  await page.getByTestId("reading").getByText("保存した発言です。", { exact: true }).waitFor();
  await switchTo("聞いて直す");
  assert.equal(await page.getByTestId("segment").count(), 6);
  await switchTo("議事録を仕上げる");
  await page.getByRole("textbox", { name: "議題", exact: true }).fill("次回の予定");
  await switchTo("読む");
  await switchTo("議事録を仕上げる");
  assert.equal(await page.getByRole("textbox", { name: "議題", exact: true }).inputValue(), "次回の予定");
  await page.getByRole("button", { name: "確定", exact: true }).click();
  await page.getByRole("button", { name: "書き出し", exact: true }).click();
  await page.getByRole("menu", { name: "書き出す形式" }).waitFor();
  assert.equal(await page.locator("body").evaluate((el) => el.scrollWidth > innerWidth), false);
  assert.deepEqual(errors, []);
  console.log("4画面・編集保存・議事録入力保持・書き出し: OK");
} finally {
  await browser.close();
}
