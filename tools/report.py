#!/usr/bin/env python3
"""登録CSVを集計して、案ごとの順位と「作る3本」の候補を出す。

入力: data/signups.csv  (列: timestamp,idea,email,os,price)
  フォームの送信先(Googleフォーム/スプレッドシート等)からエクスポートして置く。

使い方:
  python3 tools/report.py
  python3 tools/report.py --csv path/to/signups.csv --top 3

判定は目安。登録数が少ないうちは順位がぶれるので、最低でも案ごとに数十件、
かつ期間を揃えて(各LPに同じ導線で)集めてから判断する。
"""
import argparse
import csv
import statistics
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# 価格帯の代表値(円)。「買わない」は0扱い。
PRICE_MID = {
    "〜¥1,000": 800,
    "¥1,000〜2,000": 1500,
    "¥2,000〜3,000": 2500,
    "¥3,000〜5,000": 4000,
    "¥5,000以上": 6000,
    "買わない": 0,
}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--csv", default=str(ROOT / "data" / "signups.csv"))
    ap.add_argument("--top", type=int, default=3)
    ap.add_argument("--min", type=int, default=30, help="判断に必要な最低登録数の目安")
    args = ap.parse_args()

    path = Path(args.csv)
    if not path.exists():
        print(f"ファイルがありません: {path}", file=sys.stderr)
        return 1

    emails = defaultdict(set)
    prices = defaultdict(list)
    oses = defaultdict(lambda: defaultdict(int))
    with path.open(encoding="utf-8-sig", newline="") as f:
        for row in csv.DictReader(f):
            idea = (row.get("idea") or "").strip()
            email = (row.get("email") or "").strip().lower()
            if not idea or not email:
                continue
            emails[idea].add(email)
            p = (row.get("price") or "").strip()
            if p in PRICE_MID:
                prices[idea].append(PRICE_MID[p])
            o = (row.get("os") or "").strip()
            if o:
                oses[idea][o] += 1

    if not emails:
        print("有効な登録がありません。")
        return 0

    rows = []
    for idea, s in emails.items():
        n = len(s)
        pr = prices.get(idea, [])
        pay = [x for x in pr if x > 0]
        median = statistics.median(pay) if pay else 0
        will = (len(pay) / len(pr)) if pr else 0.0
        # 目安スコア = 登録数 × 購入意向率 × 価格の中央値。回答なしは意向率0.5として扱う。
        score = n * (will if pr else 0.5) * (median if median else 1000)
        top_os = max(oses[idea].items(), key=lambda kv: kv[1])[0] if oses[idea] else "-"
        rows.append((idea, n, len(pr), will, median, top_os, score))

    rows.sort(key=lambda r: r[6], reverse=True)
    print(f"{'順位':<4}{'案':<22}{'登録':>6}{'価格回答':>8}{'購入意向':>8}{'価格中央値':>10}  最多端末")
    for i, (idea, n, npr, will, med, top_os, _) in enumerate(rows, 1):
        print(f"{i:<4}{idea:<22}{n:>6}{npr:>8}{will:>8.0%}{int(med):>10}  {top_os}")

    print()
    picks = rows[: args.top]
    print("作る候補: " + ", ".join(r[0] for r in picks))
    thin = [r[0] for r in picks if r[1] < args.min]
    if thin:
        print(f"注意: 登録が{args.min}件未満の案があります({', '.join(thin)})。順位はまだぶれます。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
