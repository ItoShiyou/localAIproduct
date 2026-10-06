# minutes の小鳥と動き

2026-10-07追記: 名前は「こより / KOYORI」。現行の性格・色・使い方・六つのしぐさは [KOYORI_BRAND_GUIDE.md](KOYORI_BRAND_GUIDE.md) を基準とする。紹介ページは `character/`、現行の動きは `koyori.js`・`bird-motion.mjs`・`koyori.css`。以下は元絵制作時の記録。

2026-10-06。ノートを抱えて言葉を手元に残す、小さな鳥。淡いセージ、クリーム、アプリコット。既存キャラクターの複製ではなく独自生成。

資産：`web/assets/minutes-bird.png`（透過PNG）。組み込み image_gen で生成、元画像は保存したままプロジェクトへコピー。画像生成の最終プロンプト：

> Use case: illustration-story. Asset type: transparent brand mascot for a Japanese privacy-first local meeting-notes desktop app website. Create one original memorable little bird character lovingly cradling a small closed notebook under one wing, other wing gently extended, looking curious and dependable. Rounded pear-shaped creamy-white body, tiny charcoal dot eyes, tiny apricot beak, short dark twig legs, one pale sage wing. Very simple flat hand-drawn editorial ink illustration, slightly imperfect dark muted forest-gray outlines, soft pastel flat fills, grown-up Japanese stationery warmth, quiet humor, not babyish. Full body centered, generous tight clean silhouette suitable for a hero illustration and small footer mascot. Genuine transparent background, no setting, no shadow rectangle, no text, no letters, no logos, no watermark, no gradients, no 3D, no glossy robot, no sparkles, no lock or medical cross. Bird must be original, not reproduce an existing mascot.

参考：[bondavi](https://bondavi.jp/works)の親しみやすい製品説明、[Apple](https://www.apple.com/jp/macbook-air/)の大きな製品ビジュアルと段階的な説明。独自のページレイアウトとキャラクターとして制作。

実装：マスコットの呼吸、波形、スクロール連動の製品画面の傾きと拡大、段階的な説明の表示、タブ切替のアニメーション。スクロールの強制制御はなし。OSの動きを減らす設定とページ末尾の停止ボタンに対応。JavaScriptが使えなくても内容は表示。

医療・研究での機密会話を冒頭の用途として紹介。実際の音声処理はローカル。モデル取得時の通信、所属先の録音保存ルール、原音確認に必要な説明はデータ案内・FAQへ。制作版・仮称・公開前プレビューの文章は除去。配布前の状態は「近日公開」で表示。

## 表示確認

- PC、390px幅で横のはみ出しなし。小鳥・アプリ画面の画像を表示。
- スクロールで説明が登場し、画面の傾きが0度へ変化。ページ進行表示も更新。
- タブで議事録画面へ切替、右矢印キーで一文編集画面へ切替。
- 動きを停止するとマスコットのanimationがnoneになり、全説明が表示。再開操作も確認。
- ブラウザのエラーログなし。3ページのローカルリンク検査とJavaScript構文確認、差分の空白検査が成功。
- スクリーンショット：作業フォルダーの `minutes-brand-desktop.jpg`、`minutes-brand-mobile.jpg`。
