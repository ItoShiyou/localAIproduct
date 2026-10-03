# 依存ライブラリとモデルのライセンス

状態: **OCRエンジン・OCRモデル・Tauri・React等の依存は未決定のため、未記入。** 下は、いま `src-tauri/` で使っているRustクレートのみ。
ライセンス表記は、crates.io からダウンロードした各クレートの `Cargo.toml` の `license` 欄から転記したもの。
**配布物に必要な表記(著作権表示・ライセンス全文の同梱)の中身は、各リポジトリのLICENSEファイルで未確認**(配布前に確認し、同梱方法を決める。要確認)。

## 実行時の依存(`bizcard-logic` + `core/engine`)

直接の依存の一覧と、配布時に必要な表記は、receipt-db の `docs/licenses.md`(同じ `core/engine` を使う)に従う。確認日・方法もそちらに書かれている。このアプリ固有の依存は次のとおり。

| 名前 | バージョン | ライセンス(Cargo.toml の表記) | 用途 |
|---|---|---|---|
| regex | 1.13.1 | MIT OR Apache-2.0 | ルール抽出の正規表現 |
| serde / serde_json | 1.0.229 / 1.0.151 | MIT OR Apache-2.0 | OCR出力・正解ラベルの読み込み |
| factory-core(`core/engine`) | 0.1.0 | 社内の共通コア(`publish = false`) | OCR・DB・CSV・エラー型 |

テスト専用(配布物に入れない): csv 1.4.0(Unlicense/MIT)、encoding_rs 0.8.42((Apache-2.0 OR MIT) AND BSD-3-Clause)、image 0.25(MIT OR Apache-2.0)。
SQLite は core の rusqlite(bundled)を使う(このアプリ独自の SQLite 依存は無い)。

## 実OCRの測定に使ったモデル(スパイクのみ。リポジトリには含めず、製品にも同梱しない)

receipt-db の `docs/licenses.md` の「モデルと辞書」と同じファイルを使った(取得元とSHA-256はそちら)。

| ファイル | 状態 |
|---|---|
| ch_PP-OCRv4_det(検出)、ch_PP-OCRv4_rec(認識・中国語、数字/英字/記号に強い) | `rapidocr-onnxruntime` 1.4.4(PyPI、Apache-2.0)の同梱ファイル。RapidOCR の公開ハッシュと一致を確認済み(receipt-db 側) |
| japan_PP-OCRv3_rec と japan_dict.txt(認識・日本語) | **出所未確認**: npm `multilingual-purejs-ocr` 1.0.1(ISC)の同梱ファイルで、モデル自体のライセンス表記がなく、公式の配布元とのハッシュ照合もできていない。**製品には同梱しない扱い。** 製品に入れるには、公式配布元からの取得と照合が先(要確認) |
| ONNX Runtime 1.30.0 共有ライブラリ(Linux x86_64) | MIT。Mac/Windows 版の取得は未確認 |

名刺管理は、日本語の名刺を読むために日本語の認識モデルが事実上必須(中国語モデル単独は仮名を落とす)。出所確認が済まないうちは、製品として日本語を読めない。

## モデル

生成モデルは使わない。
