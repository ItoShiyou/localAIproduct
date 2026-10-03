# Mac(MacBook Air M2)での実証 — Claude Code への指示書

このファイルを読んで、上から順に進めてください。クラウド環境ではできなかった「実機・実モデル」の検証を、この Mac で行います。

## 前提と守ること

- 先に `CLAUDE.md`(各アプリ内)と `docs/dev-environment.md` を読む。
- **M2 は最低ライン機より速い。** 速度は「Mac(M2)での値」と明記し、合格判定の基準にしない。
- 数値を捏造しない。測れなかったものは「未実測」と書く。
- モデルファイル・音声・画像などの大きいファイルは **コミットしない**(`.gitignore` に入れる)。
- 実在の他人の個人情報(名刺・領収書・録音)を使わない。ユーザーが提供した場合は `testset-real/`(`.gitignore` 済み)に置き、リポジトリに入れない。
- 作業ブランチ: `claude/zen-franklin-7n1e8j` から `mac/verify` を切って作業・push する(元のブランチへ直接 push しない)。
- 止まって確認すること(実行しない): 価格、販売登録、署名証明書・Apple Developer 登録、アカウント作成、ユーザーデータを送る通信の追加。

## 準備

```
xcode-select --install          # 未導入なら
curl https://sh.rustup.rs -sSf | sh   # Rust(未導入なら)
brew install node               # Node.js LTS(未導入なら)
git checkout -b mac/verify
```

## 段階1: ビルドとテスト(Mac 初回確認)

1. `apps/receipt-db/core/engine` で `cargo test` と `cargo test --features onnx`。
2. `apps/receipt-db/src-tauri` と `apps/bizcard/src-tauri` で `cargo test`。
3. `apps/bizcard/ui` で `npm ci && npm run build`。
4. 失敗したものは原因を調べ、Mac 固有の問題(onnxruntime の読み込み、パス、依存)なら修正してコミット。結果は各アプリの `docs/spike-results.md` に「Mac(M2)」の行として追記。

## 段階2: 日本語OCRモデルの出所確認

1. `apps/receipt-db/docs/licenses.md` の「japan_PP-OCRv3 の出所が未確認」を読む。
2. 公式配布元(PaddleOCR / RapidOCR の公式一覧)から日本語認識モデルを取得し、現在使っている再配布版(npm `multilingual-purejs-ocr`)とハッシュを照合。一致/不一致、ライセンス(Apache-2.0 か)を `docs/licenses.md` に根拠URLつきで記録。
3. 公式版が使えるなら、モデルの取得手順を `docs/` に書き、`core::ocr` の読み込みパスを合わせる(core 変更は `core:` コミット+`docs/core-changes.md`)。
4. 可能なら PP-OCRv5 の日本語認識も同じテストセットで比較(会社名の精度が上がるか)。

## 段階3: 名刺管理の実機確認

1. Tauri 2 の殻(`tauri.conf.json`、`main.rs`)を作り、`ui/` と `src-tauri/src/tauri_glue.rs` を接続して `cargo tauri dev` で起動。
2. Mac のカメラで名刺(ユーザーが用意した架空/自分の名刺)を撮影 → 下部の確認枠 → 打ち替え → 確定 → 検索、を実際に通す。スクリーンショットを `apps/bizcard/docs/ui/mac-*.png` に保存。
3. ユーザーが実名刺を提供した場合のみ、`testset-real/` で精度を測る(結果の数値だけを記録)。

## 段階4: 議事録の文字起こし実測

1. `apps/minutes/spike/bench_asr.py` を、`apps/minutes/testset/` に対して実行(候補: kotoba-whisper v2.0、Whisper large-v3-turbo、ReazonSpeech 系)。Python 環境は venv を作る。
2. 誤り率(文字単位)、再生時間比、ピークメモリ、モデル容量を表にして `apps/minutes/docs/spike-results.md` に追記。スレッド数は 6 に絞った測定も併記(近似)。
3. ReazonSpeech 系と kotoba-whisper の商用可否は、モデルカード原文を読んで `docs/licenses.md` に引用つきで記録。

## 終わったら

- `git push -u origin mac/verify`
- 結果を短く報告: 通ったもの、通らなかったもの、未実測、ユーザーの判断が要る点。
