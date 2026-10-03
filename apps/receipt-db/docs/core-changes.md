# core/ の変更記録

領収書DBは共通コアを作る役なので、`core/README.md` の 🔲 の部品を `core/` に実装している(KICKOFF.md の例外)。
`core/` を変えるコミットは、先頭に `core:` を付け、アプリ固有の変更と分けている。ここに、変更の内容と理由を書く。

## 2026-10-03

| 部品 | 変更 | 理由 |
|---|---|---|
| `ocr`(新規) | `Ocr` トレイト、型 `BBox` / `OcrLine` / `OcrPage` / `OcrImage`、`FakeOcr`(テスト用)。`OcrPage::to_text()`(読み順の本文)、`to_alt_text()` | 名刺管理(`apps/bizcard/src-tauri/src/ocr_input.rs`)の「行・位置・縦書きフラグ・信頼度」と同じ形にした(座標は左上原点のピクセル、縦書きは `bbox.w` が文字の大きさ)。`alt_text`(併用モデルの読み取り)だけ追加。名刺管理は `serde` の既定値で読めるので、変換は不要のはず(要確認) |
| `ocr::ppocr`(新規、feature `onnx`) | ONNX Runtime 上の PP-OCR(検出 + 認識、2つ目の認識モデルを併用可)。画像処理は `image` のみ、検出領域は主成分で向きを求めて水平に切り出す。縦書き判定 | スパイクで選んだ方式(`docs/spike-results.md`)。`ort` は `load-dynamic` なので、onnxruntime の共有ライブラリは実行時に読み込む。`onnx` 機能を切ればビルドに ONNX は不要 |
| `ocr::tesseract`(新規) | Tesseract の実行ファイルを標準入力経由で呼ぶ実装(TSV から行・位置・信頼度)。`OMP_THREAD_LIMIT=1` を付ける | 予備の方式。一時ファイルを作らない |
| `db`(新規) | `Db`(開く・`user_version` によるマイグレーション・新しい版のDBは開かない)、`search_rowids`(3文字以上は FTS5 trigram、3文字未満は LIKE)、`fold_for_search`(NFKC) | 2文字の語が trigram で 0 件になるため(実測)。判断は利用者の指示 |
| `export` | 汎用の `write_csv(header, rows, encoding)` と `Cell`(文字列は無害化、整数はそのまま)を追加。`journal_to_csv` は `write_csv` を呼ぶ形にして互換を保つ | `core/README.md` の「汎用の write_csv に切り出す」。判断は利用者の指示 |
| `error` | `CoreError::Ocr`、`CoreError::Db` を追加 | 新しい部品のため |
| `Cargo.toml` | `image`、`rusqlite`(bundled)、`unicode-normalization`、`ort`・`ndarray`(feature `onnx`、任意)を追加 | 同上。ライセンスは `docs/licenses.md` |

| `jobs`(新規) | `JOBS_SCHEMA`(アプリのマイグレーションに追加するSQL)、`Jobs`(enqueue / run_pending / recover / retry_failed / cancel_pending / summary)、`CancelToken` | 状態をDBに保存して中断・再開。1件の失敗(panic含む)で止めない。スレッドは作らず、呼び出し側が実行する |
| `settings`(新規) | `AppData`(データフォルダ、目印ファイル、`settings.json` の読み書き、`network_list`、`update_check_allowed`、`delete_all`)、`NetworkEntry` | 通信一覧の表示、更新確認の停止、全データ削除。目印が無いフォルダは消さない |
| `logging`(新規) | `Logger`(固定文字列と数値だけ書ける、サイズ上限で1世代退避) | 個人データをログに書けない作りにした |

### 縦書きの改善(`ocr::ppocr`、bizcard の依頼)

- 原因: 縦書きの領域をすべて「読む方向に回して」読んでいたため、正立のまま縦に積まれた日本語が横倒しの文字として認識に渡されていた。加えて、ほぼ縦の領域で向きの符号の誤差から上向きになり、英数字の縦書き行が上下逆さまに読まれることがあった。
- 変更: (1) 向きの正規化を修正(縦に近い領域は必ず下向き)。(2) 縦書きの領域は「回して読む」と「正立の文字を1字ずつ切り出して横に並べて読む(`stacked_strip`)」の両方を試し、信頼度が高いほうを採る。回した結果が英数字中心(ASCII 60%以上)で信頼度 0.8 以上なら、メール・電話を壊さないため回した結果を採る。(3) 切り出しは、余白を除いた墨の範囲から文字数(長さ/幅)を求め、等分位置の近くの墨量が最少の行で切る。余白は文字幅の15%。2つ目の認識モデルにも同じ画像を渡す。
- 公開 API の変更なし(`DetBox`、`rotated_crop` は従来どおり。`stacked_strip`、`ascii_ratio` を追加)。
- 測定(bizcard の縦書き8枚 = 05・06・17・25 の a/b、`PpOcr` 日本語 v3 + 中国語 v4 併用、2スレッド): 氏名の完全一致 2/8 → 8/8、会社 0/8 → 2/8(文字の類似度の平均 0.53 → 0.67)、役職 1/8 → 3/8(0.21 → 0.68 付近。通常の結果と併用側の良いほうの行で採点)、メール 1/6 → 5/6、電話 6/8 → 8/8(メール・電話は併用モデルの結果で採点)。receipt-db の合成50件は変更前と同じ(日付・金額・税率・登録番号 50/50、支払先 48/50)。
- 限界: 会社名・役職は仮名や小さい文字の読み違いが残る(日本語 v3 モデルの限界とみられる)。合成画像の縦書きのみで、実際の縦書き名刺は未検証。処理時間は縦書き領域が増えると認識の回数が増える(この共有マシンは負荷が高く、時間の再測定は不正確。縦書き8枚で中央値 約0.6秒)。

テスト: `core/engine` は 42 件 → 69 件(feature なし)、78 件(`--features onnx`)。既存の 42 件は変更していない(`journal_to_csv` の既存テストはそのまま通る)。

## まだやっていないこと(次の段階)

- `model_manager`、`ui`(TypeScript)は未実装(🔲 のまま)。
- `receipt.rs`・`rules.rs`・`journal.rs` は `src-tauri/` に**コピー**してある。`core/engine/src/lib.rs` から外す変更は、`llm.rs` が `receipt` に依存しているため、`llm` の整理と一緒に行う(別のコア変更として、実施前に記録する)。
- `license` は実接続を作らない(販売プラットフォームのアカウントが必要なため)。既存のトレイトとロジックはそのまま。
