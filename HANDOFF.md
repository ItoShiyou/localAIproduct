# 実装への引き渡し(最初に読む)

3本(領収書DB・議事録・名刺管理)を、**独立したアプリ・独立したリポジトリ**として Claude Code に作らせるための一式です。設計書は Claude Docs の「ローカルAI買い切りアプリ 設計書」にあります(本文の下側に古い節が残っている場合は、結論から始まる上側が最新)。

## 事前に用意するもの

| もの | 用途 | メモ |
|---|---|---|
| `claude`(Claude Code)、`tmux`、`git` | 並行起動 | `tools/parallel.sh` が使う |
| Rust(`rustup`)、Node.js(LTS) | Tauri 2 のビルド | |
| Mac | 開発・動作確認 | Apple Silicon |
| RTX 3060 機(Windows) | Windows の確認、CPUのみでの近似測定 | GPUを使わない設定で測る |
| 最低ライン相当の機械(あれば) | 発売前の最終確認 | Core i5-12500 級・メモリ16GB・GPUなし。無ければ近似で進み、発売前に実機か近い中古で確認 |

## 起動の順序(並行は最大3本)

```
cd factory
./tools/new_app.sh receipt-db      # specs/receipt-db の仕様書つきで作業場所を作る
./tools/new_app.sh minutes
./tools/new_app.sh bizcard
./tools/parallel.sh start receipt-db minutes   # まず2本。bizcard は後で
./tools/parallel.sh status
./tools/parallel.sh attach minutes             # 抜けるときは Ctrl+b → d
```

| 本 | 先に動く内容 | 待つもの |
|---|---|---|
| receipt-db | スパイク → 報告 → 実装。**共通コアの 🔲 部品もここで作る** | あなたの返事(スパイク後) |
| minutes | **スパイクだけ**(ノイズ除去と文字起こしの実測)→ 報告で止まる | 共通コアの完成(実装に入る前) |
| bizcard | 起動は後。receipt-db の OCR 方式が決まってから | receipt-db のスパイクの結果 |

共通コアができたら:

```
./tools/pull_core.sh receipt-db           # receipt-db で作ったコアを factory/core に取り込む
./tools/sync_core.sh minutes bizcard      # ほかのアプリに配る
./tools/parallel.sh start bizcard         # 3本目を起動
```

## あなたが判断すること(Claude Code は止まって聞く)

1. **各アプリのスパイクの報告**(合格ラインに届いたか、採用する技術)。特に議事録の文字起こしモデルの選定とライセンス。
2. 価格、販売ページの登録、署名証明書の取得、アカウント作成、配布ページの公開(Claude Code は実行しない)。
3. SQLCipher(アプリ内の暗号化)を初版に入れるか。名刺管理は他人の個人情報を扱うため、**発売前に必ず判断**。
4. 登録ページの公開: `config.json` の `studio_name` と `form_action` を設定して `python3 tools/make_lps.py` を実行(`dist/<slug>/index.html` ができる)。

## 確認していること / していないこと

- 確認済み: 共通コアの純ロジック(`core/engine/`)は、Linux で `cargo test` が42件通る。`new_app.sh` が3本の作業場所を仕様書つきで作れる。LP生成が3本分できる。
- **未確認**: Mac / Windows でのビルド、OCR・文字起こしの日本語精度と速度(最低ライン)、Polar のライセンスAPIの実接続、署名と配布、競合との差別化。仕様書の数値の合格ラインはすべて提案値。

## 実装に入る前の注意

- 仕様書の「要確認」に Claude Code が質問を書きます。`parallel.sh status` で、チェックボックスと要確認の数を見られます。
- テストデータに実在する他人の個人情報(名刺・領収書・録音)を使わせないこと。架空のデータで作る(`CLAUDE.md` に明記済み)。
- 競合調査は、各アプリの SPEC.md に「別に行う(未実施)」と書いた。登録ページを出す前に、あなたかこのチャットで行う。
