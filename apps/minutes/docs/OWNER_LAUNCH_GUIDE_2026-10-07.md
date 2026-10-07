# minutes 販売開始のための所有者向け指示書

更新日：2026-10-07。現在は販売準備中です。下記を完了する前に購入受付を開始しないでください。

## 現在できていること

- 無料版とProの別ビルド。無料版は約203MiB、Proは約3.13GiB（本体、配布ZIPではない）。Proの処理データは同梱済み。
- Proには購入キーの入力・オンライン認証が不要。販売先のデジタル商品ダウンロードを使うため、個別のキー発行・メール送付サーバーは不要。
- ローカルの同梱データを使った文字起こし・Proの要約テストは実施済み。署名・実ダウンロード・実機録音・無料版からの引き継ぎ試験は別です。
- 複数販売先の選択ページと、未完了の配布・販売条件があると購入リンクを有効にしないチェック。

## あなたに必要な操作（この順番）

### 1. 販売者情報・条件を決める

販売者の正式な名義、問い合わせメール、販売上必要な住所・連絡先、利用可能台数、返金条件、更新とサポートの範囲を決めてください。買い切り3,980円は予定価格として記載しています。決済時の税・通貨・最終金額は販売先で設定・確認します。

`checklists/support-policy.md` の条件は例であり、採用済みではありません。存在しない名義・住所・窓口や、無期限サポートの約束を入れないでください。通信販売の表示事項は [消費者庁の案内](https://www.no-trouble.caa.go.jp/what/mailorder/advertising.html)を確認し、該当する場合は専門家に確認してください。販売先を使うだけで表示義務がすべて解消されるとは扱いません。

### 2. Macの署名を準備する

このMacで確認できた有効な署名IDは0件です。Apple Developer Programの加入状況を確認し、Developer ID Application証明書とApple公証用のKeychainプロファイルを準備してください。[AppleのDeveloper ID案内](https://developer.apple.com/developer-id/)を参照してください。これにはAppleの費用が発生し得ます。販売サイトの無料プランとは別です。

パスワード・秘密鍵・APIトークンをチャットやGitへ貼らないでください。証明書と公証資格情報はKeychainで管理します。

### 3. 正式な著作権表示でビルドし直す

以下の「実際の権利者名」は、確認した権利者名に置き換えます。未署名の既存アプリをそのまま販売しないでください。

```sh
cd /Users/keitoozono/localAIproduct
MINUTES_COPYRIGHT='© 2026 実際の権利者名' node apps/minutes/tools/build_edition.mjs free --shared-cache --bundles app
MINUTES_COPYRIGHT='© 2026 実際の権利者名' node apps/minutes/tools/build_edition.mjs pro --shared-cache --bundles app
```

### 4. 配布用コピーを署名・公証する

次はProの例です。署名ID・Keychainプロファイルを実際のものに置き換えます。出力先にはまだ存在しない専用フォルダーを指定してください。

```sh
python3 tools/minutes_macos_release.py \
  --app 'apps/minutes/src-tauri/target/release/bundle/macos/minutes Pro.app' \
  --output '/private/tmp/minutes-pro-release-20261007' \
  --identity 'Developer ID Application: 実際の署名ID' \
  --notary-profile '実際のKeychainプロファイル名'
```

最初は計画表示だけです。確認後、同じコマンドへ `--execute` を加えます。その操作でアプリのコピーを署名し、アプリをAppleへ公証のため送信します。会話のデータは送信しません。Freeにも別の出力先で同じ手順を実施します。

成功すると `minutes-pro-0.1.0-macos-arm64.zip` または `minutes-free-0.1.0-macos-arm64.zip` とSHA-256付きの `release-manifest.json` ができます。自動公開はしません。

### 5. 本当のダウンロードから確認する

機密情報を含まない音声で実施し、OSとメモリ、結果を記録します。

- ブラウザーから取得した署名済みZIPを展開して起動。Gatekeeperの無効化を必要としないこと。
- マイク許可の許可／拒否、短い録音、停止、文字起こし、編集、確定、書き出し。
- インターネットを切った状態で文字起こしとProの要約。外部送信の保証を追加する場合は別途通信監査。
- 無料版で作った記録をバックアップし、無料版を終了してProへ切り替える。記録・音声・メモが読めること。
- Proの話者判別・用語集・各書き出し。長い音声と入力機器の切断時の扱い。
- 認識精度は自分の利用場面の非機密サンプルで確認。合成音声の成功だけで専門分野の精度を保証しないこと。

Windows版は新しい版別配布のビルド・実機試験が未完了です。まずMacのみ販売し、Windowsの購入受付は確認後に別途開始します。

### 6. 2つの販売先へProを登録する

Gumroadの既存商品： https://shiyou5.gumroad.com/l/minutespro

主候補はGumroadとPayhipです。それぞれに署名済みPro ZIPをアップロードし、商品の説明、対応OS、条件、問い合わせ先、価格を設定します。購入後にそのZIPを受け取れるかテストモード等で確認してください。Proを公開GitHub Releasesへ置かないでください。

Payhipは[1ファイル最大5GB、アプリはZIPでの提供](https://help.payhip.com/article/59-adding-a-digital-product)に対応します。BOOTHはファイル容量制限があるため、現状の全同梱Proをそのまま登録できる前提にはせず、選択ページでは準備中のままにします。無料プランでも売上手数料・決済手数料はあります。契約・本人確認・支払先設定は所有者が行います。

商品説明のたたき台：

> minutes Proは、録音をパソコンの中で文字にする買い切りの議事録アプリです。文字起こし、話者の判別、用語集、要約の下書きと複数形式の書き出しに対応。必要なデータは同梱され、購入キーの登録は不要です。音声処理に会話のクラウド送信は使いません。重要な内容は原音で確認してください。Mac（Apple Silicon）向け。ご購入前に無料版と対応環境をご確認ください。

### 7. Webの販売表示を有効にする

1. `web/minutes/support/index.html` に実際の問い合わせ先・確定した条件・必要な販売者表示へのリンクを掲載する。
2. 署名済み無料版の公開ダウンロード先を `web/minutes/install/index.html` に掲載する。
3. `web/minutes/purchase/stores.json` に確認済みの実際の商品URLを入れ、準備できた販売先だけ `enabled: true` にする。
4. `web/minutes/purchase/readiness.json` の各項目は、実際に確認を終えてから `true` にする。ファイルを変えるだけでは試験したことにはなりません。
5. トップ・購入・インストール・サポートページの「準備中」を、公開した版の事実と整合するよう修正する。
6. 下のチェックに合格後、GitHub Pagesへ公開し、公開URLからスマホ・Macで確認する。

```sh
python3 web/minutes/check_site.py
node --test web/minutes/purchase/sales-gate.test.mjs
python3 -m unittest discover -s tools -p test_minutes_macos_release.py
```

## 将来の事業・著作物の引き継ぎ

製品ソース、第三者ライセンス、ビルド／配布手順、公開先、販売条件をセットで管理します。GitHubや販売先の移管は各サービスの規則と所有者の手続きが必要です。コードがあるだけでアカウントや第三者モデルの権利まで自由に売却できるとは扱いません。APIキーや署名秘密鍵を引き継ぎ資料に同梱しないでください。

## いま不要な作業

別売りProへの切り替えでは、Gumroad OAuth設定、購入キーの発行サーバー、メール送信サービス、GitHub定期キー配送は不要です。旧方式の資料とツールは履歴として残っていますが、新しい販売手順では使いません。
