# 配布時の表記(第三者ライセンス)と、その満たし方

作成: 2026-10-04。**法的な最終判断ではない**(販売前に専門家の確認を推奨)。ライセンスの条文に照らして、必要な表記をどう満たしているかの記録。

## 方針

- 文字起こしのモデルは**アプリに同梱**し、アプリ単体で動く形で売る(2026-10-04、利用者の決定)。
- 同梱する第三者のソフトウェア・モデルの**著作権表示と許諾文の全文**を、`THIRD_PARTY_NOTICES.txt` にまとめてアプリに同梱し、アプリの「設定 → ライセンスの全文を表示」からいつでも読めるようにする。
- 生成は `tools/gen_notices.py`(リポジトリ直下)。`apps/minutes/tools/prepare_bundle.sh` が、モデルの取得(ハッシュ照合)と一緒に作り直す。**依存を更新したら、配布前に必ず作り直す。**

## 集め方(`tools/gen_notices.py`)

1. Rust: `cargo metadata` の依存グラフを、macOS(aarch64)と Windows(x86_64)の両方の対象でたどり、**通常の依存**(配布物に入るもの。build / dev 依存は除く)を集める。各クレートの `LICENSE*` / `COPYING*` / `NOTICE*` / `COPYRIGHT*` の全文を入れる。Apache-2.0 の `NOTICE` ファイルがあるものは、それも入る(Apache-2.0 第4節(d))。
2. クレートに全文が無いもの(例: symphonia)は、SPDX の標準テキスト(`tools/licenses/`、spdx/license-list-data から取得)と、作者(Cargo.toml の authors)・取得元を入れる。
3. クレートが同梱するネイティブのソース(whisper-rs-sys の whisper.cpp / ggml、libsqlite3-sys の SQLite)の表記も入れる。
4. npm: 画面の JS に入る依存(react、react-dom、scheduler、loose-envify、js-tokens)の LICENSE。
5. Rust / npm の外の部品(モデルの重み)は `apps/minutes/legal/extra.json` に書く。
6. 全文を用意できない部品があれば、終了コード 2 で止まる(配布前に気付けるように)。2026-10-04 時点で 327 件、欠けなし。

## ライセンスごとの義務と、満たし方

| ライセンス | 主な部品 | 義務(要旨) | 満たし方 |
|---|---|---|---|
| MIT | Whisper のモデルの重み(OpenAI)、whisper.cpp / ggml、Tauri の多く、React | 著作権表示と許諾文を、ソフトウェアの複製に含める | `THIRD_PARTY_NOTICES.txt` に全文。アプリ内で表示 |
| Apache-2.0 | Tauri の一部、tao など | ライセンスの写しを渡す。NOTICE ファイルがあれば、その内容を表示する。改変したファイルにはその旨 | 全文と NOTICE を同梱。改変はしていない |
| BSD-3-Clause | nnnoiseless(RNNoise:Xiph.Org、Mozilla、Jean-Marc Valin ほか) | バイナリで配る場合、著作権表示・条件・免責を、文書かほかの資料に含める。名前を宣伝に使わない | 全文を同梱・表示。販売ページで RNNoise・Mozilla などの名前を推薦の形で使わない |
| **MPL-2.0** | symphonia(音声の読み込み)、cssparser、selectors ほか | 実行形式で配る場合、そのファイルのソースコードの入手方法を知らせる(第3.2節)。ライセンスの写し。改変した MPL のファイルは公開 | **改変せずに使用**。`THIRD_PARTY_NOTICES.txt` の冒頭に、MPL の部品とソースの入手先(GitHub と crates.io の版つきURL)を列挙し、各部品に MPL-2.0 の全文を入れている。アプリ全体を公開する義務は無い(ファイル単位のコピーレフト) |
| Unlicense / CC0 / 0BSD / MIT-0 | whisper-rs ほか | 表示義務なし | 参考として掲載 |
| ISC / Zlib / BSL-1.0 | 一部のクレート | 著作権表示と許諾文 | 全文を掲載 |
| Unicode-3.0 | Unicode のデータ表(文字の正規化など) | 著作権表示と許諾文 | 全文を掲載 |
| CDLA-Permissive-2.0 | webpki-roots(TLS の証明書の一覧。ライセンス認証の通信用) | 許諾文を含める | 全文を掲載 |
| パブリックドメイン | SQLite | なし | 参考として掲載(libsqlite3-sys の MIT は掲載) |

- **GPL・AGPL・LGPL のみの部品は含まない**(`cargo metadata` で確認。`r-efi` は MIT / Apache-2.0 / LGPL の選択式で、MIT を選ぶ)。ffmpeg(GPL ビルド)は使っていない。
- **ReazonSpeech 系・kotoba-whisper は含まない**(学習データの「著作権法30条の4の目的に限る」条件の扱いが未確認のため。`docs/licenses.md`)。
- WebView(macOS の WKWebView、Windows の WebView2)は OS の部品で、同梱しない。

## アプリ内の表示(実装済み)

- 設定 → 「使っているソフトウェアとモデル」: 主な部品の一覧と、「ライセンスの全文を表示」ボタン(同梱の `THIRD_PARTY_NOTICES.txt` を表示)。
- 画面上部の注意書き: 「文字起こしには誤りが含まれます。重要な箇所は音声で確認してください。録音の同意を得る責任は利用者にあります。」(SPEC の注意書き)
- 初回の取り込み時: 録音の同意についての確認。
- 通信一覧(設定画面)。

## 販売前に残っていること(ライセンス表記の外)

- `tauri.conf.json` の `copyright`(アプリ自体の著作権者名)が仮の文言。販売名義が決まったら記入する。
- 利用規約(EULA)・プライバシーポリシー・返金ポリシー・特定商取引法に基づく表記・サポート範囲: 販売ページの準備のとき(販売登録と一緒に、利用者が後で行う)。
- アプリの名前が「議事録(仮称)」のまま。正式名称が他社の商標と重ならないかの確認。
- 依存のライセンスは版によって変わりうるため、版を上げたら `prepare_bundle.sh` で作り直し、差分を確認する。
