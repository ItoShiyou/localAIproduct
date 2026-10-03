# 依存ライブラリとモデルのライセンス

状態: **OCRエンジン・OCRモデル・Tauri・React等の依存は未決定のため、未記入。** 下は、いま `src-tauri/` で使っているRustクレートのみ。
ライセンス表記は、crates.io からダウンロードした各クレートの `Cargo.toml` の `license` 欄から転記したもの。
**配布物に必要な表記(著作権表示・ライセンス全文の同梱)の中身は、各リポジトリのLICENSEファイルで未確認**(配布前に確認し、同梱方法を決める。要確認)。

## 実行時の依存(`bizcard-logic`)

| 名前 | バージョン | ライセンス(Cargo.toml の表記) | 用途 |
|---|---|---|---|
| regex | 1.13.1 | MIT OR Apache-2.0 | ルール抽出の正規表現 |
| csv | 1.4.0 | Unlicense/MIT | CSV書き出し(暫定。core の汎用書き出しが入るまで) |
| encoding_rs | 0.8.42 | (Apache-2.0 OR MIT) AND BSD-3-Clause | Shift_JIS 変換 |
| serde / serde_json | 1.0.229 / 1.0.151 | MIT OR Apache-2.0 | OCR出力の読み込み |
| factory-core(`core/engine`) | 0.1.0 | 社内の共通コア(`publish = false`) | 無害化・文字コード・エラー型 |

`factory-core` 自身の依存(thiserror 1.0.69 MIT OR Apache-2.0、base64、ureq など)も配布物に入る。それらの表記は、依存ツリー全体を `cargo tree` で洗い出して確認する(未実施)。

## テスト専用の依存(配布物に入れない)

| 名前 | バージョン | ライセンス | 用途 |
|---|---|---|---|
| rusqlite | 0.32.1 | MIT | スキーマ案のテスト |
| libsqlite3-sys(SQLite 本体を同梱ビルド) | 0.30.1 | MIT(SQLite 本体はパブリックドメインとされる。**未確認**) | 同上 |

製品の SQLite 接続は、core の `db`(未実装)に合わせる。製品で rusqlite を使うことになった場合は、本表を実行時の依存に移す。

## モデル

なし(生成モデルは使わない。OCRモデルは OCR 方式の決定後に記入)。
