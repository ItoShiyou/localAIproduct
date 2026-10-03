# 名刺OCR＋連絡先管理（「この人誰だっけ」メモ中心）競合調査

調査日: 2026-10-03 / 対象: 日本市場向け・完全ローカル・買い切り・Mac/Windows

## 0. 調査の限界（必ず読むこと）

- 情報源は WebSearch の要約結果のみ。WebFetch は apps.apple.com / saas.imitsu.jp / psrn.jp が egress 制限でブロックされ、**一次ソース（公式ページ）の本文は直接確認できていない**。
- 価格は検索結果に表示された値で、調査日時点の最新とは限らない。数字は着手前に公式で再確認すること。
- 「未確認」は確認できなかった点。推測は「推測」と明記する。

## 1. 競合一覧

| 名称 | 価格 | ローカル/クラウド | 主な機能 | 弱点 | 出典 |
|---|---|---|---|---|---|
| Eight（Sansan社） | 無料（ほぼ全機能）。Eightプレミアム 600円/月 or 6,000円/年（両面全項目データ化、一括ダウンロード等） | クラウド。人力入力でデータ化、名刺交換（リンク）機能で相手の情報が更新される | スマホ撮影→人力データ化、人脈ネットワーク、転職等の自動更新 | データはクラウド保管。データ化に翌営業日〜8〜15日（大量時）の待ち。デスクトップ向け買い切り・ローカル運用は不可（Macアプリ有無は未確認） | https://apps.apple.com/jp/app/id444423637 / https://itmedia.co.jp/bizid/spv/1304/26/news025_2.html / https://cloud.watch.impress.co.jp/docs/news/1366705.html |
| Sansan | 基本料金 19,800円（表示値。単位は未確認）。詳細は要問い合わせ（初期費用・スキャナ・オプション別） | クラウド（法人向け） | 組織共有の名刺DB、人物・企業情報、CRM/SFA連携 | 法人向けで個人・小規模には高額/不透明。ローカル不可 | https://saas.imitsu.jp/cate-business-card/service/369/price |
| Wantedly People | 無料プラン（同時登録10枚など制限の記載あり）。People Premium 600円/月（CSV出力、広告非表示） | クラウド（スマホアプリ中心） | 複数枚同時読取、SNS的な繋がり、Wantedly連携 | モバイル中心、クラウド前提、無料枠の制限。Mac/Windows版は未確認 | https://saas.imitsu.jp/cate-business-card/service/378/price |
| CamCard / CAMCARD BUSINESS | 個人向け無料プランあり（有料詳細は未確認）。法人版 STANDARD 1,700円/ID/月、PROFESSIONAL 2,500円/ID/月（最低5ID・12か月） | クラウド | OCR＋高精度校正（月20/50枚）、チーム共有 | 法人サブスク。個人向け有料プランの内容・価格は未確認 | https://saas.imitsu.jp/cate-business-card/service/374/price |
| ScanSnap Home（PFU/リコー）＋ScanSnap | ソフトはScanSnap専用（スキャナ購入が前提。ソフト単体価格は未確認） | PC保存が基本（ScanSnap Cloud経由でEight等のクラウド連携も可） | 名刺をスキャン、OCR、タグ・フォルダ整理 | ScanSnap所有が前提。連絡先DB・人物メモ中心の設計ではない（推測を含むため、実機確認が必要） | https://www.pfu.ricoh.com/imaging/downloads/manual/basic/win/jp/topics/scan_sshome_cards.html |
| やさしく名刺ファイリング PRO（NJK/メディアドライブ） | 過去版 税別7,800円（v.15、スキャナ付 16,800円）。現行は 8,580円/60,500円（税込）の掲載あり（製品・版は未確認） | ローカル（Windows、DB管理） | スキャナ取込→OCR→DB、日英中韓対応 | Windows中心（Mac対応は未確認）。販売終了予定 2027/3/31、サポート終了 2030/3/31（検索結果の記載）。UIが古い印象（推測） | https://www.biccamera.com/bc/item/3080296/ / https://www.bcnretail.com/news/detail/20171122_43959.html / https://saas.imitsu.jp/cate-business-card/service/3831 |
| Mac向けApp Storeの名刺アプリ（「名刺管理 - スキャン 名刺 と 連絡先」「Card Scan」「Log book Digi」） | 価格・課金形態は未確認 | 保存先（ローカル/iCloud）は未確認 | OCR、タグ、連絡先アプリ連動（Log book Digi） | 内容はストア要約のみ。詳細未確認 | https://apps.apple.com/jp/app/id1498739833 |
| 無料OCR（各社の無料枠、OS標準のテキスト認識等） | 無料 | 製品による | 文字抽出 | 名刺の項目分解・DB化・検索は別途必要。本調査では具体製品を検証していない（未確認） | 未確認 |

## 2. 価格帯の相場（確認できた範囲）

- 個人向けクラウド: 無料〜 月600円/年6,000円（Eight、Wantedly People プレミアム）。
- 法人クラウド: 1,700〜2,500円/ID/月（CAMCARD BUSINESS）、Sansan は基本料金19,800円表示＋要問い合わせ。
- ローカル買い切り: 数千〜1万円台（やさしく名刺ファイリング PRO 旧版7,800円〜、現行表示8,580円）、スキャナ付や上位版は6万円台まで。
- 示唆: 個人向けはクラウド無料が強力。買い切りで取れる価格は、Eightプレミアム約1〜1.5年分（6,000〜9,000円）が心理的な上限の目安（推測）。

## 3. 個人情報保護法・取り扱い上の注意

出典は検索結果の要約が中心。法的判断は専門家・個人情報保護委員会の一次資料で確認すること。

- 名刺の氏名・会社名・連絡先等は個人情報に該当し得る。名刺をデータベース化（検索可能に整理）すると法の規律対象になる、という整理が複数の解説に見られる（https://www.kotora.jp/c/?p=112255）。
- 2022年の改正で、5,000件以下の事業者も一律に適用対象になった旨の解説がある（https://www.bengo4.com/other/1146/n_4619/ ほか）。
- 取得時は利用目的を通知または公表する義務（名刺交換時の目的は、名刺を受け取る場面で自明な範囲かどうかの整理が必要。詳細は未確認）。不正の手段による取得は禁止。
- 名刺交換で得た連絡先への広告メール送信可否が、個人情報保護委員会のQ&Aに追加されたとの記載あり（https://www.psrn.jp/topics/detail.php?id=12570 ※本文は未取得）。
- 第三者提供・委託・安全管理措置: 完全ローカルなら、アプリ提供者はデータを取得しない設計にでき、**自社（開発者）側の個人データ取扱いが原則発生しない**（設計上の主張であり、法的見解ではない）。ただし利用者（事業者）側には安全管理措置（端末暗号化、バックアップ、紛失対策）の責任が残る。
- 製品要件の示唆（推測）: DBの暗号化、書き出しエクスポート、一括削除、メモ欄に「機微な情報（病歴・信条等の要配慮個人情報）を書かない」旨の注意表示、クラウドAI/外部送信を行わないことの明記。
- 弁護士確認: 未確認。リリース前にプライバシーポリシー・利用規約を法務確認すること。

## 4. 差別化の余地

確認できた事実に基づく仮説:

1. 完全ローカル・オフライン: Eight/Wantedly/Sansan/CamCard はクラウド前提（上表）。名刺データを外に出したくない層（士業、医療、取材者、小規模事業者）に訴求可能。需要規模は未確認。
2. 買い切り: 個人向けクラウドはサブスク/無料で、ローカル買い切り主力のやさしく名刺ファイリングは Windows中心かつ販売終了予定。Mac/Windows両対応の現行品は、本調査では確認できなかった（網羅検索ではない）。
3. 「この人誰だっけ」メモ: 出会った場所・話した内容・紹介者・次のアクション・写真等の文脈メモと全文検索。既存品は名刺情報の管理が主で、文脈メモを中心にした製品は本調査では確認できなかった（未確認）。
4. 人力データ化の遅延（Eight）に対し、端末内OCRの即時性。精度は人力に劣る可能性（要検証）。
5. 弱点・リスク: Eight が無料で人脈更新まで提供する点は強い。無料クラウドに対し、買い切りの優位はプライバシー・オフライン・ロックイン回避に限られる。ScanSnap利用者はScanSnap Homeで足りる可能性。

## 5. 推奨: 「絞る」

- 作らない: Sansan/Eight型の組織共有・人脈ネットワーク・自動更新（クラウド必須で勝負にならない）。
- 作る/絞る: 「ローカル完結の個人向け『この人誰だっけ』メモ＋名刺OCR」に絞る。核は(a)端末内OCR（手入力補正前提）、(b)出会いの文脈メモ・タグ・全文検索、(c)CSV/vCardエクスポート、(d)暗号化DB、(e)Mac/Windows両対応、(f)買い切り（目安 3,000〜6,000円台。根拠は上記相場からの推測）。
- 着手前の検証: 日本語名刺（縦書き・多言語）OCR精度、Eight利用者の乗り換え動機（ヒアリング）、Mac/Windows既存ローカル品の再調査（ブロックされた公式ページ含む）、法務確認。
- 判断基準: 上記検証で「プライバシー目的で買い切りに払う層」が確認できなければ、作らない。

## 出典一覧

- https://cloud.watch.impress.co.jp/docs/news/1366705.html
- https://apps.apple.com/JP/app/id444423637
- https://www.itmedia.co.jp/bizid/spv/1304/26/news025_2.html
- https://saas.imitsu.jp/cate-business-card/service/369/price
- https://saas.imitsu.jp/cate-business-card/service/378/price
- https://saas.imitsu.jp/cate-business-card/service/374/price
- https://saas.imitsu.jp/cate-business-card/service/3831
- https://www.pfu.ricoh.com/imaging/downloads/manual/basic/win/jp/topics/scan_sshome_cards.html
- https://www.biccamera.com/bc/item/3080296/
- https://www.bcnretail.com/news/detail/20171122_43959.html
- https://apps.apple.com/jp/app/id1498739833
- https://www.kotora.jp/c/?p=112255
- https://www.bengo4.com/other/1146/n_4619/
- https://www.psrn.jp/topics/detail.php?id=12570
