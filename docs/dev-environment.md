# 開発・検証環境

## 実機: MacBook Air M2(ユーザー所有)

このクラウド環境(Linux・4 vCPU・外部の配布元に届かない)でできない検証は、MacBook Air M2 にリポジトリを引いて行う。

### Mac で埋められる「未確認」

| 項目 | 内容 |
|---|---|
| ビルド | Tauri 2 の macOS(Apple Silicon)ビルド、`cargo test`(core・各アプリ) |
| モデル取得 | 日本語OCRモデルの公式配布元からの取得とハッシュ照合(出所確認)、文字起こしモデル(kotoba-whisper / Whisper large-v3-turbo / ReazonSpeech 系) |
| 議事録の実測 | `apps/minutes/spike/bench_asr.py` を `testset/` に対して実行し、誤り率・速度・メモリを `docs/spike-results.md` に記入 |
| カメラ | 名刺管理のウェブカメラ撮影を実機で確認 |
| OCR | OS標準OCR(Vision)との比較、実名刺・実領収書(個人情報を伏せたもの)での精度 |

### 注意

- **M2 は最低ライン(Core i5-12500 級・メモリ16GB・GPUなし)より速い。** 速度の合格判定の基準にしない。「Mac(M2)での値」と明記し、最低ライン相当は別途(Windows機、GPUなし・スレッド数6)で測る。
- Windows の確認は RTX 3060 機(GPUを使わない設定)が別に必要。
- 署名・公証は Apple Developer Program への加入が要る(ユーザーが行う)。

### 手順(例)

```
git clone <このリポジトリ> && cd localAIproduct
git checkout claude/zen-franklin-7n1e8j
cd apps/receipt-db/core/engine && cargo test --features onnx
```

結果は各アプリの `docs/spike-results.md` に「Mac(M2)」の行として追記する。
