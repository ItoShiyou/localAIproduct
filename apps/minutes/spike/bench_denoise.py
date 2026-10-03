"""ノイズ除去の実測(RNNoise / DeepFilterNet3)。GPUなし。スレッド数は引数。
使い方: python bench_denoise.py <testsetディレクトリ> <DFN3モデルdir> <threads> <出力dir>"""
import sys, os, time, json, resource
sys.path.insert(0, os.path.dirname(__file__))
try: import shim  # torchaudio新版でdeepfilternetを読むための互換
except ImportError: pass
import numpy as np, soundfile as sf, scipy.signal as ss, torch
TS, DFN, TH, OUT, ONLY = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4], sys.argv[5]  # ONLY: rnnoise か dfn3(別プロセスで測る)
torch.set_num_threads(TH); torch.set_flush_denormal(True);  # 無音(0)入力でGRUが極端に遅くなるのを避ける
os.makedirs(OUT, exist_ok=True)

def align(est, ref, maxlag=3000):  # 処理遅延(フレーム遅れ)を相互相関で補正
    n = min(len(est), len(ref), 16000 * 30); c = ss.correlate(est[:n], ref[:n], mode="full", method="fft")
    mid = n - 1; k = int(np.argmax(c[mid - maxlag: mid + maxlag + 1])) - maxlag  # k>0: estがk遅れ
    return (est[k:], ref) if k >= 0 else (est, ref[-k:])

def sisdr(est, ref):
    est, ref = align(est, ref); n = min(len(est), len(ref)); est, ref = est[:n].astype(np.float64), ref[:n].astype(np.float64)
    est -= est.mean(); ref -= ref.mean()
    a = np.dot(est, ref) / np.dot(ref, ref); t = a * ref; e = est - t
    return 10 * np.log10(np.dot(t, t) / np.dot(e, e))

def rnnoise(x16):
    from pyrnnoise import RNNoise
    x48 = ss.resample_poly(x16, 3, 1)
    d = RNNoise(48000); pcm = (np.clip(x48, -1, 1) * 32767).astype(np.int16)[None, :]
    outs = [f for _, f in d.denoise_chunk(pcm, partial=True)]
    y = np.concatenate([np.asarray(o).reshape(-1) for o in outs]).astype(np.float32) / 32767
    return ss.resample_poly(y, 1, 3)

_dfn = None
def dfn3(x16):
    global _dfn
    from df.enhance import init_df, enhance
    if _dfn is None: _dfn = init_df(DFN, log_level="ERROR")
    x16 = x16 + (np.random.default_rng(0).standard_normal(len(x16)) * 1e-4).astype(np.float32)  # 完全な無音(0)だけの区間があると極端に遅くなる(実測)ため微小ノイズを足す
    m, st, _ = _dfn; sr = st.sr() if callable(st.sr) else st.sr
    x = torch.from_numpy(ss.resample_poly(x16, sr, 16000).astype(np.float32))[None]
    with torch.no_grad(): y = enhance(m, st, x)
    return ss.resample_poly(y[0].numpy(), 16000, sr).astype(np.float32)

res = []
for name in sorted(f for f in os.listdir(TS) if f.endswith(".wav") and not f.endswith("_ref.wav")):
    x, sr = sf.read(os.path.join(TS, name), dtype="float32"); assert sr == 16000
    ref = None
    if os.path.exists(os.path.join(TS, "t01_clean_ref.wav")) and name in ("t02_aircon.wav", "t03_keyboard.wav"):
        ref, _ = sf.read(os.path.join(TS, "t01_clean_ref.wav"), dtype="float32")
    for tag, fn in ((ONLY, {"rnnoise": rnnoise, "dfn3": dfn3}[ONLY]),):
        t0 = time.perf_counter(); c0 = time.process_time(); y = fn(x); dt = time.perf_counter() - t0; cpu = time.process_time() - c0
        sf.write(os.path.join(OUT, f"{name[:-4]}.{tag}.wav"), y, 16000, subtype="PCM_16")
        r = dict(file=name, model=tag, dur_s=round(len(x) / 16000, 1), proc_s=round(dt, 1), rtf=round(dt / (len(x) / 16000), 3), cpu_s=round(cpu, 1))
        if ref is not None: r["sisdr_in"] = round(sisdr(x, ref), 1); r["sisdr_out"] = round(sisdr(y, ref), 1)
        r["peak_rss_mb"] = round(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024)
        res.append(r); print(json.dumps(r, ensure_ascii=False), flush=True)
json.dump(res, open(os.path.join(OUT, f"denoise_results_{ONLY}.json"), "w"), indent=1)
