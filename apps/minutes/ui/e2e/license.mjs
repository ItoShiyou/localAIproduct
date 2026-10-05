// ライセンス(オフライン方式)の設定画面を、モックAPIで Playwright により確認する。
// 無料版で開く → 形式違い・署名違い・別端末のキーはエラーが出る → 有効なキーで有料版+登録名 → 外す(確認が出る)→ 無料版に戻る
// → ネットワークを使わない(要約のモデルの取得が止まり、ファイルから取り込める)→ オフライン版のキー(切り替え不可)
// 使い方: (ui/ で) npx vite --port 5175 &  PLAYWRIGHT_DIR=<場所> CHROMIUM=<実行ファイル> [SHOTS=<画像の保存先>] node e2e/license.mjs
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
const require = createRequire(path.join(process.env.PLAYWRIGHT_DIR ?? process.cwd(), "node_modules/"));
const { chromium } = require("playwright");
const here = path.dirname(fileURLToPath(import.meta.url));
const shots = process.env.SHOTS ?? path.resolve(here, "../../docs/ui");
const URL = process.env.URL ?? "http://127.0.0.1:5175/";

const browser = await chromium.launch({ executablePath: process.env.CHROMIUM });
const ctx = await browser.newContext({ viewport: { width: 1280, height: 1000 }, permissions: ["clipboard-read", "clipboard-write"] });
const page = await ctx.newPage();
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
const shot = (n) => page.screenshot({ path: path.join(shots, `license-${n}.png`), fullPage: false });

await page.goto(URL + "?free");
// 無料版の入口: 残り時間の帯から設定へ
const strip = page.getByTestId("plan-strip");
await strip.getByText("残り 60 分").waitFor();
await strip.getByRole("button", { name: /ライセンスの登録/ }).click();

const lic = page.getByTestId("license");
await lic.getByTestId("license-status").getByText("無料版").waitFor();
assert.equal(await lic.getByTestId("license-install").isDisabled(), true, "空のままでは登録できない");
await lic.scrollIntoViewIfNeeded();
await shot("1-free");

// 形式が違う / 署名が合わない / 別の端末用 → エラーが出て、無料版のまま
for (const [key, msg] of [["abc", "形式が違います"], ["MNT1-XXXX-XXXX", "署名が合いません"], ["MNT1-MOCK-OTHER-1", "別の端末用です"]]) {
  await lic.getByTestId("license-input").fill(key);
  await lic.getByTestId("license-install").click();
  await lic.getByTestId("license-error").getByText(msg).waitFor();
  await lic.getByTestId("license-status").getByText("無料版").waitFor();
}
await shot("2-error");

// 有効なキー → 有料版・登録名。画面のあちこち(帯・プラン・モデル)も切り替わる
await lic.getByTestId("license-input").fill("MNT1-MOCK-AAAA-BBBB");
await lic.getByTestId("license-install").click();
await lic.getByTestId("license-licensee").getByText("架空大学 山田研究室 様").waitFor();
await lic.locator(".lic-tier").getByText("有料版", { exact: true }).waitFor();
await lic.getByTestId("license-ok").waitFor();
await page.getByTestId("plan-card").getByText("ご利用のプラン: 有料版").waitFor();
await page.getByTestId("summary-model").getByTestId("summary-model-locked").waitFor({ state: "detached" });
assert.equal(await page.getByTestId("summary-model-get").isDisabled(), false, "有料版では要約のモデルを取得できる");
await page.getByRole("tab", { name: "議事録" }).click();
assert.equal(await page.getByTestId("plan-strip").count(), 0, "有料版では無料版の帯が消える");
await page.getByRole("tab", { name: "設定" }).click();
await lic.scrollIntoViewIfNeeded();
await shot("3-pro");

// 端末コードの表示とコピー
await lic.getByTestId("machine-code").getByText("ABCD-EFGH-IJKL-MNOP").waitFor();
await lic.getByTestId("machine-copy").click();
await lic.getByText("コピーしました").waitFor();
assert.equal(await page.evaluate(() => navigator.clipboard.readText()), "ABCD-EFGH-IJKL-MNOP");

// 外す: 確認が出て、やめるでは変わらない → 外すで無料版に戻る
await lic.getByTestId("license-remove").click();
await lic.getByTestId("license-remove-confirm").getByRole("button", { name: "やめる" }).click();
await lic.getByTestId("license-licensee").waitFor();
await lic.getByTestId("license-remove").click();
await lic.getByTestId("license-remove-confirm").waitFor();
await lic.getByTestId("license-remove-yes").click();
await lic.getByTestId("license-status").getByText("無料版").waitFor();
assert.equal(await lic.getByTestId("license-licensee").count(), 0);
await page.getByTestId("plan-card").getByText("ご利用のプラン: 無料版").waitFor();
assert.equal(await page.getByTestId("summary-model-get").isDisabled(), true, "無料版では取得できない");

// ネットワークを使わない: 要約のモデルの取得が止まり、ファイルからの取り込みが使える
await lic.getByTestId("license-input").fill("MNT1-MOCK-AAAA");
await lic.getByTestId("license-install").click();
await lic.getByTestId("license-licensee").waitFor();
const sm = page.getByTestId("summary-model");
assert.equal(await sm.getByTestId("summary-model-get").isDisabled(), false);
const toggle = page.getByTestId("offline-toggle");
assert.equal(await toggle.isDisabled(), false);
await toggle.check();
assert.equal(await sm.getByTestId("summary-model-get").isDisabled(), true, "オフラインでは取得できない");
await sm.getByTestId("summary-model-offline").getByText("ファイルから取り込んでください").waitFor();
await page.getByText("通信一覧").scrollIntoViewIfNeeded();
assert.ok((await page.getByText("止めています(ネットワークを使わない設定)").count()) >= 1, "通信一覧が止めている表示になる");
// 取り込みの失敗(内容の確認で不一致)→ メッセージ → もう一度で成功
await page.evaluate(() => globalThis.__mockImportError("ファイルの内容が想定と違います(SHA-256 が一致しません)。ファイルが壊れているか、別のファイルです"));
await sm.getByTestId("summary-model-import").click();
await sm.getByText("SHA-256 が一致しません").waitFor();
await sm.getByTestId("summary-model-import").click();
await sm.getByTestId("summary-model-progress").getByText("取り込み中").waitFor();
await sm.getByText("取り込みました").waitFor();
await sm.scrollIntoViewIfNeeded();
await shot("4-offline-summary");
await toggle.uncheck();
await sm.getByText("入っています").waitFor();

// オフライン版のキー: 切り替えられず、常にオン
await lic.getByTestId("license-remove").click();
await lic.getByTestId("license-remove-yes").click();
await lic.getByTestId("license-input").fill("MNT1-MOCK-OFFLINE-1");
await lic.getByTestId("license-install").click();
await lic.getByTestId("license-status").getByText("有料版・オフライン版").waitFor();
assert.equal(await toggle.isDisabled(), true);
assert.equal(await toggle.isChecked(), true);
await page.getByTestId("offline-forced").waitFor();
await page.getByTestId("offline-mode").scrollIntoViewIfNeeded();
await shot("5-offline-edition");

// ファイルから登録(モックでは固定のキー)
await lic.getByTestId("license-remove").click();
await lic.getByTestId("license-remove-yes").click();
await lic.getByTestId("license-file").click();
await lic.getByTestId("license-licensee").waitFor();

assert.deepEqual(errors, []);
console.log("ok: ライセンス(エラー → 登録 → 端末コード → 外す → ネットワークを使わない → オフライン版)");
await browser.close();
