"""話者の判別の評価用に、架空の会議(4人)を macOS の `say` で合成する。
出力は testset/generated/(Git に入れない。Apple の音声の再配布条件が未確認のため、生成手順だけを置く)。
  python3 spike/make_diarize_testset.py
出力: diarize4.wav(16kHz モノラル)、diarize4.json(行ごとの 話者・開始・終了・文)
台本・人物はすべて架空。
"""
import json, os, subprocess, tempfile, wave

VOICES = {"A": "Kyoko", "B": "Eddy", "C": "Grandpa", "D": "Flo"}
LINES = [
    ("A", "それでは、来月の展示会に向けた打ち合わせを始めます。"),
    ("B", "よろしくお願いします。まず、ブースの場所が決まりました。"),
    ("A", "どのあたりになりましたか。"),
    ("B", "入口から入って右側の、二列目です。"),
    ("C", "人の流れが多い場所ですね。展示する製品の数を少し増やしてもよさそうです。"),
    ("D", "パンフレットは何部用意しましょうか。"),
    ("C", "前回は三百部でしたが、足りなくなったので五百部にしましょう。"),
    ("A", "では、パンフレットは五百部で手配をお願いします。"),
    ("D", "わかりました。来週の水曜日までに入稿します。"),
    ("B", "設営の業者には、前日の午後から入れるか確認しておきます。"),
    ("C", "当日の説明担当は、午前と午後で交代にしたいと思います。"),
    ("D", "午前は私が担当できます。"),
    ("B", "では、午後は私が入ります。"),
    ("A", "ありがとうございます。最後に、予算の確認をしておきましょう。"),
    ("C", "見積もりは前回より一割ほど高くなっていますが、範囲内です。"),
    ("A", "わかりました。今日の打ち合わせは以上です。お疲れさまでした。"),
]
GAP = 0.6  # 発話の間の無音(秒)


def main():
    out_dir = os.path.join(os.path.dirname(__file__), "..", "testset", "generated")
    os.makedirs(out_dir, exist_ok=True)
    frames, labels, t = b"", [], 0.0
    with tempfile.TemporaryDirectory() as tmp:
        for i, (spk, text) in enumerate(LINES):
            aiff, wav = os.path.join(tmp, f"{i}.aiff"), os.path.join(tmp, f"{i}.wav")
            subprocess.run(["say", "-v", VOICES[spk], "-o", aiff, text], check=True)
            subprocess.run(["afconvert", "-f", "WAVE", "-d", "LEI16@16000", "-c", "1", aiff, wav], check=True)
            w = wave.open(wav)
            data = w.readframes(w.getnframes())
            dur = w.getnframes() / 16000
            labels.append({"speaker": spk, "start": round(t, 3), "end": round(t + dur, 3), "text": text})
            frames += data + b"\0\0" * int(GAP * 16000)
            t += dur + GAP
    out = wave.open(os.path.join(out_dir, "diarize4.wav"), "wb")
    out.setnchannels(1); out.setsampwidth(2); out.setframerate(16000); out.writeframes(frames); out.close()
    json.dump(labels, open(os.path.join(out_dir, "diarize4.json"), "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    print(f"{len(LINES)} 行、{t:.1f} 秒 → {out_dir}")


if __name__ == "__main__":
    main()
