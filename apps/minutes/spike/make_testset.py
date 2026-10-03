"""架空の台本から合成音声(OpenJTalk)のテストセットを作る。実在の人物・会社・録音は一切含まない。
雑音は合成(フィルタ雑音・クリック列・減衰+残響・2話者の重ね)。実録音ではないので、結果は「近似」。
使い方: python make_testset.py <出力ディレクトリ>   (乱数は固定。毎回同じ結果)"""
import sys, os, json
import numpy as np, soundfile as sf, scipy.signal as ss, pyopenjtalk

OUT = sys.argv[1] if len(sys.argv) > 1 else "../testset"
SR = 16000
rng = np.random.default_rng(20261003)

LINES = [
 "本日の定例会議を始めます。",
 "最初に、先月の売上の報告をお願いします。",
 "はい、先月の売上は前年の同じ月と比べて八パーセント増えました。",
 "新しい取引先との契約が二件決まったことが主な理由です。",
 "来週の月曜日までに、見積書を作成して共有します。",
 "システムの更新作業は、金曜日の夜に行う予定です。",
 "作業の間は、社内の共有フォルダに入れなくなります。",
 "その件について、担当者から説明をお願いできますか。",
 "了解しました。手順書を今日中に配ります。",
 "次の議題は、来月の研修の日程についてです。",
 "会議室は三階の第二会議室を押さえてあります。",
 "参加者は全部で十二名の予定です。",
 "資料は事前にメールで送りますので、目を通しておいてください。",
 "予算については、前回の会議で決めた範囲に収めたいと考えています。",
 "追加の費用が必要な場合は、事前に相談してください。",
 "問い合わせ対応の件数が、先週は少し増えました。",
 "原因は、新しい製品の説明書が分かりにくいという声が多かったためです。",
 "説明書の改訂版を、来月の初めに公開する予定です。",
 "次回の会議は、再来週の水曜日の午後二時からにします。",
 "それでは、本日の会議はこれで終わります。お疲れさまでした。",
 "ここで一度、これまでの内容をまとめておきます。",
 "決まったことは三つあります。見積書の共有、更新作業の日程、研修の会場です。",
 "宿題として、各自で今月の目標を見直してください。",
 "プロジェクトの進み具合は、おおむね予定どおりです。",
 "ただし、検査の工程が少し遅れています。",
 "人手が足りないので、別の部署から応援をお願いしたいです。",
 "上長に確認して、明日までに返事をします。",
 "新しい資料の名前は、四半期報告書の最終版にしましょう。",
 "データの保存場所は、これまでと変えずに社内のサーバーにします。",
 "以上で、私からの報告は終わりです。",
]

def tts(text, ht=0.0, speed=1.0):
    x, sr = pyopenjtalk.tts(text, speed=speed, half_tone=ht)
    x = x.astype(np.float32) / 32768.0
    return ss.resample_poly(x, SR, sr).astype(np.float32)

def gap(sec): return np.zeros(int(SR * sec), np.float32)

def speech(lines, ht, speed):
    parts = []
    for t in lines:
        parts += [tts(t, ht, speed), gap(rng.uniform(0.4, 1.2))]
    return np.concatenate(parts)

def rms(x): return float(np.sqrt(np.mean(x**2) + 1e-12))

def mix_at_snr(clean, noise, snr_db):
    n = np.resize(noise, len(clean))
    g = rms(clean) / (rms(n) * 10 ** (snr_db / 20))
    return clean + g * n

def aircon(n):   # 低域に寄ったフィルタ雑音
    w = rng.standard_normal(n); b, a = ss.butter(2, 600 / (SR / 2)); return ss.lfilter(b, a, w).astype(np.float32)
def keyboard(n): # ランダムなクリック列
    x = np.zeros(n, np.float32)
    for p in rng.integers(0, n - 400, size=n // (SR // 6)):
        k = rng.standard_normal(300) * np.exp(-np.arange(300) / 40.0); x[p:p+300] += k.astype(np.float32)
    return x
def far_mic(x):  # 減衰+簡易残響+小さな背景雑音
    ir = (rng.standard_normal(int(0.3 * SR)) * np.exp(-np.arange(int(0.3 * SR)) / (0.06 * SR))).astype(np.float32); ir[0] = 1
    y = ss.fftconvolve(x, ir)[:len(x)] * 0.25
    return y + 0.002 * rng.standard_normal(len(x)).astype(np.float32)

def write(name, x, ref_text, ref_clean=None):
    x = np.clip(x / max(1e-9, np.max(np.abs(x))) * 0.8, -1, 1)
    sf.write(os.path.join(OUT, name + ".wav"), x, SR, subtype="PCM_16")
    open(os.path.join(OUT, name + ".txt"), "w", encoding="utf-8").write(ref_text + "\n")

os.makedirs(OUT, exist_ok=True)
meta = {}
# 1) 基準: 1話者・雑音なし(約3分)
L1 = LINES[:]; c1 = speech(L1, 0.0, 1.0)
write("t01_clean", c1, "".join(L1)); sf.write(os.path.join(OUT, "t01_clean_ref.wav"), c1 / np.max(np.abs(c1)) * 0.8, SR, subtype="PCM_16")
ref = c1 / np.max(np.abs(c1)) * 0.8
# 2) エアコン(SNR 5dB)  3) キーボード(SNR 5dB)  4) 遠いマイク
write("t02_aircon", mix_at_snr(ref, aircon(len(ref)), 5), "".join(L1))
write("t03_keyboard", mix_at_snr(ref, keyboard(len(ref)), 5), "".join(L1))
write("t04_farmic", far_mic(ref), "".join(L1))
# 5) 2話者の交互+一部同時発話(正解は主話者の文のみ。同時発話は誤りが増える想定の難条件)
L5 = LINES[10:20]; a = speech(L5, 0.0, 1.0); b = speech(LINES[:10], -3.0, 1.1)
n = max(len(a), len(b)); a = np.pad(a, (0, n - len(a))); b = np.pad(b, (0, n - len(b)))
write("t05_overlap", a + 0.5 * b, "".join(L5))
# 6) 速度測定用の長尺(約10分): 台本を4回繰り返し、エアコン雑音(SNR 10dB)
L6 = LINES * 4; c6 = speech(L6, 0.0, 1.0); c6 = c6 / np.max(np.abs(c6)) * 0.8
write("t06_long10min", mix_at_snr(c6, aircon(len(c6)), 10), "".join(L6))
for f in sorted(os.listdir(OUT)):
    if f.endswith(".wav"): i = sf.info(os.path.join(OUT, f)); meta[f] = round(i.duration, 1)
json.dump(meta, open(os.path.join(OUT, "durations.json"), "w"), indent=1)
print(meta)
