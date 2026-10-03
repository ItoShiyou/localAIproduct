#!/usr/bin/env python3
"""架空の名刺画像を50枚作る(`cards/NN.ocr.json` のレイアウトを画像に描き起こす)。

  /path/to/python gen_images.py     # Pillow が必要。出力: images/NN-a.png(きれい)、images/NN-b.jpg(劣化)

すべて架空。画像の文字は cards/NN.ocr.json の text(一部は『OCRの誤認識を模したテキスト』を
本来の正しい綴りに直したもの。OVERRIDES 参照)を、フォントで描いたもの。正解ラベルは cards/NN.expected.json。
実在の名刺・人物・会社とは無関係。フォントは IPAゴシック / IPAPゴシック / 文泉驛正黑(Linux 標準)。
"""
import json, os, random
from PIL import Image, ImageDraw, ImageFont, ImageFilter, ImageChops

HERE = os.path.dirname(os.path.abspath(__file__))
CARDS = os.path.join(HERE, "cards")
OUT = os.path.join(HERE, "images")
os.makedirs(OUT, exist_ok=True)
for f in os.listdir(OUT):
    os.remove(os.path.join(OUT, f))

FONTS = {
    "ipag": ("/usr/share/fonts/opentype/ipafont-gothic/ipag.ttf", 0),
    "ipagp": ("/usr/share/fonts/opentype/ipafont-gothic/ipagp.ttf", 0),
    "wqy": ("/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc", 0),
}
_cache = {}
def font(name, size):
    k = (name, size)
    if k not in _cache:
        path, idx = FONTS[name]
        _cache[k] = ImageFont.truetype(path, int(size), index=idx)
    return _cache[k]

# OCR誤認識を模したテキストを、画像では正しい綴りに戻す(画像の文字は正しく描き、読み違いは実OCRに任せる)
OVERRIDES = {
    "12": {"matsumoto@yamabiko-denki,example.com": "matsumoto@yamabiko-denki.example.com"},
    "23": {"riley.chen @ quartzlabs.example.com": "riley.chen@quartzlabs.example.com"},
    "25": {"電話 0 3 - 0 0 0 0 - 2 5 0 1": "電話 03-0000-2501"},
}

def is_ascii_run(c):
    return ord(c) < 128

def draw_vertical(img, text, x, y, size, fnt, fill):
    """縦書き。全角は正立で積み、英数字の連続は90度回して1つの塊として置く(実際の縦書き名刺の多くと同じ)。"""
    d = ImageDraw.Draw(img)
    cy = y
    i = 0
    while i < len(text):
        c = text[i]
        if is_ascii_run(c) and c != " ":
            j = i
            while j < len(text) and is_ascii_run(text[j]) and text[j] != " ":
                j += 1
            run = text[i:j]
            w = int(d.textlength(run, font=fnt)) + 2
            tmp = Image.new("RGBA", (w, int(size * 1.3)), (0, 0, 0, 0))
            ImageDraw.Draw(tmp).text((0, 0), run, font=fnt, fill=fill)
            tmp = tmp.rotate(-90, expand=True)
            img.paste(tmp, (x + int((size - tmp.width) / 2) + 1, cy), tmp)
            cy += tmp.height + 2
            i = j
        elif c == " ":
            cy += int(size * 0.5)
            i += 1
        else:
            cw = d.textlength(c, font=fnt)
            d.text((x + (size - cw) / 2, cy), c, font=fnt, fill=fill)
            cy += size
            i += 1

STYLES = [
    # (名前, 背景, 文字色, フォント)
    ("white-gothic", (255, 255, 255), (20, 20, 20), "ipag"),
    ("cream-propgothic", (250, 244, 225), (40, 30, 20), "ipagp"),
    ("navy-white-zenhei", (24, 40, 78), (245, 245, 245), "wqy"),
    ("gray-band-gothic", (236, 236, 236), (30, 30, 30), "ipag"),
    ("green-tint-zenhei", (226, 240, 228), (16, 50, 30), "wqy"),
]

def render(meta, style, degrade, rng):
    sname, bg, fg, fname = style
    W, H = int(meta["width"]), int(meta["height"])
    img = Image.new("RGB", (W, H), bg)
    d = ImageDraw.Draw(img)
    if "band" in sname:
        d.rectangle([0, 0, 18, H], fill=(70, 90, 160))
    if "cream" in sname:
        d.rectangle([8, 8, W - 9, H - 9], outline=(160, 140, 90), width=2)
    low = "low-confidence" in meta["tags"]
    if low:
        fg = (120, 120, 125)
        bg2 = (225, 225, 228)
        img.paste(bg2, [0, 0, W, H])
    for ln in meta["lines"]:
        t = OVERRIDES.get(meta["id"], {}).get(ln["text"], ln["text"])
        b = ln["bbox"]
        size = ln["bbox"]["w"] if ln["vertical"] else ln["bbox"]["h"]
        fnt = font(fname, size)
        if ln["vertical"]:
            draw_vertical(img, t, int(b["x"]), int(b["y"]), int(size), fnt, fg)
        else:
            if "zenhei" in fname or "wqy" in fname:
                d.text((b["x"] + 1, b["y"] + 1), t, font=fnt, fill=tuple(max(0, c - 50) if sum(bg) > 400 else min(255, c + 0) for c in bg))
            d.text((b["x"], b["y"]), t, font=fnt, fill=fg)
    return degrade(img, rng)

def noise(img, rng, amount):
    px = img.load()
    W, H = img.size
    for _ in range(int(W * H * amount)):
        x, y = rng.randrange(W), rng.randrange(H)
        v = rng.randrange(0, 255)
        px[x, y] = (v, v, v)
    return img

def rotated(img, deg, bg):
    return img.rotate(deg, resample=Image.BICUBIC, expand=True, fillcolor=bg)

def deg_clean(img, rng):
    return img

def make_degrade(kind):
    def f(img, rng):
        bg = img.getpixel((img.width - 3, img.height - 3))
        if kind == 0:   # 4度の傾き + ノイズ
            img = noise(rotated(img, 4, bg), rng, 0.01)
        elif kind == 1: # ぼかし + 低コントラスト
            img = img.filter(ImageFilter.GaussianBlur(1.1))
            img = Image.blend(img, Image.new("RGB", img.size, (200, 200, 200)), 0.35)
        elif kind == 2: # せん断(斜め撮影風) + ノイズ
            w, h = img.size
            img = img.transform((w + 60, h), Image.AFFINE, (1, 0.06, 0, 0, 1, 0), resample=Image.BICUBIC, fillcolor=bg)
            img = noise(img, rng, 0.006)
        elif kind == 3: # 影のグラデーション + 強いJPEG圧縮は保存時
            w, h = img.size
            grad = Image.linear_gradient("L").resize((w, h)).rotate(90)
            shade = Image.merge("RGB", [grad.point(lambda v: 255 - int(v * 0.35))] * 3)
            img = ImageChops.multiply(img, shade)
        else:           # -6度の傾き + ごま塩ノイズ + 小さいボカシ
            img = rotated(img, -6, bg).filter(ImageFilter.GaussianBlur(0.6))
            img = noise(img, rng, 0.015)
        return img
    return f

DESC = ["傾き4度+ノイズ", "ぼかし+低コントラスト", "せん断(斜め撮影風)+ノイズ", "影のグラデーション+JPEG圧縮強め", "傾き-6度+ごま塩ノイズ+ぼかし"]
manifest = []
ids = sorted(f[:2] for f in os.listdir(CARDS) if f.endswith(".ocr.json"))
for n, cid in enumerate(ids):
    meta = json.load(open(os.path.join(CARDS, f"{cid}.ocr.json")))
    # きれいな版(a): 背景・フォントを変える
    rng = random.Random(1000 + n)
    style_a = STYLES[n % len(STYLES)]
    ia = render(meta, style_a, deg_clean, rng)
    ia.save(os.path.join(OUT, f"{cid}-a.png"))
    manifest.append({"file": f"{cid}-a.png", "card": cid, "style": style_a[0], "degrade": "なし"})
    # 劣化版(b)
    rng = random.Random(2000 + n)
    style_b = STYLES[(n + 2) % len(STYLES)]
    kind = n % 5
    ib = render(meta, style_b, make_degrade(kind), rng)
    q = 30 if kind == 3 else 70
    ib.save(os.path.join(OUT, f"{cid}-b.jpg"), quality=q)
    manifest.append({"file": f"{cid}-b.jpg", "card": cid, "style": style_b[0], "degrade": DESC[kind]})
json.dump(manifest, open(os.path.join(OUT, "manifest.json"), "w"), ensure_ascii=False, indent=1)
print(len(manifest), "images")
