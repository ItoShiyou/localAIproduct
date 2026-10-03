# 依存とモデルのライセンス(スパイク時点)

調べた日: 2026-10-03。「確認元」は、このスパイクで実際に読めたもの。**Hugging Face(モデルカード)は実行環境から到達できず、モデルの重みのライセンスは一次情報を読めていない**(読めなかったものは「要確認」)。配布時の表記は、各 LICENSE 全文と著作権表示を同梱する前提で書いている。法的な最終判断ではない。

## 採用候補(ノイズ除去)

| 名前 | 版 | ライセンス | 確認元 | 配布時に必要なこと | 状態 |
|---|---|---|---|---|---|
| RNNoise(xiph/rnnoise。重みはソースに内蔵) | 0.4.5 の pyrnnoise 同梱ライブラリで実測(本体の版は未確認) | BSD-3-Clause(著作権表示: Jean-Marc Valin, Amazon, Mozilla, Xiph.Org, Mark Borgerding) | `raw.githubusercontent.com/xiph/rnnoise/main/COPYING` を読んだ | 著作権表示と許諾文の同梱 | 確認済み |
| pyrnnoise(RNNoise の Python 束ね。本番では使わず、C ライブラリを Rust から使う想定) | 0.4.5 | Apache-2.0(wheel 内 LICENSE を確認) | wheel の `licenses/LICENSE` | 本番で使うなら表記が必要 | 確認済み(ただし同梱ライブラリ側の表記は上の BSD-3) |
| DeepFilterNet(DFN3 のコードと学習済み重み。リポジトリの `models/` に重みあり) | 0.5.6(PyPI)、重みは `models/DeepFilterNet3*.zip` | MIT または Apache-2.0(選択) | `LICENSE-MIT` と `LICENSE-APACHE` をリポジトリで読んだ。PyPI メタデータも MIT | 著作権表示と許諾文の同梱 | コードは確認済み。**重みが学習データ(音声コーパス)由来の制約を受けるかは未確認 → 要確認** |

## 採用候補(文字起こし。今回は重みを取得できず、すべて未実測)

| 名前 | ライセンス | 確認元 | 状態 |
|---|---|---|---|
| Whisper(large-v3-turbo などの重み) | MIT(コードと重み) | `openai/whisper` の README に「code and model weights are released under the MIT License」、LICENSE 全文を読んだ | 確認済み(large-v3-turbo の個別モデルカードは未読) |
| faster-whisper(CTranslate2 実行基盤) | MIT / CTranslate2 も MIT | 両リポジトリの LICENSE を読んだ。PyPI メタデータも MIT | 確認済み |
| whisper.cpp | MIT | LICENSE を読んだ | 確認済み |
| kotoba-whisper v2.0(日本語向け蒸留) | Apache-2.0 とされる(二次情報: 競合調査 `docs/research/minutes.md`) | HF のモデルカードは到達不可。GitHub の `kotoba-tech/kotoba-whisper` の README にはライセンス記載なし | **要確認**。学習に ReazonSpeech を使っているため、データ由来の条件も要確認。v2.1 / v2.2 は別途 |
| ReazonSpeech のコード(`reazon-research/ReazonSpeech`) | Apache-2.0 | リポジトリの LICENSE と README.rst の LICENSE 節を読んだ | コードは確認済み |
| ReazonSpeech の学習済み重み(nemo-v2、k2-v2、sherpa-onnx 版 zipformer-ja など) | モデルは Apache-2.0 とされる(二次情報) | HF のモデルカードは到達不可 | **要確認(商用可否は未確定)**。下の注記を参照 |
| sherpa-onnx(実行基盤) | Apache-2.0 | LICENSE 全文を読んだ | 確認済み |
| ONNX Runtime | MIT | LICENSE を読んだ | 確認済み |
| Silero VAD(音声区間の検出の候補) | MIT | LICENSE を読んだ | 確認済み(重みの扱いは同リポジトリのため MIT とみなせるが、モデルカード相当は未読) |

### ReazonSpeech の商用可否について(根拠と、確認できなかったこと)

- コードのリポジトリは Apache-2.0(読めた)。モデル重みも Apache-2.0 と案内されているという二次情報はあるが、**モデルカード原文は読めていない**。
- 学習データの ReazonSpeech コーパスは CDLA-Sharing-1.0 で、Hugging Face 上では「著作権法30条の4の範囲での利用」に同意するゲートが付いている、という報道・解説がある(二次情報。`docs/research/minutes.md` の出典)。**重みの配布・商用利用が、このデータ由来の条件を受けるか**は、利用条件の原文を読めておらず判断できない。
- したがって、**ReazonSpeech 系と、それで学習した kotoba-whisper は、販売前に一次情報の確認と法務の確認が済むまで「製品に同梱する」候補にしない**。Whisper(MIT)は、この問題を避けられる。

## テスト・開発のみで使うもの(配布物に含めない)

| 名前 | ライセンス | 用途 |
|---|---|---|
| pyopenjtalk-prebuilt 0.3.0(OpenJTalk と同梱の HTS 音声) | MIT(PyPI メタデータ)。同梱の辞書・音声の個別ライセンスは未確認 | テスト音声の合成。生成した音声ファイルを `testset/` に置くため、辞書・音声のライセンス(音声の再配布条件)は**要確認** |
| PyTorch 2.14.1 / torchaudio 2.11.0 | BSD 系(未確認の細目あり) | DeepFilterNet の実測(本番では ONNX 版か Rust 実装を使う案) |
| jiwer 4.0.0 | Apache-2.0(メタデータ) | 誤り率の計算 |
| numpy / scipy / soundfile | BSD 系(未確認) | スクリプト |

## 同梱する可能性があるもの

| 名前 | 状態 |
|---|---|
| ffmpeg | このスパイク環境の ffmpeg 6.1.1 は `--enable-gpl` でビルドされている(GPL)。**製品に同梱する場合は LGPL 構成でのビルドか、Rust のデコーダ(symphonia など。ライセンス未確認)に替えるかを決める必要がある → 要確認**。実行時に別プロセスで呼ぶだけでも GPL の配布条件が問題になりうる。 |
