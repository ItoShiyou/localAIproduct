# 販売に向けた実装・運用の準備

対象はminutes、第一弾Mac、Pro買い切り3,980円の方針。Windowsは一般実機の確認後に販売。Webは引き続きβ募集であり、購入受付を勝手に開いていない。

## 今回追加した実装

- `tools/minutes_orders.py`: 販売者が支払いを確認した注文を、端末内のSQLite台帳で一度だけ交付する。同じ注文IDの再操作は同じキーを返す。同時実行もトランザクションで直列化する。購入者・版・署名公開鍵が違う再操作は拒否。返金・保留扱いの注文は再交付しない。台帳はGitの外に置き、Unixでは600へ制限。
- 発行者CLI: 購入者名をCLIフラグと誤認しない。既存ライセンス出力の上書きを拒否。秘密鍵のGit内保存・読込、Unixで広すぎる秘密鍵の読取権限を拒否。Windowsの鍵生成も排他的な新規作成を使う。既存の親フォルダーの権限を勝手に変更しない。
- 製品側の署名検証: Ed25519の厳密検証を利用し、弱い公開鍵設定と署名なしの偽キーを拒否。正常な既存署名の形式は維持。
- `tools/minutes_macos_release.py`: 既定は計画表示だけ。所有者が明示実行すると、元アプリを保持したコピーにDeveloper ID署名、公証申請、staple、検証、ZIP・SHA-256作成を行う。未設定、公証拒否、既存出力先、元アプリ内への出力を拒否。自動販売・公開はしない。
- Mac署名時のマイク権限（audio-input）とHardened Runtimeの設定を追加。実際の署名後のマイク操作は、証明書が揃ってからの実機確認が必要。

## 一度だけ所有者が準備するもの

1. Apple DeveloperのDeveloper ID Application証明書をMacのKeychainに用意する。公証用認証情報もnotarytoolのKeychainプロファイルに保管する。パスワードや秘密鍵をチャット／Git／Webへ貼らない。
2. `docs/license.md`の発行ツールで本番鍵をGit外に作成し、暗号化されたバックアップを用意する。本番公開鍵をsrc-tauri/src/license.rsへ設定。鍵管理の主体は所有者。こちらでは本番鍵を生成していない。
3. 販売者名をtauri.conf.jsonの著作権表示へ設定。問い合わせ先、利用・返金条件、価格の税表示、交付・サポート範囲を決め、契約・販売表記を確認する。未確認条件を架空の文章で公開しない。
4. 決済アカウント・商品を準備し、実購入の注文状態・返金状態を所有者が確認できるようにする。OAuth・APIキー等がない状態でサービスへ接続したことにはしない。

## 支払いを確認して交付する

リポジトリのrootから発行ツールをビルド:

```sh
cargo build --release --manifest-path apps/minutes/src-tauri/Cargo.toml --no-default-features --example license_issuer
```

実交付の例（パスと購入者名は所有者が置換。注文IDはサービス名を含める）:

```sh
python3 tools/minutes_orders.py --ledger /専用の保存先/orders.sqlite --order-id polar:注文番号 --issuer /発行ツール/license_issuer --key /Git外の専用保存先/issuer.key --licensee '購入者名' --payment-confirmed
```

このフラグは販売者による支払い確認を表し、決済サービスによる自動検証ではない。キーは表示されるので出力を公開CIログや外部チャットへ転載しない。出力を購入者へ渡す操作は今回自動化していない。キーが漏れない交付経路とサポート体制は、所有者のアカウントで実購入試験をする。

返金・保留を台帳に反映する例:

```sh
python3 tools/minutes_orders.py --ledger /専用の保存先/orders.sqlite --order-id polar:注文番号 --block
```

再交付を止めるだけで、すでにオフライン端末へ入ったキーは即時失効しない。運用上の返金判断とアプリの失効一覧更新は別工程。台帳には購入者情報とキーが含まれるため、Git外・アクセス制限・暗号化バックアップ・保持期限の運用が必要。Windowsはユーザープロファイル内の専用保存先を使いACLを確認する。

## 本番のMac配布を作る

必ず本番公開鍵・販売者情報を設定した後にアプリを再ビルドし、そのアプリで本番キーの登録→Pro解放を確認する。古い配布候補に新しい公開鍵を設定したことにはならない。

```sh
python3 tools/minutes_macos_release.py --app /ビルド済み/minutes.app --output /新しい配布先フォルダー --identity 'Developer ID Application: 所有者名 (TEAMID)' --notary-profile 所有者のプロファイル名
```

上記は無変更・無通信の計画表示。所有者が準備を終えた後、`--execute`を加えた場合だけ署名とAppleへのアプリ送信を行う。Apple認証情報はKeychainから読み込まれ、スクリプトの引数やソースへ入れない。作業用コピーと申請ZIPは新しい出力先へ残す。公開するのは最終ZIPであり、申請用ZIP・台帳・秘密鍵を公開しない。

署名・公証成功だけで、利用規約・決済・実機・購入キー動作を確認したことにはならない。実際にダウンロードした最終ZIPを新しいMacへ展開し、初回起動・マイク許可・実録音・要約・各書き出し・Pro登録を確認してから販売する。

## 試験の扱い

追加した試験は発行者Rust3件、注文10件（うち実CLIの署名・検証・再交付を公開済みの開発キーのみで実施）、Mac配布処理5件。Appleの応答は模擬であり、本番署名・公証を実施済みとはしない。Windows CIにも発行・注文・配布処理の自己テストを組み込んだ。既存アプリのMac／Windows試験はFINAL_HANDOFF_2026-10-07.mdを参照。

ローカル最終確認: Releaseの音声機能なし構成89件（9.14秒）、既定構成89件（8.57秒）、発行者Rust3件、注文10件、Mac配布処理5件、マイク権限設定3件、従来の販売前検査2件が成功。Mac配布候補は署名検証と権限設定を含むコードへ再ビルド済み。実署名・公証の成功とは別。

販売前検査は本番公開鍵と権利者名が未設定のため、意図どおり失敗を返す。公証への送信や、本番の注文・秘密鍵・証明書の作成、顧客への連絡は実行していない。署名、公証、契約／決済、Windows一般実機、実購入によるPro登録は所有者側の準備後に完了できる。現状を販売開始済みと扱わない。

## 参考となる公式仕様

[Appleの公証手順](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow)、[Audio Input権限](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.security.device.audio-input)、[Windows署名ツール](https://learn.microsoft.com/en-us/windows/win32/seccrypto/signtool)。Windowsの実配布署名は所有者の証明書・署名環境が揃ってから実施・確認する。
