# 議事録アプリ（開発版）

マイク録音と音声ファイルから、端末内で文字起こし・編集・書き出しを行うMac / Windows向けアプリです。
専門用語は用語辞書に登録できます。医療など特定分野に用途を限定しません。

## 開発と検証

UIのフォルダで実行します。

```sh
cd /Users/keitoozono/localAIproduct/apps/minutes/ui
npm run test:recording
npm run build
npm run tauri -- dev
```

Tauriのコマンドは、必要な設定ファイルがある `src-tauri` を作業場所として起動します。
モデル・要約エンジンが無い新しい環境では、先に `tools/prepare_bundle.sh` の準備が必要です。
このスクリプトはモデルをダウンロードするため通信します。録音は送信しません。

Rustのテスト:

```sh
cd /Users/keitoozono/localAIproduct/apps/minutes/src-tauri
cargo test --lib
```

Macの未署名アプリのビルド:

```sh
cd /Users/keitoozono/localAIproduct/apps/minutes/ui
npm run tauri -- build --bundles app --no-sign
```

出力は `src-tauri/target/release/bundle/macos/minutes.app`。
リリース版では `MINUTES_TIER=pro` は無効です。有料機能には、組み込まれた公開鍵に対応するライセンスが必要です。
現状、本番用公開鍵は未設定なので、未署名ビルドを販売用の完成品として扱わないでください。
開発版では本番鍵なしで機能を検証できます。購入者のデータに試験用ライセンスを登録せず、検証用データを使ってください。

## 利用の流れ

1. 「録音」でマイク録音を開始するか、「録音を取り込む」で音声ファイルを選びます。
2. 録音中の文字は仮の結果です。「止めて保存」のあと、録音全体を再処理します。
3. 音声を聞きながら誤認識や話者名を直します。辞書、議題、決定事項、ToDoも編集できます。
4. 内容を確認して確定し、必要な形式で書き出します。

要約は追加モデルを使う任意の機能です。生成された下書きを確認してから議事録に取り込みます。
ネットワークを使わない設定ではモデルのダウンロードを止め、要約モデルはファイルから取り込めます。
録音・文字起こしには誤りが含まれます。重要な箇所は音声を確認してください。

開発の最新状況は `docs/CODEX_HANDOFF.md`、発売前の確認事項は `checklists/release.md` と `docs/license.md` に記録します。
