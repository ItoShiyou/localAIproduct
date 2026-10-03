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

テスト: `core/engine` は 42 件 → 69 件(feature なし)、75 件(`--features onnx`)。既存の 42 件は変更していない(`journal_to_csv` の既存テストはそのまま通る)。

## まだやっていないこと(次の段階)

- `model_manager`、`ui`(TypeScript)は未実装(🔲 のまま)。
- `receipt.rs`・`rules.rs`・`journal.rs` は `src-tauri/` に**コピー**してある。`core/engine/src/lib.rs` から外す変更は、`llm.rs` が `receipt` に依存しているため、`llm` の整理と一緒に行う(別のコア変更として、実施前に記録する)。
- `license` は実接続を作らない(販売プラットフォームのアカウントが必要なため)。既存のトレイトとロジックはそのまま。
