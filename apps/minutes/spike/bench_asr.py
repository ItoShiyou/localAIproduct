"""文字起こしの実測用(未実行: 作成環境はモデル配布元に届かなかった)。モデルを取得できる環境で実行する。
候補(最大3つ)を faster-whisper(CTranslate2, int8, CPU)で同条件に測る。
  python bench_asr.py <testset> <モデル名またはdir> <threads>
モデル例: kotoba-tech/kotoba-whisper-v2.0-faster / Systran/faster-whisper-small / deepdml/faster-whisper-large-v3-turbo-ct2
ReazonSpeech(NeMo/k2)は別ランタイムのため、sherpa-onnx 版(reazonspeech-k2-v2)を別途測る。
出力: ファイルごとの 文字誤り率(CER・句読点と空白を除く)、処理時間/再生時間(RTF)、ピークRSS。"""
import sys, os, time, json, re, resource
import soundfile as sf, jiwer
from faster_whisper import WhisperModel
TS, MODEL, TH = sys.argv[1], sys.argv[2], int(sys.argv[3])
norm = lambda s: re.sub(r"[\s、。,.!?！？「」]", "", s)
m = WhisperModel(MODEL, device="cpu", compute_type="int8", cpu_threads=TH)
for f in sorted(x for x in os.listdir(TS) if x.endswith(".wav") and not x.endswith("_ref.wav")):
    ref = norm(open(os.path.join(TS, f[:-4] + ".txt"), encoding="utf-8").read())
    dur = sf.info(os.path.join(TS, f)).duration
    t0 = time.perf_counter()
    segs, _ = m.transcribe(os.path.join(TS, f), language="ja", beam_size=1, vad_filter=True, condition_on_previous_text=False)
    hyp = norm("".join(s.text for s in segs)); dt = time.perf_counter() - t0
    print(json.dumps(dict(model=MODEL, file=f, dur_s=round(dur, 1), proc_s=round(dt, 1), rtf=round(dt / dur, 3),
        cer=round(jiwer.cer(ref, hyp), 4), peak_rss_mb=round(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024)), ensure_ascii=False), flush=True)
