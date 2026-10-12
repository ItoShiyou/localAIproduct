# minutes | Mac App Storeリリース指示（2026-10-12）

## 今回の決定（旧factory計画より優先）
ユーザーがApple Developer Program登録完了を報告（有効化状態は未照会）。
**Apple向けアプリは公式Mac App Storeからのみダウンロード。** 購入はApple公式決済。無料版＋買い切りProが基本案で、Appleの非消耗型アプリ内購入を利用する。
**Mac版にPolar / Lemon Squeezy / 独自ライセンスキー / DMG直販 / Developer ID公証フローは使用しない。**
本repoの `docs/sales-platforms.md` と `apps/minutes/SPEC.md` は旧直販構想を含む**歴史的資料**。当該資料の全体を勝手に書き換えず、Mac App Store版の判断には本書を優先する。Windows配布はApple経由ではできないので**別途方針決定まで保留**。

## 重要な実装上の前提
このリポジトリにある `apps/minutes` は初期のSPEC・スパイク・検証資産が中心。**Codex Desktopにある最新のminutes本体を探し、実際に確認したものを基準にすること。** 現在の録音、文字起こし、要約、書き出し等の実装範囲は本リポジトリの旧仕様から確定しない。

## Codexで可能なタスク
- [ ] 最新のminutes本体パス、ソース管理、コミット、macOSターゲット、OS要件、ビルド/テスト方法を特定。未pushファイルと古い資料を混同しない。
- [ ] 実在する機能のE2Eテスト：音声入力、処理進捗、日本語文字起こし、音声との突合、修正、書き出し、終了/再開、長時間、権限拒否、破損入力、容量不足、オフライン。
- [ ] ローカル処理の証明：音声・転写文・議事録・メタデータを外部送信しないことを通信ログなどで実証。ライセンスIAP通信、モデルダウンロード、アップデートの例外は利用者にわかるよう分けて記載。
- [ ] App Sandboxの権限とユーザー選択ファイル、マイク・録音関連の許可、子プロセスとモデルの同梱方法/保存先を検証。Sandbox外でしか動かない構成は審査提出しない。
- [ ] 日本語文字起こしの再現可能な評価（誤り率・速度・ピークメモリ）を実測。認識モデル・ノイズ除去モデル・ffmpeg等のライセンスとApple Storeへの再配布条件を一次資料で確認。不明なら出荷ブロック。
- [ ] 現在の無料版/Pro版を実装と照合し、StoreKitの**non-consumable** IAPで永久解除。Appleの検証済み取引を使い、初回購入・キャンセル・復元・返金/取り消し・オフライン権利反映を検証。無料版上限と販売価格は**未最終決定**。
- [ ] App Store向けArchive/配布署名・App Store Connectアップロード・TestFlight・申請手順を整備。**Developer ID公証やDMG配布はApp Store用工程ではない**。
- [ ] App Store審査用のサンプルデータ（架空の会議音声）、操作説明、プライバシーポリシー・サポート、App Privacy申告、スクリーンショット、暗号化輸出申告、ライセンス表記を準備。
- [ ] Mac初回起動・購入の復元・最低動作環境での実機検証。外部テスターの反応は実際に受け取った時だけ完了にする。

## 本人の操作・確認（Codexはパスワードや2FAを受け取らない）
- [ ] App Store Connectの有料アプリ契約への同意、税務フォーム、銀行口座。Small Business Programは別途申請。
- [ ] macOSアプリレコード：正式名・主要言語日本語・Bundle ID・SKU・製品ID、Pro価格と地域設定を決める。既存IDがあれば変えない。
- [ ] Appleアカウント認証と証明書の安全な管理、TestFlightの配布操作、審査・公開の最終承認。

## 仮の日本語App Store説明（レビュー前に実機照合が必要）
「会議や面談の音声を、手元のMacで文字に。minutesは、クラウドへの録音データ送信を避けたい人に向けたローカル議事録アプリです。音声を確認しながら文字を修正し、記録として残すことを目指しています。文字起こしには誤りが含まれるため、重要な箇所は原音でご確認ください。」
**注意**：録音ファイル処理、リアルタイム録音、AI議事録、Pro範囲などの最新実装確認までは、この文案を正式製品説明として公開しない。

## リリース判定
- [ ] 正規の現行ソースの場所、Version、Bundle ID、実測テストログ
- [ ] App SandboxとMac App Store用署名での実機成功
- [ ] StoreKit SandboxでProの購入・取り消し・復元成功
- [ ] Apple配布ビルドのアップロードとTestFlightテスト成功
- [ ] App Store Reviewの承認（承認以前に「販売開始」とは書かない）

## Apple公式
- https://developer.apple.com/documentation/security/app_sandbox
- https://developer.apple.com/help/app-store-connect/manage-agreements/sign-and-update-agreements/
- https://developer.apple.com/help/app-store-connect/manage-banking-information/enter-banking-information
- https://developer.apple.com/help/app-store-connect/manage-tax-information/provide-tax-information/
- https://developer.apple.com/help/app-store-connect/manage-in-app-purchases/create-consumable-or-non-consumable-in-app-purchases/
- https://developer.apple.com/help/app-store-connect/test-a-beta-version/testflight-overview/
