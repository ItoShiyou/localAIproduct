// 議事録の画面の流れを、モックAPI(ブラウザだけ)で Playwright により確認する。スクリーンショットを docs/ui/ に保存する。
// 取り込み(設定・同意)→ 処理 → 話者(名前の変更・判別し直し)→ 文を押して移動・要確認の巡回 → 修正・結合・分割・元に戻す
// → 検索・置換 → 定型欄・タグ・絞り込み → 確定 → 書き出し(Word・PDF)→ やり直しの画面 → 検索 → 用語辞書(CSV)→ 設定
// 実際の文字起こし・話者の判別は src-tauri/tests/real_*.rs、処理の再開は pipeline のテストで確認する。
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

const browser = await chromium.launch({ executablePath: process.env.CHROMIUM, args: ["--use-fake-ui-for-media-stream", "--use-fake-device-for-media-stream"] });
const page = await (await browser.newContext({ viewport: { width: 1280, height: 900 } })).newPage();
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
const shot = (n) => page.screenshot({ path: path.join(shots, `mock-${n}.png`) });
const segs = page.getByTestId("segment");
const segTexts = () => segs.locator("textarea").evaluateAll((els) => els.map((e) => e.value));
const speakersOf = () => segs.getByLabel("話者").evaluateAll((els) => els.map((e) => e.value));

await page.goto(URL);
await page.getByTitle(/画面確認用のモック/).waitFor();

// 0) 用語辞書(手入力と CSV)
await page.getByRole("tab", { name: "用語辞書" }).click();
await page.getByPlaceholder(/誤りやすい表記/).fill("やまだ商事");
await page.getByPlaceholder(/正しい表記/).fill("山田商事");
await page.getByRole("button", { name: "追加" }).click();
await page.getByRole("cell", { name: "山田商事" }).waitFor();
await page.getByRole("button", { name: "CSV から取り込む" }).click();
await page.getByText("1 件を取り込みました").waitFor();
await page.getByRole("cell", { name: "クラウド" }).waitFor();

// 1) 取り込みの設定(話者3人・範囲)→ 同意 → 処理
await page.getByRole("tab", { name: "議事録" }).click();
await page.getByRole("button", { name: /取り込みの設定/ }).click();
const opts = page.getByTestId("options");
await opts.getByRole("combobox").first().selectOption("3");
await page.getByLabel("範囲の始まり").fill("ab");
await page.getByText(/範囲は「分:秒」/).waitFor();
await page.getByLabel("範囲の始まり").fill("0:30");
await page.getByRole("button", { name: "録音を取り込む" }).click();
await page.getByRole("dialog", { name: "録音の同意について" }).waitFor();
await page.getByRole("button", { name: "確認しました" }).click();
await page.getByText("文字起こし済み").first().waitFor({ timeout: 10000 });
await segs.first().waitFor();
assert.equal(await segs.count(), 6);
assert.match((await segTexts())[2], /山田商事/, "用語辞書で置換される");
assert.match(await page.locator(".meta-row").innerText(), /範囲 0:00:30〜最後|範囲 0:30〜最後/);

// 2) 話者: 自動のラベル → 名前をまとめて変更 → 判別し直し
assert.deepEqual(await speakersOf(), ["話者1", "話者2", "話者1", "話者2", "話者3", "話者3"]);
const spk = page.getByTestId("speakers");
await spk.getByRole("button", { name: /話者1/ }).click();
await spk.getByLabel("話者1 の新しい名前").fill("佐藤");
await spk.getByLabel("話者1 の新しい名前").press("Enter");
await page.waitForFunction(() => [...document.querySelectorAll("[data-testid=segment] input[aria-label=話者]")].filter((i) => i.value === "佐藤").length === 2);
await shot("2-speakers");
await spk.getByLabel("話者を判別し直す").selectOption("2");
await page.getByText("話者を判別し直しました").waitFor();
assert.deepEqual([...new Set(await speakersOf())].sort(), ["話者1", "話者2"]);
await page.getByRole("button", { name: "元に戻す" }).click();
await page.waitForFunction(() => [...document.querySelectorAll("[data-testid=segment] input[aria-label=話者]")].some((i) => i.value === "佐藤"));

// 3) 文を押すと、その位置へ(再生中の印が移る)。要確認の巡回
await segs.nth(3).evaluate((el) => el.click()); // カードの余白を押したのと同じ
await page.waitForFunction(() => document.querySelectorAll("[data-testid=segment]")[3].classList.contains("playing"));
await page.getByRole("button", { name: "次の要確認" }).click();
await page.waitForFunction(() => document.querySelectorAll("[data-testid=segment]")[4].classList.contains("playing"));
assert.equal(await page.evaluate(() => document.activeElement?.tagName), "TEXTAREA", "要確認の文の入力欄に移る");
await page.getByRole("button", { name: "次の要確認" }).click();
await page.waitForFunction(() => document.querySelectorAll("[data-testid=segment]")[2].classList.contains("playing"));
// キー: ↓ で次の文
await page.locator("body").click({ position: { x: 5, y: 5 } });
await page.keyboard.press("ArrowDown");
await page.waitForFunction(() => document.querySelectorAll("[data-testid=segment]")[3].classList.contains("playing"));

// 4) 修正・結合・分割・元に戻す
const t0 = segs.nth(0).locator("textarea");
await t0.fill("それでは、定例会議を始めます。");
await t0.blur();
await page.waitForFunction(() => document.querySelectorAll("[data-testid=segment] textarea")[0].value.startsWith("それでは、定例"));
await segs.nth(1).getByRole("button", { name: "次と結合" }).click();
await page.waitForFunction(() => document.querySelectorAll("[data-testid=segment]").length === 5);
const t1 = segs.nth(1).locator("textarea");
await t1.click();
await t1.evaluate((el) => el.setSelectionRange(10, 10));
await segs.nth(1).getByRole("button", { name: "ここで分割" }).click();
await page.waitForFunction(() => document.querySelectorAll("[data-testid=segment]").length === 6);
assert.equal((await segTexts())[1], "本日の議題は二つです");
await page.getByRole("button", { name: "元に戻す" }).click();
await page.waitForFunction(() => document.querySelectorAll("[data-testid=segment]").length === 5);

// 5) 検索・置換(⌘F)
await page.locator("body").click({ position: { x: 5, y: 5 } });
await page.keyboard.press("Control+f");
const fb = page.getByTestId("findbar");
await fb.getByPlaceholder("この議事録の中を検索").fill("展示会");
await fb.getByText("1 / 1 件").waitFor();
await fb.getByPlaceholder("置き換える語").fill("見本市");
await fb.getByRole("button", { name: "すべて置換" }).click();
await page.getByText("1 件の文で置き換えました").waitFor();
assert.ok((await segTexts()).some((t) => t.includes("見本市")));
await shot("3-review");

// 6) 定型欄・タグ
await page.getByRole("tab", { name: /議題・決定事項・ToDo/ }).click();
const notes = page.getByTestId("notes");
await notes.getByLabel("議題").fill("見積もり\n展示会の準備");
await notes.getByLabel("決定事項").fill("パンフレットは五百部");
await notes.getByRole("button", { name: "＋ ToDo を追加" }).click();
await notes.getByPlaceholder("やること").fill("入稿");
await notes.getByPlaceholder("担当").fill("鈴木");
await notes.getByLabel("期限").fill("2026-10-08");
await notes.getByLabel("期限").blur();
await page.getByRole("textbox", { name: "タグ", exact: true }).fill("定例、案件A");
await page.getByLabel("タイトル", { exact: true }).fill("10月の定例会議");
await page.getByLabel("タイトル", { exact: true }).blur();
await page.getByRole("tab", { name: "編集" }).click();
await page.locator(".items .tag-chip", { hasText: "案件A" }).first().waitFor();
await shot("4-notes");
// タグで絞り込み
await page.getByLabel("タグで絞り込み").selectOption("案件A");
await page.waitForFunction(() => document.querySelectorAll(".items li").length === 1);
await page.getByLabel("タグで絞り込み").selectOption("");
await page.waitForFunction(() => document.querySelectorAll(".items li").length === 1); // 取り込みは1件

// 7) 確定 → 書き出し(Word・PDF)
assert.equal(await page.getByLabel("書き出し").isDisabled(), true, "確定前は書き出せない");
await page.getByRole("button", { name: "確定", exact: true }).click();
await page.getByText("確定しました").waitFor();
await page.getByLabel("書き出し").selectOption("docx");
await page.getByText("書き出しました").waitFor();
await page.getByLabel("書き出し").selectOption("pdf");
const pd = page.getByRole("dialog", { name: "PDF にする" });
await pd.getByRole("button", { name: "印刷の画面を開く" }).click();
await page.getByText("PDF として保存").waitFor();
// 印刷の見た目(Chromium で PDF にして確かめる。アプリでは OS の印刷画面から保存する)
await page.pdf({ path: path.join(shots, "print-sample.pdf"), format: "A4", preferCSSPageSize: true, printBackground: true });
// 印刷用の表示に、議題・ToDo・話者が入っている
const printed = await page.locator(".print-doc").evaluate((el) => el.textContent);
assert.match(printed, /10月の定例会議/);
assert.match(printed, /入稿鈴木2026-10-08/, "ToDo の表(内容・担当・期限)");
assert.match(printed, /決定事項パンフレットは五百部/);
assert.match(printed, /佐藤/);

// 8) 設定を変えてやり直す(画面だけ確認して、やめる)
await page.locator("details.more > summary").click();
await page.getByRole("button", { name: "設定を変えてやり直す" }).click();
const redo = page.getByRole("dialog", { name: "設定を変えてやり直す" });
await redo.getByText("これまでの修正").waitFor();
await redo.locator("select").nth(1).selectOption("en");
await shot("5-redo");
await redo.getByRole("button", { name: "やめる" }).click();

// 9) 全体の検索 → 該当の位置を開く
await page.getByRole("tab", { name: "検索" }).click();
await page.getByPlaceholder(/全文検索/).fill("パンフレット");
await page.locator(".hits button").first().waitFor().catch(() => undefined);
await page.getByPlaceholder(/全文検索/).fill("見本市");
await page.locator(".hits button").first().waitFor();
await page.locator(".hits button").first().click();
await page.getByLabel("タイトル", { exact: true }).waitFor();
await page.waitForFunction(() => [...document.querySelectorAll("[data-testid=segment]")].some((s) => s.classList.contains("playing") && s.querySelector("textarea").value.includes("見本市")));

// 10) マイクで録音(仮の文字 → 止めると正確な文字起こしで置き換わる)
await page.getByRole("tab", { name: "議事録" }).click();
await page.getByRole("button", { name: "● 録音する" }).click();
const rec = page.getByTestId("recorder");
await rec.getByText("録音中").waitFor({ timeout: 10000 });
await page.getByTestId("provisional").waitFor();
await page.waitForFunction(() => [...document.querySelectorAll("[data-testid=segment]")].some((s) => s.textContent.includes("仮の文字")), null, { timeout: 15000 });
await shot("6-recording");
assert.equal(await segs.locator("textarea").count(), 0, "仮の文字は編集できない");
await rec.getByRole("button", { name: "止めて保存" }).click();
await rec.waitFor({ state: "detached" });
await page.waitForFunction(() => !document.querySelector("[data-testid=provisional]") && document.querySelectorAll("[data-testid=segment] textarea").length > 0, null, { timeout: 15000 });
assert.ok(!(await segTexts()).some((t) => t.includes("仮の文字")), "正確な文字に置き換わる");

// 11) 設定
await page.getByRole("tab", { name: "設定" }).click();
await page.getByText("通信一覧").waitFor();
const model = page.getByTestId("model");
await model.getByText("取得済み").waitFor();
await page.getByLabel(/音声のコピーを残す/).uncheck();
await page.getByRole("button", { name: "全データを削除…" }).click();
assert.equal(await page.getByRole("button", { name: "削除する" }).isDisabled(), true);

assert.deepEqual(errors, []);
console.log("ok: 取り込み設定 → 話者 → 移動・要確認 → 修正 → 検索置換 → 定型欄・タグ → 確定・Word・PDF → やり直し → 検索 → 辞書CSV → 録音 → 設定");
await browser.close();
