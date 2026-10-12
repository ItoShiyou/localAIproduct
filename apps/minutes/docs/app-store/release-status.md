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

1. Apple Developer/App Store ConnectでminutesのBundle IDとアプリを登録・照合する。dev.localaiproduct.minutesは2026-10-13に所有者がRegisterを押し、DeveloperのIdentifiers一覧で登録を確認した。App Store Connectのアプリ登録は別途必要。非消耗型Proはdev.localaiproduct.minutes.pro。
2. 必要な有料契約・銀行・税務情報を本人が確認する。価格は承認済み3,980円、無料版は累計60分・1件15分。
3. Mac App Store Application/Installer証明書は既存のものを読み取り専用確認で検出した。標準のプロファイル保存場所にはminutes用の一致する配布プロファイルが見つからなかった。DeveloperのProfilesでMac App Store Connect用を作成し、明示App ID dev.localaiproduct.minutesと既存Application証明書を選択してダウンロードする。秘密鍵や認証情報をチャット・リポジトリへ貼らない。

   Apple公式手順: https://developer.apple.com/help/account/provisioning-profiles/create-an-app-store-provisioning-profile
4. 署名済みpkgをアップロードし、TestFlight試験を実施する。最終審査提出・公開は所有者承認後。
5. サポート・プライバシーポリシーのHTTPSページを公開し、申請URLへ登録する。準備したローカルHTMLは公開済み扱いにしない。

署名コマンドと提出手順: build-and-submit.md。詳細な実施範囲: qa-matrix.md / gui-verification-2026-10-12.md / verification-2026-10-12.md。

「実装済み」「開発用ビルド生成済み」「App Store販売中」は異なる状態として記録する。

## 2026-10-13の追加作業

所有者の本人サインイン後、minutesの明示App ID（アプリ内購入のみ）を登録確認した。Mac App Store Connect用プロファイルを既存Mac App Distribution証明書で生成直前まで準備した。所有者の実行時承認後に生成・ダウンロードし、Bundle ID・有効期限・Team ID・選択済み証明書のSHA-1一致を確認した。秘密鍵は出力していない。正式署名パッケージ生成を実行中。正式署名ツールへ期限切れ・開発用・デバイス限定・別アプリ・Team ID不一致のプロファイル拒否を追加し、合成メタデータによる5件の試験が成功した。署名情報・認証鍵の拡張子をGit対象から除外した。


## Apple側で完了した準備（2026-10-13）

- App ID dev.localaiproduct.minutesを登録、App Store配布プロファイルを生成・ダウンロード・照合。
- アプリ名「minutes」は使用済みのため、所有者承認の「minutes - ローカル議事録」でApp Store Connectへ登録（Apple ID 6822096105、SKU minutes-macos、日本語、macOS）。
- 非消耗型minutes Proを製品ID dev.localaiproduct.minutes.proで登録（Apple ID 6822096405）。価格・配信可否・ローカライズの設定を継続中。
- バージョン0.1.0の日本語説明・キーワード・著作権・審査説明・連絡先を保存、手動リリースを選択。審査連絡先の電話番号はAppleフォームにのみ入力し、この記録へ残さない。
- 合成会議の実GUIスクリーンショット3枚をアップロードし、画面の3/10表示を確認。
- 既存Application/Installer証明書で署名したminutes.pkgを生成。codesignのdeep/strict検証、pkgutilの署名検査が成功。成果物はartifacts/minutes-mas-2026-10-13-app-store/。秘密鍵は出力していない。
- Transporterへ0.1.0 (1)を読み込み、Apple検証を開始。検証・アップロードの成功はまだ報告しない。公開・審査提出は未実施。

初回のApple検証は、LSApplicationCategoryType不足（90242）で失敗した。Mac専用構成へProductivityを追加し、署名ツールでも必須検査する。回帰試験は6件成功。修正したアプリをソース構成から再ビルド中。Proの日本語ローカライズを保存し、日本価格3,980円を登録した。所有者は初回日本のみを指定した。
