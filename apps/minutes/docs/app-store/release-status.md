# 販売・ダウンロードの現在地

一般向けMac App Store公開: **未実施**。TestFlight配布: **未実施**。ローカルの開発用署名アプリを、正式販売版や公開済みダウンロードとして案内しない。

## 完了した開発・検証

- Mac専用StoreKit非消耗型Pro、復元・取引更新。独立したAppleローカル試験あり。実アプリのローカルキャッシュ権利でProの音声処理・要約・編集・全形式の書き出しを確認。
- App Sandbox・音声入力・利用者選択ファイル・印刷権限。要約子プロセスのinherit署名。macOS 12向けONNX Runtimeをテレメトリーなしで再構築。
- オフライン音声処理、限定したPIDの通信観測、合成30分音声の中断再開、容量不足の復元。
- 起動時のキーチェーン許可待ち、価格取得失敗表示、PDF保存権限、長時間WAVの全件メモリ保持を修正。
- 日本語申請文・ライセンス表記・アイコン・実GUIスクリーンショットを用意。Windowsの既存資産は保持。

## 開発側に残る確認

- macOS 12実機、実マイク長時間、OSマイク拒否・ファイルアクセス拒否、旧データ移行の実GUI。
- 要約の認識誤り・決定事項欠落を含む品質評価。下書きの人による確認は必須。
- 正式署名後のApple Sandbox/TestFlightで新規購入、復元、返金、承認待ち、購入キャンセル、オフライン再起動。
- アプリ各操作の通信とプライバシー説明の最終照合。

## 所有者による準備が必要な箇所

1. Apple Developer/App Store ConnectでminutesのBundle IDとアプリを登録・照合する。現コードはdev.localaiproduct.minutes、非消耗型Proはdev.localaiproduct.minutes.pro。既存登録と異なる場合はビルド前に揃える。
2. 必要な有料契約・銀行・税務情報を本人が確認する。価格は承認済み3,980円、無料版は累計60分・1件15分。
3. Mac App Store Application/Installer証明書は既存のものを読み取り専用確認で検出した。標準のプロファイル保存場所にはminutes用の一致する配布プロファイルが見つからなかった。DeveloperのProfilesでMac App Store Connect用を作成し、明示App ID dev.localaiproduct.minutesと既存Application証明書を選択してダウンロードする。秘密鍵や認証情報をチャット・リポジトリへ貼らない。

   Apple公式手順: https://developer.apple.com/help/account/provisioning-profiles/create-an-app-store-provisioning-profile
4. 署名済みpkgをアップロードし、TestFlight試験を実施する。最終審査提出・公開は所有者承認後。
5. サポート・プライバシーポリシーのHTTPSページを公開し、申請URLへ登録する。準備したローカルHTMLは公開済み扱いにしない。

署名コマンドと提出手順: build-and-submit.md。詳細な実施範囲: qa-matrix.md / gui-verification-2026-10-12.md / verification-2026-10-12.md。

「実装済み」「開発用ビルド生成済み」「App Store販売中」は異なる状態として記録する。
