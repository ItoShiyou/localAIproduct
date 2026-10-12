# 販売・ダウンロードの現在地（2026-10-13）

一般向けMac App Store公開: **未実施**。TestFlightからのインストール: **未確認**。正式署名済みパッケージは生成したが、販売中・ダウンロード可能とは案内しない。

## Apple側の準備と実行済み工程

- 所有者の本人認証後、App ID dev.localaiproduct.minutesを所有者がRegisterで登録。In-App Purchaseのみ有効。
- 所有者の実行時承認後、既存Mac App Distribution証明書でminutes Mac App Store配布プロファイルを生成・取得。Bundle ID・期限・Team ID・証明書の指紋一致を確認。秘密鍵は出力していない。
- App Store Connectの名称minutesは使用済みだったため、所有者承認の「minutes - ローカル議事録」で登録。Apple ID 6822096105、SKU minutes-macos、日本語・macOS。
- 非消耗型minutes Proを登録。製品ID dev.localaiproduct.minutes.pro、Apple ID 6822096405。日本語名・説明、日本を基準に3,980円の価格ポイントを設定。
- 所有者の選択に従いProの配信地域を日本のみに設定。画面の選択数1と再読込後の保持を確認。本体の無料価格・日本限定配信設定はまだ未確認。
- バージョン0.1.0の日本語説明・キーワード・著作権・審査説明・連絡先を保存。手動リリースを選択。電話番号はAppleの審査フォームにのみ入力し、本資料・公開ページ・Gitには記載しない。
- 合成会議の実GUIスクリーンショット3枚をアップロードし、3/10を確認。Pro審査用の編集画面も追加。
- 既存Application/Installer証明書でminutes.pkgを生成。codesign deep/strict・pkgutil署名検証成功。
- 最初のTransporter検証はLSApplicationCategoryType不足（90242）で失敗。Mac専用構成へProductivityを追加し、署名前の必須検査を追加。試験6件成功、ソースから再ビルド・正式再署名した。
- 修正版はartifacts/minutes-mas-2026-10-13-category/minutes.pkg。Transporterで旧パッケージを置き換え、検証・アップロードのデリバリを開始した。完了・TestFlight配布はまだ確認していない。
- App Store Connectの後続操作は、自動承認サービスがタイムアウトし、許可された1回の再試行でも利用上限により失敗した。未承認操作は実行せず、迂回していない。

## 開発・検証

Mac専用StoreKit、購入復元・取引更新、App Sandbox、マイク・利用者選択ファイル・印刷権限、要約子プロセスのinherit署名を実装。macOS 12向けONNX Runtimeをテレメトリーなしで再構築。Windows資産は保持。

オフライン音声処理、限定PIDの通信観測、合成30分音声の中断再開、実容量不足からの復元を確認。実GUIで文字起こし・話者判別・要約下書き・編集・検索・全形式の書き出しを確認。最新のRust試験91件成功。StoreKit独立ローカル試験は成功したが、実アプリのApple Sandbox/TestFlight新規取引とは区別する。

起動時のキーチェーン許可待ち、価格取得失敗表示、PDF保存権限、長時間WAVの全件メモリ保持を修正した。要約は認識誤り・決定事項欠落の例があり、確認・修正が必要。

## 残作業

1. Transporterの修正版デリバリ結果を確認し、Appleが指摘する問題があれば修正する。
2. App Store Connectで本体の無料価格・日本限定配信、サブタイトル・カテゴリ、プライバシー回答などの残る申請情報を設定する。
3. 有料契約・銀行・税務等の状態を本人が確認し、必要な同意・入力を行う。契約への同意は代行しない。
4. TestFlightビルド処理・配布を設定し、実機でインストール、購入・復元・返金・キャンセル・承認待ち・オフライン再起動を検証する。
5. macOS 12実機、実マイク長時間、OSマイク拒否・ファイルアクセス拒否、旧データ移行GUI、各操作の通信・プライバシー最終照合を確認する。
6. サポート・プライバシーポリシーのHTTPSページを公開して到達確認し、申請URLへ登録する。
7. 最終審査提出・公開は所有者承認後。現在は手動リリース設定。

詳細: build-and-submit.md / qa-matrix.md / gui-verification-2026-10-12.md / verification-2026-10-12.md。

## 追加のプライバシー提出準備

PrivacyInfo.xcprivacyをMac App Store版のContents/Resourcesへ同梱し、現在の収集しない方針との整合性を署名前に検査する。ビルド番号2でソースからの生成に成功した。ビルド1のTransporterは所有者から「アップロード・検証中」と報告を受けた。自動承認の利用上限が解消するまでは、こちらで完了を確認したとは扱わない。ビルド2の正式署名パッケージをartifacts/minutes-mas-2026-10-13-build2/minutes.pkgへ生成した。署名工程・パッケージ署名検査は成功し、ビルド番号2・カテゴリ・プライバシーマニフェストの実同梱を確認した。提出前検査の試験7件が成功。ビルド2は未アップロード。Gitへのpushは行わず、ローカルコミットを保存する。
