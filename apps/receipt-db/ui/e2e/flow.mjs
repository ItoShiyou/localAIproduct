// 取り込み → 読み取り → 確認(修正・確定) → 一覧・検索 → 設定 の流れを、モックAPI(ブラウザだけ)で Playwright により確認する。
// スクリーンショットを docs/ui/ に保存する。実際の読み取りは src-tauri/tests/real_ocr_flow.rs で確認する。
// 使い方: (ui/ で) npx vite --port 5174 &  PLAYWRIGHT_DIR=<playwright を入れた場所> CHROMIUM=<実行ファイル> node e2e/flow.mjs
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
const require = createRequire(path.join(process.env.PLAYWRIGHT_DIR ?? process.cwd(), "node_modules/"));
const { chromium } = require("playwright");

const here = path.dirname(fileURLToPath(import.meta.url));
const shots = path.resolve(here, "../../docs/ui");
const files = ["01_receipt_clean.png", "16_invoice_textpdf.pdf", "02_receipt_clean.png"].map((f) => path.resolve(here, "../../testset/files", f));
const URL = process.env.URL ?? "http://127.0.0.1:5174/";

const browser = await chromium.launch({ executablePath: process.env.CHROMIUM });
const page = await (await browser.newContext({ viewport: { width: 1200, height: 800 } })).newPage();
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
const shot = (n) => page.screenshot({ path: path.join(shots, `mock-${n}.png`) });

await page.goto(URL);
await page.getByText("画面確認用のモック").waitFor();
// 1) 取り込み
await page.locator('input[type=file]:not([webkitdirectory])').setInputFiles(files);
await page.getByText("取り込み結果(3 件を追加)").waitFor();
await page.getByText("完了 3 件").waitFor({ timeout: 10000 });
await shot("1-import");
// 2) 確認画面: 3件の下書き。日付の無い行には指摘が出る
await page.getByRole("tab", { name: /確認/ }).click();
await page.getByText("3 件").first().waitFor();
await page.locator(".items button").filter({ hasText: "架空書店" }).click();
await page.getByText("日付が読み取れていません").waitFor();
assert.equal(await page.getByRole("button", { name: "この内容で確定" }).isDisabled(), true, "指摘があるうちは確定できない");
await page.locator("#f-date").fill("2026-04-05");
await page.locator("#f-invoice_no").fill("T9876543210123");
await page.locator("#f-invoice_no").press("Enter");
await page.waitForFunction(() => !document.querySelector(".issue"));
await shot("2-review");
await page.getByRole("button", { name: "この内容で確定" }).click();
await page.locator(".items li").nth(1).waitFor({ state: "detached" }).catch(() => undefined);
// 残り2件を一括確定
await page.getByRole("button", { name: /一括確定/ }).click();
await page.getByText("確認を待っている読み取り結果はありません").waitFor();
// 3) 一覧・検索
await page.getByRole("tab", { name: "一覧・検索" }).click();
await page.getByText("3 件(確定済みの合計").waitFor();
await page.getByPlaceholder(/支払先・摘要/).fill("みどり");
await page.getByText("1 件(確定済みの合計 ¥880)").waitFor();
await shot("3-list");
// 4) 設定: 更新確認のオフ、全削除の確認
await page.getByRole("tab", { name: "設定" }).click();
await page.getByText("通信一覧").waitFor();
await page.locator("input[type=checkbox]").uncheck();
await page.getByText("オフ").waitFor();
await page.getByRole("button", { name: "全データを削除…" }).click();
assert.equal(await page.getByRole("button", { name: "削除する" }).isDisabled(), true);
await shot("4-settings");

assert.deepEqual(errors, []);
console.log("ok: 取り込み → 確認 → 確定 → 検索 → 設定");
await browser.close();
