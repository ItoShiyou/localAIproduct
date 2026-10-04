# core/ の変更記録(議事録から)

議事録の実装で必要になった共通コアの変更。`core/` を変えるコミットは先頭に `core:` を付け、`tools/sync_core.sh --all` で各アプリに配っている。
(それ以前の変更は `apps/receipt-db/docs/core-changes.md`)

## 2026-10-04

| 部品 | 変更 | 理由 |
|---|---|---|
| `model_manager`(新規) | `ModelSpec`、`ModelManager`(`status` / `is_installed` / `installed_path` / `download` / `delete`)、`sha256_file`。取得の再開(`<名前>.part` と HTTP Range、応じないサーバーなら最初から)、SHA-256 の照合(照合済みの印 `<名前>.sha256`)、空き容量不足のエラー、中断 | 議事録の文字起こしモデル(574MB)を、利用者の操作で取得するため。`core/README.md` の 🔲 だった部品。通信は「モデルの取得(操作したときのみ)」で、各アプリの通信一覧に既に載っている |
| `error` | `CoreError::Model`、`CoreError::Cancelled` を追加 | 同上 |
| `Cargo.toml` | `sha2 = "0.10"`(MIT OR Apache-2.0)を追加 | ハッシュの照合 |

判断: 利用者から「承認を得ずに実装を続行してよい」と指示があったため(2026-10-04)、`CLAUDE.md` の「共通コアの変更は止まる」の例外として実装した。既存の API は変えていない(追加のみ)。
