# 販売プラットフォームの選定(2026-10-03 調査)

方針: **決済・税・ライセンス発行は外部サービスに任せ、自分では実装しない。** アプリ側に残るのは、ライセンスを検証する薄いクライアントだけ。

## 結論

**Polar を第一候補、Lemon Squeezy を予備にする。** この2つは、条件を公式文書で確認できた範囲で、ほぼ同等だった。

| 観点 | Polar | Lemon Squeezy |
|---|---|---|
| 税の代行(販売者として購入者に売る形) | ○ | ○ |
| 日本からの受取 | ○ 日本が対応国に明記(Stripe Connect Express) | ○ 日本が銀行振込の対応国に明記。PayPalも可 |
| ライセンスキー | ○ 自動発行、台数制限、有効期限、検証API | ○ 自動発行、台数制限、有効期限。検証APIの詳細は未確認 |
| 手数料(基本) | 5% + $0.50(Starter。国際カードは+1.5%) | 5% + $0.50(国際取引は+1.5%、PayPalは+1.5%) |
| 出金時の費用 | 月$2(出金がある月)+ 出金ごとに0.25%+$0.25、国際送金は追加 | 米国外の口座は1%。PayPal出金は3%(上限$30) |
| 返金 | 初期手数料は戻らない。チャージバックは1件$15 | 未確認 |
| 決済手段 | カードのみ(第三者の比較記事による) | カード、PayPal |
| 不安材料 | 手数料が改定されたとする記事がある(料金は変わりうる) | Stripeの傘下で、Stripe Managed Payments への移行が2026年1月に告知されている |

Polar を先にする理由は、ライセンス検証APIの仕様を公式文書で確認できたことと、移行告知の影響を受けないこと。決定的な差ではないので、**共通コアのライセンス部分はアダプターにして、切り替えを1日で済む作りにする**(`core/README.md`)。

## 他の候補を外した理由

| サービス | 状況 |
|---|---|
| Gumroad | 直販は10% + $0.50で最も高い。税の代行(2025年1月〜)とライセンスキーはあるが、手数料負担が重い |
| BOOTH | 5.6% + 45円(2025年10月28日に22円から改定)で最安。ただしライセンス認証の仕組みを確認できず、ソフトの販売規約も未確認。認証なしのダウンロード販売なら選択肢 |
| Paddle | ソフトの買い切りとライセンスキーの送付に対応と書かれているが、仕様の詳細、日本の販売者の対応、審査の要件を確認できなかった |

## 手取りの目安

前提: 為替は1ドル=150円(仮置き)。税抜相当で、出金時の費用と為替手数料は除く。購入者は米国外のカード。

| 価格 | Polar / Lemon Squeezy | Gumroad | BOOTH(認証なし) |
|---|---|---|---|
| ¥1,480 | 約¥1,309(実効11.6%) | 約¥1,257 | 約¥1,352 |
| ¥3,980 | 約¥3,646(実効8.4%) | 約¥3,507 | 約¥3,712 |

固定費の$0.50(約¥75)が効くので、**低価格の単体は手数料の比率が重くなる。** 以前に仮置きした15%よりは軽いが、¥1,480の単体は薄い。全部入りパスと、¥1,980以上の価格を検討する根拠になる。

Polar は月$20のプランで 3.8% + $0.40 になる。1件あたり約¥63安くなるので、月に約48件売れる規模になるまでは、基本プランのままでよい。

## 確認できていないこと(契約前に確かめる)

| 項目 | 確かめ方 |
|---|---|
| **返金したときに、ライセンスが自動で失効するか** | Polar の公式文書で確認できたのは、サブスクの取消時の自動失効だけ。テスト商品で購入→返金→検証を通す |
| **消費税とインボイス**: 事業者の購入者が、仕入税額控除に使える領収書になるか | 各社のサポートへ問い合わせ。購入者の多くが個人事業主なので重要 |
| 特定商取引法の表記が、自分に必要か | 各社の規約と、消費者庁の案内で確認 |
| Lemon Squeezy の License API と返金時の挙動 | 予備に切り替える前に確認 |
| 日本の購入者向けの価格表示(税込か税別か)と決済手段 | テスト商品のチェックアウト画面で確認 |

## 最初の週にやる検証

1. Polar でアカウントを作り、**テスト商品1つ**を作る(ライセンスキー有効、台数制限2、有効期限なし)。
2. チェックアウト → キー受領 → `core` のクライアントでアクティベート → 検証 → 返金 → 検証失敗、を一通り通す。
3. 日本の購入者向けの表示と、領収書の様式を確認し、上の未確認事項を埋める。

アカウント作成と本人確認、銀行口座の登録は、あなた自身が行う必要があります。

## 出典

- [Lemon Squeezy: Fees](https://docs.lemonsqueezy.com/help/getting-started/fees)
- [Lemon Squeezy: Supported Countries](https://docs.lemonsqueezy.com/help/getting-started/supported-countries)
- [Lemon Squeezy: ライセンスキーの発行](https://docs.lemonsqueezy.com/help/licensing/generating-license-keys)
- [Lemon Squeezy: 2026 Update(Stripe Managed Payments)](https://www.lemonsqueezy.com/blog/2026-update)
- [Polar: Fees](https://polar.sh/docs/merchant-of-record/fees)
- [Polar: Supported Countries](https://polar.sh/docs/merchant-of-record/supported-countries)
- [Polar: License Keys](https://polar.sh/docs/features/benefits/license-keys)
- [Gumroad: Pricing](https://gumroad.com/pricing)
- [Gumroad: License Keys(ヘルプ)](https://gumroad.com/help/article/76-license-keys)
- [Paddle: Digital products](https://developer.paddle.com/get-started/how-paddle-works/digital-products)
- [BOOTH 手数料改定(GameBusiness.jp)](https://www.gamebusiness.jp/article/2025/07/24/24711.html)
- [デジタル商品プラットフォーム比較(Shareuhack)](https://www.shareuhack.com/en/posts/digital-product-platform-comparison-asia-2026)
