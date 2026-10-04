// 議事録の画面の流れを、モックAPI(ブラウザだけ)で Playwright により確認する。スクリーンショットを docs/ui/ に保存する。
// 取り込み(設定・同意)→ 処理 → 話者(名前の変更・判別し直し)→ 文を押して移動・要確認の巡回 → 修正・結合・分割・元に戻す
// → 検索・置換 → 定型欄・タグ・絞り込み → 要約(追加機能。取得 → 下書き → 中断 → 確認して取り込む)→ 確定 → 書き出し(Word・PDF)→ やり直しの画面 → 検索 → 用語辞書(CSV)→ 設定
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

const browser = await chromium.launch({ executablePath: process.env.CHROMIUM, // --disable-audio-output: 音の出力装置が無い環境(CI・出力先なしの Mac)では AudioContext の時計が止まり、AudioWorklet が呼ばれず録音の音が届かないため
  args: ["--use-fake-ui-for-media-stream", "--use-fake-device-for-media-stream", "--disable-audio-output"] });
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
// 選んだだけでは走らない: ボタン → 設定のダイアログ → 実行
await spk.getByRole("button", { name: "話者を判別し直す…" }).click();
const dd = page.getByRole("dialog", { name: "話者を判別し直す" });
await dd.getByText(/数十秒〜数分かかります/).waitFor();
await dd.getByLabel("話者の人数").selectOption("2");
assert.equal(await page.getByTestId("diar-run").count(), 0, "実行を押すまで始まらない");
await dd.getByRole("button", { name: "実行" }).click();
await spk.getByTestId("diar-run").getByText(/話者を判別しています/).waitFor();
// 処理中も、ほかの操作ができる(参加者の欄を直して、保存済みになる)
await page.getByLabel("参加者").fill("佐藤、鈴木");
await page.getByLabel("参加者").blur();
await page.getByTestId("save-ind").getByText(/保存済み/).waitFor();
assert.equal(await page.getByTestId("diar-run").count(), 1, "他の操作の間も判別は続いている");
// 判別している間は確定できない(終わったときのラベルの更新で下書きに戻ってしまうため)
assert.equal(await page.getByRole("button", { name: "確定", exact: true }).isDisabled(), true, "判別中は確定を押せない");
await page.getByTestId("confirm-why").getByText(/話者を判別している間は確定できません/).waitFor();
await page.getByText("話者を判別し直しました").waitFor({ timeout: 10000 });
assert.equal(await page.getByTestId("diar-run").count(), 0);
assert.deepEqual([...new Set(await speakersOf())].sort(), ["話者1", "話者2"]);
await page.getByRole("button", { name: "元に戻す" }).click();
await page.waitForFunction(() => [...document.querySelectorAll("[data-testid=segment] input[aria-label=話者]")].some((i) => i.value === "佐藤"));
// 判別の中断: 途中で止めると、話者の表示はそのまま・確定を押せる状態に戻る
const before = await speakersOf();
await spk.getByRole("button", { name: "話者を判別し直す…" }).click();
await page.getByRole("dialog", { name: "話者を判別し直す" }).getByRole("button", { name: "実行" }).click();
await spk.getByTestId("diar-run").getByText(/話者を判別しています/).waitFor();
await spk.getByTestId("diar-run").getByRole("button", { name: "中断" }).click();
await page.getByText("話者の判別を中断しました").waitFor({ timeout: 10000 });
assert.equal(await page.getByTestId("diar-run").count(), 0);
assert.deepEqual(await speakersOf(), before, "中断したら話者の表示はそのまま");
assert.equal(await page.getByRole("button", { name: "確定", exact: true }).isDisabled(), false, "中断のあとは確定できる");

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

// 6.5) 入力した直後(入力欄を離す前)に「確定」を押しても、直した文が保存されてから確定される(保存と確定が前後しない)
const raced = "確定の直前に直した文です。";
await segs.nth(0).locator("textarea").fill(raced);
await page.getByRole("button", { name: "確定", exact: true }).click();
await page.locator(".d-actions .badge.ok", { hasText: "確定" }).waitFor({ timeout: 5000 });
await page.getByText("確定しました。書き出せます").waitFor();
await page.getByTestId("save-ind").getByText(/保存済み/).waitFor();
assert.equal((await segTexts())[0], raced, "直した文が残る");
assert.equal(await page.locator(".d-actions .badge.ok").innerText(), "確定", "下書きに戻っていない");
// 確定したあとに直すと下書きに戻る。そのことを知らせる
await segs.nth(0).locator("textarea").fill(raced + "追記");
await segs.nth(0).locator("textarea").blur();
await page.getByText("内容を直したため下書きに戻りました").waitFor();
await page.locator(".d-actions .badge.muted", { hasText: "下書き" }).waitFor();
await shot("4b-draft-toast");

// 6b) 要約(追加機能。モックのモデル): 未取得 → 設定で取得 → 下書き → 中断 → 確認して取り込む(確認するまで保存されない)
await page.getByRole("tab", { name: /要約/ }).click();
const sum = page.getByTestId("summary");
await sum.getByTestId("summary-note").getByText("要約は自動で作った下書きです。内容を確認してから使ってください。").waitFor();
await sum.getByTestId("summary-nomodel").waitFor();
assert.equal(await sum.getByTestId("summary-run").count(), 0, "モデルが無ければ要約は始められない");
await sum.getByRole("button", { name: "設定を開く" }).click();
const sm = page.getByTestId("summary-model");
await sm.getByText("Apache-2.0").first().waitFor();
await sm.getByRole("button", { name: "取得する" }).click();
await sm.getByTestId("summary-model-progress").waitFor();
await sm.getByText("取得済み").waitFor();
await shot("7-summary-settings");
await page.getByRole("tab", { name: "議事録" }).click();
await page.getByRole("tab", { name: /要約/ }).click();
// 中断できる
await page.getByTestId("summary-run").click();
await page.getByTestId("summary-progress").waitFor();
await page.getByTestId("summary-cancel").click();
await page.getByTestId("summary-error").getByText("要約を中断しました").waitFor();
assert.equal(await page.getByTestId("summary-draft").count(), 0, "中断したら下書きは出ない");
// もう一度作る → 下書き(チェック付き)。ここまでメモは変わっていない
await page.getByTestId("summary-run").click();
const dr = page.getByTestId("summary-draft");
await dr.waitFor({ timeout: 10000 });
await shot("8-summary-draft");
assert.equal(await dr.getByLabel("要点を取り込む").count(), 2);
assert.equal(await dr.getByLabel("ToDo を取り込む").count(), 2);
await page.getByRole("tab", { name: /議題・決定事項・ToDo/ }).click();
assert.equal(await page.getByTestId("notes").getByLabel("議題").inputValue(), "見積もり\n展示会の準備", "確認するまでメモは変わらない");
await page.getByRole("tab", { name: /要約/ }).click();
await dr.waitFor(); // タブを切り替えても下書きは残る
// 直す・外す: 2つ目の要点は取り込まない、ToDo の担当を直す
await dr.getByLabel("要点を取り込む").nth(1).uncheck();
await dr.getByLabel("担当").first().fill("鈴木");
await dr.getByLabel("決定事項", { exact: true }).first().fill("ブースの設営は外部に依頼する(確認済み)");
await dr.getByTestId("summary-import").click();
await dr.getByTestId("summary-confirm").getByText("選んだ 4 件").waitFor();
await dr.getByTestId("summary-confirm").getByRole("button", { name: "やめる" }).click();
await page.getByRole("tab", { name: /議題・決定事項・ToDo/ }).click();
assert.equal(await page.getByTestId("notes").getByLabel("議題").inputValue(), "見積もり\n展示会の準備", "やめたら保存されない");
await page.getByRole("tab", { name: /要約/ }).click();
await dr.getByTestId("summary-import").click();
await dr.getByTestId("summary-confirm-yes").click();
await page.getByText(/4 件をメモに追加しました/).first().waitFor();
await dr.waitFor({ state: "detached" }).catch(() => undefined);
await page.getByRole("tab", { name: /議題・決定事項・ToDo/ }).click();
const n2 = page.getByTestId("notes");
const agenda = await n2.getByLabel("議題").inputValue();
assert.match(agenda, /^見積もり\n展示会の準備\n新しい見積もりについて/, "要点は議題の末尾に足される");
assert.ok(!agenda.includes("ブースの設営は外部に依頼済み"), "チェックを外した要点は入らない");
assert.match(await n2.getByLabel("決定事項").inputValue(), /^パンフレットは五百部\nブースの設営は外部に依頼する\(確認済み\)$/);
const todoTexts = await n2.getByPlaceholder("やること").evaluateAll((els) => els.map((e) => e.value));
assert.deepEqual(todoTexts, ["入稿", "見積もりの回答内容を共有する(期限: 来週の月曜日まで)", "展示会の準備状況を確認する"], "日付でない期限は文に添える");
assert.deepEqual(await n2.getByPlaceholder("担当").evaluateAll((els) => els.map((e) => e.value)), ["鈴木", "鈴木", ""]);
assert.equal(await n2.getByLabel("期限").last().inputValue(), "2026-11-01");
await page.getByRole("tab", { name: "編集" }).click();

// 7) 確定 → 書き出し(Word・PDF)
assert.equal(await page.getByLabel("書き出し").isDisabled(), true, "確定前は書き出せない");
await page.getByRole("button", { name: "確定", exact: true }).click();
await page.getByText("確定しました").waitFor();
await page.getByLabel("書き出し").click();
await page.getByRole("menuitem", { name: /Word/ }).click();
await page.getByText("書き出しました").waitFor();
assert.equal(await page.getByRole("menu", { name: "書き出す形式" }).count(), 0, "選ぶと閉じる");
await page.getByLabel("書き出し").click();
await page.keyboard.press("Escape");
assert.equal(await page.getByRole("menu", { name: "書き出す形式" }).count(), 0, "Esc で閉じる");
await page.getByLabel("書き出し").click();
await page.mouse.click(5, 5);
assert.equal(await page.getByRole("menu", { name: "書き出す形式" }).count(), 0, "外を押すと閉じる");
await page.getByLabel("書き出し").click();
await page.getByRole("menuitem", { name: /PDF/ }).click();
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
console.log("ok: 取り込み設定 → 話者 → 移動・要確認 → 修正 → 検索置換 → 定型欄・タグ → 要約 → 確定・Word・PDF → やり直し → 検索 → 辞書CSV → 録音 → 設定");
await browser.close();
