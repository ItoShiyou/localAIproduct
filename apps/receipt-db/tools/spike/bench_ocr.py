#!/usr/bin/env python3
"""スパイク用: テストセットの各ファイルから本文テキストを取り、時間とピークメモリを測る。
使い方: bench_ocr.py ENGINE OUTDIR   (ENGINE = pdftext | tesseract | rapidocr)
本番の実装ではない(Python は製品に含めない)。候補の精度・速度の目安を取るためだけの道具。
"""
import sys, os, json, time, resource, io, subprocess
import pymupdf
from PIL import Image

engine, outdir = sys.argv[1], sys.argv[2]
HERE = os.path.dirname(os.path.abspath(__file__))
FILES = os.path.join(HERE, "..", "..", "testset", "files")
os.makedirs(outdir, exist_ok=True)
THREADS = int(os.environ.get("THREADS", "4"))

if engine == "rapidocr":
    from rapidocr_onnxruntime import RapidOCR
    ocr = RapidOCR(intra_op_num_threads=THREADS, inter_op_num_threads=1)

def ocr_image(img: Image.Image) -> str:
    if engine == "tesseract":
        buf = io.BytesIO(); img.save(buf, "PNG")
        env = dict(os.environ, OMP_THREAD_LIMIT=str(THREADS))
        r = subprocess.run(["tesseract", "stdin", "stdout", "-l", "jpn", "--psm", os.environ.get("PSM", "6")] + (["--tessdata-dir", os.environ["TESSDATA"]] if os.environ.get("TESSDATA") else []),
                           input=buf.getvalue(), capture_output=True, env=env)
        return r.stdout.decode("utf-8", "replace")
    if engine == "rapidocr":
        import numpy as np
        res, _ = ocr(np.array(img.convert("RGB"))[:, :, ::-1])
        if not res: return ""
        # 行ごとに y 座標で並べ、同じ高さの断片は左から連結する
        items = sorted(((min(p[1] for p in b), min(p[0] for p in b), t) for b, t, _ in res))
        lines, cur, cy = [], [], None
        for y, x, t in items:
            if cy is None or abs(y - cy) < 14: cur.append((x, t)); cy = y if cy is None else cy
            else:
                lines.append("  ".join(t for _, t in sorted(cur))); cur = [(x, t)]; cy = y
        if cur: lines.append("  ".join(t for _, t in sorted(cur)))
        return "\n".join(lines)
    raise SystemExit("unknown engine")

rows = []
for fn in sorted(os.listdir(FILES)):
    p = os.path.join(FILES, fn)
    t0 = time.perf_counter(); used = "ocr"; text = ""
    if fn.endswith(".pdf"):
        doc = pymupdf.open(p); page = doc[0]
        text = page.get_text()
        if len(text.strip()) >= 10 and engine != "pdftext_only_ocr":
            used = "textlayer"
        else:
            pix = page.get_pixmap(dpi=200)
            img = Image.open(io.BytesIO(pix.tobytes("png"))).convert("L")
            if engine != "pdftext": text = ocr_image(img)
            else: text = ""
    else:
        if engine == "pdftext": text = ""
        else: text = ocr_image(Image.open(p).convert("L"))
    dt = time.perf_counter() - t0
    open(os.path.join(outdir, fn + ".txt"), "w").write(text)
    rows.append(dict(file=fn, used=used, sec=round(dt, 3)))
peak_mb = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024
json.dump(dict(engine=engine, threads=THREADS, peak_rss_mb=round(peak_mb), rows=rows),
          open(os.path.join(outdir, "_timing.json"), "w"), ensure_ascii=False, indent=1)
print(engine, "peak_rss_mb", round(peak_mb))
