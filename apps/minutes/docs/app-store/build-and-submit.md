# Mac App Storeビルドと提出

元のWindows・直接販売用コードを保持し、mac-app-store feature とtauri.mas.conf.jsonを併用する。edition-free / edition-pro との併用はビルドエラー。製品ID候補はInfo.mas.plistに設定する。正式Bundle ID・SKU・製品IDは所有者がApp Store Connect登録時に照合する。

```
CARGO_NET_OFFLINE=true VITE_MINUTES_CHANNEL=mac-app-store npm --prefix apps/minutes/ui run tauri -- build --features mac-app-store --config tauri.mas.conf.json --bundles app
python3 tools/minutes_app_store.py --app <生成したminutes.app> --output <新規出力ディレクトリ>
```

App Sandbox、マイク、利用者が選択したファイルの読み書き、モデル取得用クライアント通信を宣言。データ・モデルはTauriのapp_data_dir（Sandboxコンテナ内）に保存。選択ファイルはアプリ内へコピーするため、再開に元ファイルの永続アクセス権は不要。任意モデルは実行コードではなくデータのみで、サイズ・SHA-256を照合する。外部購入ページはMac版で拒否する。主要処理は同梱モデルを使える。

要約はContents/MacOSの同梱子プロセス。Sandboxとinheritを持つ専用Entitlementsで先に署名する。ONNX dylib等のMach-Oを個別署名してから本体を署名。Mac版は実行ファイル・モデルの環境変数差し替えを無効にする。アプリ内で任意実行コードを取得しない。

所有者が用意する項目: App Store配布用Application証明書、Installer証明書、配布用プロファイル、有料契約・銀行・税務・App Record・非消耗型Pro商品・承認済み価格と無料上限。

署名ツールは既存キーチェーンの証明書名をApple codesignに渡す。秘密鍵を出力・コピーしない。--identity にApple Distributionまたは3rd Party Mac Developer Application、--installer-identity に3rd Party Mac Developer Installer、--profile にApp Store配布用プロファイルを指定。必要な本人操作を済ませ、QA・モデル権利・販売仕様の承認記録ができてから --release-approved --execute を使う。通常のDeveloper ID公証DMGを提出物にしない。

生成pkgをAppleのTransporterでアップロードし、App Store Connectの処理結果を確認する。アップロードだけで販売完了としない。TestFlightで別のMacへのインストール、初回権限、購入、復元、返金取消、再起動、オフライン権利を確認。StoreKitローカル構成だけの試験はSandbox購入の代わりにならない。

提出前: 説明・スクリーンショット・サポートとポリシーURL・価格と無料上限・年齢区分・輸出コンプライアンス・プライバシー回答を確定する。最終審査提出・公開は所有者の承認後。

一次情報: https://developer.apple.com/app-store/review/guidelines/ ; https://developer.apple.com/documentation/security/app-sandbox ; https://developer.apple.com/documentation/storekit/transaction/currententitlements ; https://developer.apple.com/help/app-store-connect/test-a-beta-version/testflight-overview/

## 実行できるローカルStoreKit検証

```
xcodebuild -project apps/minutes/qa/StoreKitQA.xcodeproj -scheme MinutesStoreKitQA -derivedDataPath /tmp/minutes-storekit-xcode test
```

独立したテストホストで本番と同じStoreKitBridge.swiftをコンパイルする。Minutes.storekitの0.99はローカル試験専用で、正式価格ではない。購入・復元・返金をXCTestで検証。ダイアログがテスト環境によりキャンセルされた場合は結果を記録し、公式SKTestSessionで模擬購入を作る。TestFlight・Apple Sandboxの実取引とは区別する。

## macOS 12向け話者エンジンの再構築

公式v1.30.0ソースをlegal/onnxruntime-macos12-build.jsonのrevisionへ固定し、ソース内に隔離Python `.venv`を作成後、`apps/minutes/tools/build_ort_macos12.sh <絶対ソースパス> <絶対ビルドパス>`を実行する。CPU版、arm64、deployment target 12.0、テレメトリーをコンパイルから除外する。生成dylibを別プロセスでWeSpeakerと検証し、vtool・otool・Sandbox試験後にresources/onnxruntime/libonnxruntime.dylibへ実ファイルとして配置する。Windows DLLは変更しない。vtoolで14.0を12.0へ書き換えるだけの方法は採用しない。

旧データを移行する際は旧アプリを終了し、新アプリの議事録が空の状態で「設定」の旧データ移行を選ぶ。以前のminutes保存フォルダーをシステム画面で明示選択する。コピー後にアプリが終了するので、再度起動して移行結果を確認する。元データは保持する。移行後の一覧・音声・編集内容を確認するまで、旧フォルダーを削除しない。
