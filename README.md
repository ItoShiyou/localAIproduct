# factory — ローカルAI買い切りアプリの並行開発ライン

**実装に渡すときは、まず `HANDOFF.md` を読む。** 3本の仕様書は `specs/<slug>/` にある。

対象は **Mac(Apple Silicon)と Windows(x64)**。販売は **外部の販売プラットフォーム**(第一候補 Polar、予備 Lemon Squeezy)に任せ、決済・税・ライセンス発行は自作しない。選定の経緯は `docs/sales-platforms.md`。

## 流れ

1. **試験LPを出す**(作る前に需要を測る)
   - `config.json` を設定する: スタジオ名、`form_action`(登録の送信先。Googleフォームなど)、連絡先
   - `python3 tools/make_lps.py --serve` で3本(領収書DB・議事録・名刺管理)のLPを生成して確認(http://localhost:8000)
   - `dist/<slug>/` を静的ホスティングに載せ、noteの記事末尾から案ごとにリンクする
2. **集計して、作る3本を決める**
   - 登録のCSVを `data/signups.csv`(列: timestamp,idea,email,os,price)に置く
   - `python3 tools/report.py` で順位と候補を出す。登録が少ないうちは順位がぶれる
3. **販売の検証を最初の週に済ませる**(`docs/sales-platforms.md` の「最初の週にやる検証」)
   - テスト商品で、購入 → キー → アクティベート → 返金 → 失効を通す
4. **共通コアを作る**(最初の1本と同時に)
   - 仕様は `core/README.md`。最初の2〜3日の試作で、技術選定(第一候補 Tauri)を実機で確かめる
5. **アプリごとの作業場所を作って、並行で回す**
   - `./tools/new_app.sh <slug>` → `apps/<slug>/`(CLAUDE.md、SPEC.md、チェックリスト、docs、core のコピー入り)
   - `./tools/parallel.sh start <slug>...` で、1アプリ=1つのtmuxセッションで Claude Code が開発開始
   - `./tools/parallel.sh status` で進み具合、`attach <slug>` で中に入る(抜ける: Ctrl+b → d)
   - コアを更新したら `./tools/sync_core.sh --all`

## 守っている設計

- 同時実行は3本まで(`MAX_PARALLEL`、超えるなら `FORCE=1`)。確認とサポートがあなたに集まるため
- Claude は、価格・販売ページの登録・署名・アカウント作成・通信の追加・決済に当たるコードでは止まって、SPEC.md の「要確認」に書く
- AIの出力は、確認画面を通さないと保存・書き出しできない
- 「完全対応」などの断定表現は `templates/checklists/copy-rules.md` で禁止
- ライセンスは無期限。期限があるのは「無料更新の権利」だけ(`templates/checklists/support-policy.md`)

## 構成

```
config.json / ideas.json     LPの設定と、8案の中身
docs/sales-platforms.md      販売プラットフォームの選定と、未確認事項
templates/lp.html            LPのひな形
templates/app/               アプリの CLAUDE.md・SPEC.md・KICKOFF.md・run.sh
templates/checklists/        表現ルール・リリース確認・サポート方針
core/                        共通コア(仕様のみ)
tools/                       make_lps.py / report.py / new_app.sh / parallel.sh / sync_core.sh
apps/  dist/  data/          生成物(アプリ・LP・登録CSV)
```

## 動作環境

bash、Python 3、git、tmux、Claude Code(`claude` コマンド)。別のコマンドで起動したいときは `CLAUDE_CMD` を指定する。
