#!/usr/bin/env python3
"""架空の領収書・請求書50件を作る(乱数固定)。社名・番号・金額はすべて架空。
実行: python3 gen_testset.py  (要: pillow, reportlab, pymupdf, numpy / フォント: IPAゴシック)
出力: testset/files/*, testset/labels.json
"""
import json, random, os, io
from PIL import Image, ImageDraw, ImageFont, ImageFilter
import numpy as np
import fitz
from reportlab.pdfgen import canvas
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont

FONT = "/usr/share/fonts/truetype/fonts-japanese-gothic.ttf"
HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "files")
os.makedirs(OUT, exist_ok=True)
rnd = random.Random(20261003)
pdfmetrics.registerFont(TTFont("IPA", FONT))

VENDORS = ["株式会社ひまわり文具", "有限会社みどり商店", "合同会社あおぞら食品", "株式会社つばめ運輸",
           "有限会社さくら書店", "株式会社こもれび珈琲", "合同会社やまびこ電機", "株式会社はなまる薬局",
           "有限会社いずみ印刷", "株式会社ほくと事務機", "株式会社あけぼの商会", "合同会社なごみ茶房",
           "有限会社かもめ園芸", "株式会社ふじみ工業", "株式会社そよかぜ旅行社", "有限会社まつかぜ酒店"]
ITEMS = [("コピー用紙 A4", 580), ("ボールペン 10本", 880), ("ファイル", 330), ("コーヒー", 450), ("弁当", 780),
         ("切手", 840), ("電池", 698), ("タクシー代", 1840), ("書籍", 1650), ("印刷代", 2200),
         ("名刺用紙", 1080), ("駐車料金", 700), ("茶葉", 1200), ("梱包資材", 960)]

def era(y, m, d, style):
    if style == 0: return f"{y}年{m}月{d}日"
    if style == 1: return f"{y}/{m:02d}/{d:02d}"
    if style == 2: return f"{y}-{m:02d}-{d:02d}"
    if style == 3: return f"令和{y-2018}年{m}月{d}日" if y > 2019 else f"令和元年{m}月{d}日"
    return f"{y}.{m:02d}.{d:02d}"

def rand_date():
    y = rnd.choice([2024, 2025, 2026]); m = rnd.randint(1, 12); d = rnd.randint(1, 28)
    return y, m, d

def make_case(i, kind):
    y, m, d = rand_date()
    vendor = rnd.choice(VENDORS)
    inv = "T" + "".join(str(rnd.randint(0, 9)) for _ in range(13))
    n = rnd.randint(1, 4)
    items = rnd.sample(ITEMS, n)
    mixed = rnd.random() < 0.15
    rate = 10
    subtotal = sum(p for _, p in items)
    if mixed:
        rate = None
    elif rnd.random() < 0.25:
        rate = 8
    total = int(round(subtotal * (1 + (0.1 if rate in (10, None) else 0.08))))
    has_inv = rnd.random() < 0.85
    return dict(y=y, m=m, d=d, vendor=vendor, inv=inv if has_inv else None, items=items, rate=rate,
                mixed=mixed, subtotal=subtotal, total=total, datestyle=rnd.randint(0, 4))

def lines_for(c, kind):
    ds = era(c["y"], c["m"], c["d"], c["datestyle"])
    L = []
    if kind in ("receipt", "screenshot"):
        L.append(c["vendor"])
        if c["inv"]: L.append(f"登録番号 {c['inv']}")
        L.append(ds + " " + f"{rnd.randint(9,20)}:{rnd.randint(10,59)}")
        for n_, p in c["items"]: L.append(f"{n_}    ¥{p:,}")
        L.append(f"小計    ¥{c['subtotal']:,}")
        if c["mixed"]:
            L.append(f"8%対象 ¥{c['items'][0][1]:,}")
            L.append(f"10%対象 ¥{c['subtotal']-c['items'][0][1]:,}" if len(c['items'])>1 else "10%対象 ¥0")
        else:
            L.append(f"({c['rate'] or 10}%対象 ¥{c['subtotal']:,})")
        L.append(f"合計    ¥{c['total']:,}" if rnd.random() < 0.7 else f"税込合計    ¥{c['total']:,}")
    else:  # invoice
        L.append("請求書")
        L.append(f"{c['vendor']}")
        if c["inv"]: L.append(f"登録番号: {c['inv']}")
        L.append(f"発行日: {ds}")
        L.append("株式会社テスト宛")
        for n_, p in c["items"]: L.append(f"{n_}        {p:,}円")
        if c["mixed"]:
            L.append("8%対象 / 10%対象 を含む")
        else:
            L.append(f"消費税({c['rate'] or 10}%)")
        L.append(f"ご請求金額    ¥{c['total']:,}")
    return L

def label(c):
    return dict(date=f"{c['y']}-{c['m']:02d}-{c['d']:02d}", total=c["total"], tax_rate=c["rate"],
                invoice_no=c["inv"], vendor=c["vendor"])

def render_img(L, w=480, size=26, bg=235, ink=20, receipt=True):
    f = ImageFont.truetype(FONT, size)
    h = 60 + len(L) * (size + 14) + 40
    img = Image.new("L", (w, h), bg)
    dr = ImageDraw.Draw(img)
    y = 30
    for ln in L:
        dr.text((20, y), ln, font=f, fill=ink)
        y += size + 14
    return img

def degrade(img, mode):
    if mode == "fade":
        a = np.array(img).astype(np.float32)
        a = 255 - (255 - a) * 0.45  # 感熱紙の薄れ
        a += np.random.RandomState(1).normal(0, 6, a.shape)
        img = Image.fromarray(np.clip(a, 0, 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(0.8))
    elif mode == "skew":
        img = img.rotate(rnd.choice([-6, -4, 4, 6]), expand=True, fillcolor=200, resample=Image.BICUBIC)
        a = np.array(img).astype(np.float32)
        a += np.random.RandomState(2).normal(0, 8, a.shape)
        img = Image.fromarray(np.clip(a, 0, 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(0.6))
    return img

def text_pdf(path, L):
    c = canvas.Canvas(path, pagesize=(595, 842))
    c.setFont("IPA", 13)
    y = 780
    for ln in L:
        c.drawString(60, y, ln); y -= 24
    c.save()

def image_pdf(path, L):
    img = render_img(L, w=900, size=34, bg=255, ink=0)
    tmp = io.BytesIO(); img.convert("RGB").save(tmp, "JPEG", quality=85)
    doc = fitz.open(); page = doc.new_page(width=595, height=842)
    page.insert_image(fitz.Rect(30, 30, 565, 30 + 535 * img.height / img.width), stream=tmp.getvalue())
    doc.save(path)

labels = {}
plan = ([("receipt_clean", "receipt", None)] * 5 + [("receipt_fade", "receipt", "fade")] * 5 +
        [("receipt_skew", "receipt", "skew")] * 5 + [("invoice_textpdf", "invoice", None)] * 15 +
        [("invoice_scanpdf", "invoice", None)] * 10 + [("screenshot", "screenshot", None)] * 10)
for i, (cat, kind, deg) in enumerate(plan, 1):
    c = make_case(i, kind); L = lines_for(c, kind)
    name = f"{i:02d}_{cat}"
    if cat == "invoice_textpdf":
        fn = name + ".pdf"; text_pdf(os.path.join(OUT, fn), L)
    elif cat == "invoice_scanpdf":
        fn = name + ".pdf"; image_pdf(os.path.join(OUT, fn), L)
    elif cat == "screenshot":
        fn = name + ".png"
        render_img(L, w=700, size=30, bg=255, ink=40).save(os.path.join(OUT, fn))
    else:
        fn = name + ".png"
        degrade(render_img(L), deg).save(os.path.join(OUT, fn))
    labels[fn] = dict(category=cat, **label(c))
json.dump(labels, open(os.path.join(HERE, "labels.json"), "w"), ensure_ascii=False, indent=1)
print(len(labels), "files")
