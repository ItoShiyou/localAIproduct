# 競合調査: 完全ローカル・買い切りの録音ノイズ除去＋日本語文字起こし(議事録)アプリ (Mac/Windows)

調査日: 2026-10-03

## 0. 調査方法と信頼性の注意 (必読)

- WebFetch は実行環境のegress proxyで notta.ai / rimo.app / gumroad / huggingface.co への直接アクセスがブロックされ、公式ページの一次確認はできなかった。
- 以下の数値は WebSearch の要約結果と、検索結果に出た二次情報サイト(比較サイト・レビュー記事)に基づく。価格は変動するため、**公式ページでの再確認が必要**。
- 「未確認」は、今回の調査で根拠が得られなかった点を指す。日本語精度の定量比較(同一テストセットでの各社CER)は得られていない。

## 1. 競合一覧表

| 名称 | 価格 | ローカル/クラウド | 日本語精度の評価 | ノイズ除去 | 弱点 / 備考 | 出典 |
|---|---|---|---|---|---|---|
| Notta | Free 0円(月120分・1回3分まで) / Premium 1,980円/月(年払い実質1,185円/月、月1,800分) / Business 4,180円/月(年払い実質2,508円/月、無制限) | クラウド(サブスク) | 公称98.86%(自社主張、第三者検証は未確認) | 未確認 | サブスク。音声がクラウド送信される。無料枠は小さい | https://notta.ai/pricing , https://www.sungrove.co.jp/notta/ , https://aipicks.jp/tool/notta/pricing |
| Rimo Voice | 文字起こしプラン 1,650円/月(月2,100分) / プロ 4,950円/月(無制限+AI要約) / チーム 6,600円/月。無料プランなし(7日トライアル) | クラウド(サブスク) | 日本語特化。定量評価は未確認 | 未確認 | 無料プランなし、サブスク | https://boxil.jp/service/8125/ , https://subanana.com/ja/blog/rimo-voice-料金 |
| CLOVA Note (LINE) | 無料(月300分) | クラウド | 日本語高精度とされる(定量は未確認) | 未確認 | **2025-07-31にサービス終了**。後継は LINE WORKS AiNote | https://www.notta.ai/blog/clova-note , https://digital-gorilla.co.jp/ai-lab/?p=2263 |
| LINE WORKS AiNote (CLOVA Note後継) | フリー 月300分 / ソロ 1,440円/月(年契約、月600分) / チーム 19,800円/月(6,000分)ほか | クラウド | 日本語に強いとされる(定量は未確認) | 未確認 | 時間上限あり、クラウド | https://line-works.com/ainote/column/ainote-price/ |
| Otter.ai | Pro $16.99/月(年払い$8.33/月) / Business $30/ユーザー/月(年払い$19.99) | クラウド | 日本語対応するが「日本語サポートが弱い」との評も(個別レビュー1件、定量は未確認) | 未確認 | 英語中心、日本語は後発、サブスク | https://sonix.ai/resources/otter-ai-pricing/ , https://subanana.com/ja/blog/otter-ai-文字起こし-日本語対応 |
| MacWhisper (Gumroad版) | Pro 買い切り 約59〜64ユーロ(約$69)、無料版あり | ローカル(Whisper/whisper.cpp、Apple Silicon) | Whisperモデル依存(日本語の定量評価は未確認) | 未確認(確認できず) | **Macのみ**。話者分離はベータ。日本語特化のUI/後処理は未確認 | https://www.getvoibe.com/resources/macwhisper-pricing/ , https://daveswift.com/macwhisper/ |
| Whisper Transcription (Mac App Store版) | $6.99/月、$29.99/年、$99.99 買い切り(アプリ内課金) | ローカル | 同上 | 未確認 | Macのみ。MacWhisperと別SKU | https://macwhisper.helpscoutdocs.com/article/40-macwhisper-whisper-transcription-difference , https://www.getvoibe.com/resources/macwhisper-pricing/ |
| Buzz | OSS(MIT、ただし検索結果で明示確認できたのは Whisper 本体とWhisperingのMIT。Buzzのライセンスは未確認) | ローカル | Whisper依存 | 未確認 | 一般向けUI/議事録機能の水準は未確認 | https://alternativeto.net/software/buzz-captions |
| Whisper Notes | $6.99 買い切り(1万語無料トライアル) | ローカル(Parakeet V3/Whisper/SenseVoice) | 未確認 | 未確認 | **Mac/iPhoneのみ、Windows版なし** | https://whispernotes.app/ja , https://whispernotes.app/ja/windows |
| GoWhisper | 買い切り $23〜$50 | ローカル(Win/Mac/Linux) | 未確認 | 未確認 | 情報が少なく詳細未確認 | https://alternativeto.net/software/gowhisper/about |

### 買い切り・ローカルの観点での整理

- ローカル+買い切りは MacWhisper(約$69)、Whisper Notes($6.99)、GoWhisper($23〜50)が確認できた。いずれも**日本語議事録に特化した訴求は今回確認できず(未確認)**。
- 日本語特化の主要サービス(Rimo, Notta, AiNote)はすべてクラウド・サブスク。
- 「ノイズ除去を統合した」ローカル買い切り日本語アプリは、今回の検索では見つからなかった(「存在しない」の断定はしない。未確認)。

## 2. モデル候補とライセンス

| モデル | ライセンス | 規模 | 日本語に関する情報 | 出典 |
|---|---|---|---|---|
| Whisper large-v3-turbo | MIT(コード・重み) | 809M(デコーダ32→4層) | OpenAI主張で large-v3 比約8倍速。日本語の定量精度は今回未確認 | https://www.simonwillison.net/2024/Oct/1/whisper-large-v3-turbo-model , https://superlinked.com/models/openai-whisper-large-v3-turbo.md |
| kotoba-whisper v2.0 | Apache 2.0 | 756M | large-v3の蒸留、6.3倍速。ReazonSpeechの720万クリップで学習。ドメイン内でlarge-v3よりCER/WER良好、CommonVoice 8.0でCER 9.2%と報告 | https://www.promptlayer.com/models/kotoba-whisper-v20 , https://catalog.ngc.nvidia.com/orgs/nvidia/teams/riva/models/kotoba_whisper |
| kotoba-whisper v2.1 / v2.2 | 未確認(v2.0と同じとは断定しない) | 未確認 | 存在はHugging Face上で確認 | https://huggingface.co/kotoba-tech/kotoba-whisper-v2.1/discussions |
| ReazonSpeech nemo-v2 | Apache 2.0(モデル) | 619M | 日本語特化、数時間の長尺音声に対応 | https://huggingface.co/reazon-research/reazonspeech-nemo-v2 |
| (参考)ReazonSpeechコーパス | CDLA-Sharing-1.0、ただしHF上は著作権法30条の4の範囲での利用に同意するゲート付き | - | **商用での扱いは要法務確認**。モデル利用とデータ再配布は別問題 | https://voxkitchen.readthedocs.io/en/latest/datasets/reazonspeech/ , https://forest.watch.impress.co.jp/docs/news/1471724.html |

注意:
- kotoba-whisper / ReazonSpeech は学習データ(ReazonSpeech)由来の権利関係について、モデルがApache 2.0表記でも製品組み込み前に法務確認を推奨(今回は規約全文を読めていない=未確認)。
- 推論実行環境(whisper.cpp, faster-whisper, NeMo等)のWindows/Mac対応・速度は未確認。ReazonSpeech nemo(NeMo/PyTorch)のオンデバイス配布のしやすさは未確認。

### ノイズ除去ライブラリ候補

| 名称 | ライセンス | 出典 |
|---|---|---|
| DeepFilterNet | MIT または Apache 2.0(選択) | https://libraries.io/pypi/deepfilternet , https://www.linuxlinks.com/deepfilternet-low-complexity-speech-enhancement-framework/ |
| RNNoise | BSD-3-Clause | https://build-test-1.opensuse.org/projects/openSUSE/packages/rnnoise/files/rnnoise.spec?expand=1 |

- 学習済み重みの利用条件・日本語音声でのノイズ除去が文字起こし精度を上げるか(逆に下げる場合もある)は**未確認。要自前検証**。

## 3. 価格帯の相場

- クラウドSaaS(月額): 約1,200〜1,980円(Notta Premium、Rimo文字起こしプラン1,650円、AiNoteソロ1,440円)、無制限/高機能で約2,500〜4,950円。年額換算で約1.4万〜6万円。
- 海外Otter: Pro $8.33〜16.99/月。
- ローカル買い切り: $6.99(Whisper Notes)〜$23〜50(GoWhisper)〜約$69(MacWhisper Pro)。App Store版の永久ライセンスは$99.99。
- 示唆: ローカル買い切りの相場は「$7〜$100」。日本語特化・議事録機能・ノイズ除去を載せて 数千円〜1.5万円程度が競合の価格帯に収まる(これは相場からの推論であり、支払意思額の調査はしていない=未確認)。

## 4. 差別化の余地 (確認事項からの推論。市場検証は未実施)

1. 完全ローカル: 日本語特化の既存サービスはすべてクラウド。機密性の高い会議(法務・医療・自治体等)で需要がある可能性(需要規模は未確認)。
2. 買い切り: 日本語特化かつ買い切りの競合は今回確認できなかった。
3. ノイズ除去+文字起こしの統合ワークフロー: 競合のノイズ除去対応は未確認。
4. Windows対応: MacWhisper / Whisper Notes はMacのみ(Windows版なし)。
5. 日本語特化の後処理(フィラー除去、句読点、固有名詞辞書、話者分離)。品質は自前評価が必要。
6. 弱点リスク: MacWhisper等の既存ローカルアプリが日本語UI/モデル(kotoba-whisper等)を追加すれば優位性は薄れる。話者分離・要約(LLM)をローカルで実用品質にするのは難度が高い(未検証)。

## 5. 推奨: 「絞る」

結論: **作る(全方位)ではなく、「絞って」作る。**

- 絞り方の案: 「機密会議向け・完全オフライン・日本語・買い切り」+「ノイズ除去前処理」に特化。Windows/Macの両対応を強みにする。
- 作る前に検証すべきこと(未確認事項):
  1. kotoba-whisper / ReazonSpeech / Whisper turbo を実際の会議音声(ノイズ有り)で比較(CER測定)。
  2. ノイズ除去の有無による精度差。
  3. ReazonSpeech由来モデルの商用配布条件の法務確認。
  4. 想定ユーザー(法人/士業/自治体等)へのヒアリングと価格受容性。
  5. 一般PC(GPUなし)での処理速度。
- 「作らない」条件: 上記1〜2で既存の無料Whisper系(Buzz等)+ツール群に対して体感差が出ない場合、または3で商用利用が不可の場合。
- 要約・AI議事録の自動生成をローカルLLMで同梱するかは、品質とPC要件の面で別途判断(今回調査対象外=未確認)。

## 出典URL一覧

- https://notta.ai/pricing
- https://www.sungrove.co.jp/notta/
- https://aipicks.jp/tool/notta/pricing
- https://boxil.jp/service/8125/
- https://subanana.com/ja/blog/rimo-voice-料金
- https://www.notta.ai/blog/clova-note
- https://digital-gorilla.co.jp/ai-lab/?p=2263
- https://line-works.com/ainote/column/ainote-price/
- https://sonix.ai/resources/otter-ai-pricing/
- https://subanana.com/ja/blog/otter-ai-文字起こし-日本語対応
- https://www.getvoibe.com/resources/macwhisper-pricing/
- https://daveswift.com/macwhisper/
- https://macwhisper.helpscoutdocs.com/article/40-macwhisper-whisper-transcription-difference
- https://alternativeto.net/software/buzz-captions
- https://whispernotes.app/ja
- https://whispernotes.app/ja/windows
- https://alternativeto.net/software/gowhisper/about
- https://www.simonwillison.net/2024/Oct/1/whisper-large-v3-turbo-model
- https://superlinked.com/models/openai-whisper-large-v3-turbo.md
- https://www.promptlayer.com/models/kotoba-whisper-v20
- https://catalog.ngc.nvidia.com/orgs/nvidia/teams/riva/models/kotoba_whisper
- https://huggingface.co/kotoba-tech/kotoba-whisper-v2.1/discussions
- https://huggingface.co/reazon-research/reazonspeech-nemo-v2
- https://voxkitchen.readthedocs.io/en/latest/datasets/reazonspeech/
- https://forest.watch.impress.co.jp/docs/news/1471724.html
- https://libraries.io/pypi/deepfilternet
- https://www.linuxlinks.com/deepfilternet-low-complexity-speech-enhancement-framework/
- https://build-test-1.opensuse.org/projects/openSUSE/packages/rnnoise/files/rnnoise.spec?expand=1
