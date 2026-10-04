// 取り込み(同意の確認)→ 処理 → 確認(修正・話者・結合・分割・元に戻す)→ 確定 → 書き出し → 検索 → 用語辞書 → 設定 の流れを、
// モックAPI(ブラウザだけ)で Playwright により確認する。スクリーンショットを docs/ui/ に保存する。
// 実際の文字起こしは src-tauri/tests/real_asr.rs、処理の再開は src-tauri/src/pipeline.rs のテストで確認する。
// 使い方: (ui/ で) npx vite --port 5175 &  PLAYWRIGHT_DIR=<playwright を入れた場所> CHROMIUM=<実行ファイル> node e2e/flow.mjs
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
const require = createRequire(path.join(process.env.PLAYWRIGHT_DIR ?? process.cwd(), "node_modules/"));
const { chromium } = require("playwright");

const here = path.dirname(fileURLToPath(import.meta.url));
const shots = path.resolve(here, "../../docs/ui");
const URL = process.env.URL ?? "http://127.0.0.1:5175/";

const browser = await chromium.launch({ executablePath: process.env.CHROMIUM });
const page = await (await browser.newContext({ viewport: { width: 1200, height: 820 } })).newPage();
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
const shot = (n) => page.screenshot({ path: path.join(shots, `mock-${n}.png`) });
const segs = page.getByTestId("segment");

await page.goto(URL);
await page.getByText("画面確認用のモック").waitFor();
// 用語辞書を先に登録
await page.getByRole("tab", { name: "用語辞書" }).click();
await page.getByPlaceholder(/誤りやすい表記/).fill("やまだ商事");
await page.getByPlaceholder(/正しい表記/).fill("山田商事");
await page.getByRole("button", { name: "追加" }).click();
await page.getByRole("cell", { name: "山田商事" }).waitFor();

// 1) 取り込み: 初回は録音の同意の確認が出る
await page.getByRole("tab", { name: "議事録" }).click();
await page.getByRole("button", { name: "録音を取り込む" }).click();
await page.getByRole("dialog", { name: "録音の同意について" }).waitFor();
await shot("1-consent");
await page.getByRole("button", { name: "確認しました" }).click();
await page.getByText("文字起こし済み").first().waitFor({ timeout: 10000 });
await segs.first().waitFor();
assert.equal(await segs.count(), 6);
assert.match(await segs.nth(2).locator("textarea").inputValue(), /山田商事/, "用語辞書で置換される");
assert.equal(await segs.nth(2).getByText("要確認").count(), 1, "信頼度が低い文に印");
// 2回目は同意の確認が出ない
await page.getByRole("button", { name: "録音を取り込む" }).click();
await page.locator(".items li").nth(1).waitFor();
await page.waitForFunction(() => !document.querySelector(".items .s-processing, .items .s-queued"), null, { timeout: 10000 });
await page.locator(".items button").nth(1).click(); // 1件目(先に取り込んだ方)に戻る

// 2) 確認: 修正・話者・結合・分割・元に戻す
await page.getByRole("button", { name: "確定" }).waitFor();
const t0 = segs.nth(0).locator("textarea");
await t0.fill("それでは、定例会議を始めます。");
await t0.blur();
await page.getByRole("button", { name: "元に戻す" }).waitFor({ state: "visible" });
await segs.nth(0).getByLabel("話者").fill("佐藤");
await segs.nth(0).getByLabel("話者").press("Enter");
await page.waitForFunction(() => [...document.querySelectorAll('[data-testid=segment] input')].every((i) => i.value === "佐藤"));
await segs.nth(1).getByRole("button", { name: "次と結合" }).click();
await page.waitForFunction(() => document.querySelectorAll("[data-testid=segment]").length === 5);
const t1 = segs.nth(1).locator("textarea");
await t1.click();
await t1.evaluate((el) => el.setSelectionRange(10, 10));
await segs.nth(1).getByRole("button", { name: "ここで分割" }).click();
await page.waitForFunction(() => document.querySelectorAll("[data-testid=segment]").length === 6);
assert.equal(await segs.nth(1).locator("textarea").inputValue(), "本日の議題は二つです");
await page.getByRole("button", { name: "元に戻す" }).click();
await page.waitForFunction(() => document.querySelectorAll("[data-testid=segment]").length === 5);
await page.getByLabel("タイトル").fill("10月の定例会議");
await page.getByLabel("参加者").fill("佐藤、鈴木");
await page.getByLabel("参加者").blur();
await shot("2-review");

// 3) 確定 → 書き出し
assert.equal(await page.getByLabel("書き出し").isDisabled(), true, "確定前は書き出せない");
await page.getByRole("button", { name: "確定", exact: true }).click();
await page.getByText("確定しました").waitFor();
await page.getByLabel("書き出し").selectOption("md");
await page.getByText("書き出しました").waitFor();

// 4) 検索 → 該当の議事録を開く
await page.getByRole("tab", { name: "検索" }).click();
await page.getByPlaceholder(/全文検索/).fill("展示会");
await page.locator(".hits button").first().waitFor();
await shot("3-search");
await page.locator(".hits button").first().click();
await page.getByLabel("タイトル").waitFor();

// 5) 設定
await page.getByRole("tab", { name: "設定" }).click();
await page.getByText("通信一覧").waitFor();
// モデル: 削除 → 取得(モックでは通信しない)
const model = page.getByTestId("model");
await model.getByText("取得済み").waitFor();
await model.getByRole("button", { name: "モデルを削除…" }).click();
await model.getByRole("button", { name: "削除する" }).click();
await model.getByRole("button", { name: "取得する" }).click();
await model.getByText("取得しました").waitFor();
await page.getByLabel(/音声のコピーを残す/).uncheck();
await page.getByRole("button", { name: "全データを削除…" }).click();
assert.equal(await page.getByRole("button", { name: "削除する" }).isDisabled(), true);
await shot("4-settings");

assert.deepEqual(errors, []);
console.log("ok: 取り込み → 確認 → 確定 → 書き出し → 検索 → 用語辞書 → 設定");
await browser.close();
