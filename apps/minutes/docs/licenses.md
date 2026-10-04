# 依存とモデルのライセンス(スパイク時点)

調べた日: 2026-10-03(2026-10-04 に Mac(M2)から Hugging Face のモデルカード原文を読み、下の「2026-10-04 追記」を加えた)。「確認元」は、このスパイクで実際に読めたもの。**Hugging Face(モデルカード)は実行環境から到達できず、モデルの重みのライセンスは一次情報を読めていない**(読めなかったものは「要確認」)。配布時の表記は、各 LICENSE 全文と著作権表示を同梱する前提で書いている。法的な最終判断ではない。

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

## 2026-10-04 追記(Mac M2 から一次情報を確認。製品の実装で採用したもの)

### モデルカード原文(Hugging Face の README.md の YAML `license:` と本文)

| モデル | モデルカードの記載(原文) | 学習データ | 判断 |
|---|---|---|---|
| `openai/whisper-large-v3-turbo` | `license: mit` | (OpenAI の学習データ。データ由来の追加条件の記載なし) | **採用**。製品の既定 |
| `ggerganov/whisper.cpp`(ggml 形式への変換済みモデルの配布元。`ggml-large-v3-turbo-q5_0.bin` を取得、SHA-256 `394221709cd5ad1f…`、574MB) | `license: mit` | 同上 | **採用**(変換・量子化済みの配布物) |
| `kotoba-tech/kotoba-whisper-v2.0` | `license: apache-2.0`。本文: "trained on the `all` subset of ReazonSpeech" | ReazonSpeech | 重みは Apache-2.0。**データ由来の条件は下記のとおりで、要確認のまま(同梱しない)** |
| `reazon-research/reazonspeech-k2-v2` | `license: apache-2.0`、本文の License 節も "Apaceh Licence 2.0"(原文の綴りのまま) | ReazonSpeech v2.0 | 同上 |
| データセット `reazon-research/reazonspeech` | メタデータ `license: other`(CDLA-Sharing-1.0)、ゲート付き。利用条件の原文: "TO USE THIS DATASET, YOU MUST AGREE THAT YOU WILL USE THE DATASET SOLELY FOR THE PURPOSE OF JAPANESE COPYRIGHT ACT ARTICLE 30-4." | — | モデルの重みに、この「30条の4の目的に限る」条件が及ぶかは、原文からは判断できない。**法務の確認が済むまで、ReazonSpeech 系と kotoba-whisper は製品に同梱しない**(方針は変えない) |

### 製品(`src-tauri/`)の依存(`cargo metadata` で確認)

| 名前 | 版 | ライセンス | 配布時に必要なこと / 注意 |
|---|---|---|---|
| whisper.cpp(whisper-rs-sys が同梱してビルド) | whisper-rs-sys 0.14.1 同梱版 | MIT | 著作権表示と許諾文 |
| whisper-rs / whisper-rs-sys(Rust 束ね) | 0.15.1 / 0.14.1 | Unlicense | 表記不要(任意)。**注意: 0.15.1 の `set_abort_callback_safe` は不具合があり使っていない**(`asr.rs` のコメント) |
| nnnoiseless(RNNoise の Rust 移植。重み内蔵) | 0.5.2 | BSD-3-Clause | 著作権表示と許諾文 |
| symphonia(音声の読み込み: wav / mp3 / aac / mp4(m4a) / flac / ogg) | 0.5.5 | **MPL-2.0** | ファイル単位のコピーレフト。改変せずに使う限り、アプリ全体の公開は不要だが、**MPL のソースの入手方法の告知と許諾文の同梱が要る → 配布前に要確認**。ffmpeg(GPL ビルド)は使っていない |
| tauri / tauri-plugin-dialog | 2.12 / 2.8 | Apache-2.0 OR MIT | 著作権表示と許諾文。依存の rfd は MIT、tao は Apache-2.0 |
| rusqlite(SQLite 同梱) | 0.40 | MIT(SQLite はパブリックドメイン) | 同上 |
| React / React DOM | 18.3 | MIT | 同上 |

### 話者の判別(2026-10-04 追加)

| 名前 | 版 | ライセンス | 確認元 | 配布時に必要なこと |
|---|---|---|---|---|
| WeSpeaker ResNet34-LM(`voxceleb_resnet34_LM.onnx`、26.5MB、SHA-256 `7bb2f06e…`) | Hugging Face `Wespeaker/wespeaker-voxceleb-resnet34-LM` | **CC BY 4.0**(モデルカードの `license: cc-by-4.0`)。VoxCeleb2 Dev で学習 | モデルカード原文 | 帰属表示(WeSpeaker プロジェクト)、ライセンスへのリンク、改変の有無。`legal/wespeaker-model-NOTICE.txt` に記載し同梱 |
| ONNX Runtime | 1.30.0(公式の osx-arm64 配布) | MIT | 配布物の LICENSE・ThirdPartyNotices.txt | 両方を同梱(`legal/`) |
| rustfft / ort / ndarray / csv / encoding_rs / zip | — | MIT OR Apache-2.0 など | `cargo metadata` | 自動生成の表記に含まれる |

- **sherpa-onnx(Rust の公式クレート)は使わない**: 静的リンクの既定で espeak-ng(GPL-3.0)を含むライブラリ群をリンクするため。話者の判別は、ONNX Runtime と自前の前処理(Kaldi 互換 fbank)・クラスタリングで実装した。
- pyannote の分割モデル(segmentation-3.0、MIT だが Hugging Face で利用条件への同意ゲートあり)は使っていない。
- **VoxCeleb のデータ自体の利用条件**(研究目的とされることがある)がモデルの商用利用に及ぶかは、モデルカード(CC BY 4.0)からは読み取れない。**販売前に要確認**(ReazonSpeech と同様の論点。モデルカードの許諾は商用可)。

推移的な依存に GPL・AGPL は無い。`r-efi`(MIT OR Apache-2.0 OR LGPL-2.1-or-later の選択式)は MIT を選べる。`webpki-roots` は CDLA-Permissive-2.0(ライセンス認証の通信用、表記が必要)。

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

## 2026-10-04 追記(要約の追加機能)

要約は有料版の追加機能。モデルはアプリに同梱せず、利用者が押したときだけ取得する。推論は llama.cpp を**別の実行ファイル**(サイドカー)にして本体から子プロセスで動かす(whisper.cpp の ggml と記号が衝突するため)。

| 名前 | 版 | ライセンス | 確認元 | 配布時に必要なこと |
|---|---|---|---|---|
| **Qwen3-4B-Instruct-2507**(Alibaba Cloud / Qwen)GGUF Q4_K_M(量子化は Unsloth) | `Qwen3-4B-Instruct-2507-Q4_K_M.gguf`、2,497,281,120 バイト、SHA-256 `3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597`(取得して照合済み) | **Apache-2.0** | `Qwen/Qwen3-4B-Instruct-2507` のモデルカード原文 `license: apache-2.0`(`license_link` は同リポジトリの LICENSE)。GGUF 配布 `unsloth/Qwen3-4B-Instruct-2507-GGUF` の `license: apache-2.0`、`base_model: Qwen/Qwen3-4B-Instruct-2507`。取得元 URL: `https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF/resolve/main/Qwen3-4B-Instruct-2507-Q4_K_M.gguf` | Apache-2.0 の写し(`legal/qwen3-model-LICENSE.txt`、`legal/extra.json` 経由で THIRD_PARTY_NOTICES に入る)。改変(再学習)なし、量子化のみ |
| llama.cpp(ggml authors。llama-cpp-sys-2 が同梱) | 同梱版(llama-cpp-sys-2 0.1.158) | MIT | クレート内 `llama.cpp/LICENSE` | `tools/gen_notices.py` が自動で全文を入れる(サイドカーの依存も対象にした) |
| llama-cpp-2 / llama-cpp-sys-2(utilityai の Rust 束ね) | 0.1.158 | MIT OR Apache-2.0 | crates.io のメタデータと LICENSE | 同上 |
| サイドカーの他の依存(serde、serde_json、tracing、enumflags2、thiserror など) | — | MIT / Apache-2.0 系(`cargo metadata` で確認) | 同上 | 同上 |

比べた他の候補(2026-10-04、モデルカード原文の `license:` を確認):
- `sbintuitions/sarashina2.2-3b-instruct-v0.1`: MIT(GGUF は mmnga 配布 Q4_K_M、2,066,390,112 バイト、SHA-256 `d96f4d98…`)。ライセンスは問題ないが、要約の忠実さで劣ったため不採用(`docs/spike-results.md`)。
- `Qwen/Qwen2.5-1.5B-Instruct-GGUF` / `Qwen2.5-7B-Instruct-GGUF`: Apache-2.0(Qwen2.5-3B は Apache でないため対象外)。日本語の質と大きさの釣り合いで 4B を優先し、試していない。
- `llm-jp/llm-jp-3-3.7b-instruct`: Apache-2.0。未試験。
- Phi-4-mini-instruct(MIT)は日本語が弱いとされるため試していない。
- 注意: 利用者の出力(要約)は自動の下書きで、画面で確認してから使う設計。Qwen3 の学習データ由来の追加条件はモデルカードに記載なし。

## ライセンスの署名の検証(2026-10-05)

| 名前 | 版 | ライセンス | 配布時に必要なこと |
|---|---|---|---|
| ed25519-dalek | 2.2.0 | BSD-3-Clause | `tools/gen_notices.py` が全文を THIRD_PARTY_NOTICES に入れる(2026-10-05 に生成して確認) |
| curve25519-dalek / curve25519-dalek-derive | 4.1.3 / 0.1.1 | BSD-3-Clause / MIT OR Apache-2.0 | 同上 |
| ed25519、signature、pkcs8、spki、der、zeroize、subtle(推移的な依存) | 2.2.3、2.2.0、0.10.2、0.7.3、0.7.10、1.9.0、2.6.1 | Apache-2.0 OR MIT(subtle は BSD-3-Clause) | 同上 |
| getrandom(発行コマンドの乱数) | 0.3.4 | MIT OR Apache-2.0 | 同上 |
