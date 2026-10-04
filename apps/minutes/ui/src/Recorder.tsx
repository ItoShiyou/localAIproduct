import { useEffect, useRef, useState } from "react";
import type { Api } from "./api";
import type { ProcessOptions, RecordStatus } from "./types";
import { hms } from "./types";

/** マイクの音を 16kHz モノラルで集めて、0.5 秒ごとに渡す AudioWorklet(インラインで読み込む) */
const WORKLET = `
class Tap extends AudioWorkletProcessor {
  process(inputs) {
    const ch = inputs[0];
    if (ch && ch[0]) {
      const n = ch[0].length, out = new Float32Array(n);
      for (let c = 0; c < ch.length; c++) for (let i = 0; i < n; i++) out[i] += ch[c][i] / ch.length;
      this.port.postMessage(out, [out.buffer]);
    }
    return true;
  }
}
registerProcessor("tap", Tap);
`;

function toBase64Int16(x: Float32Array): string {
  const b = new Uint8Array(x.length * 2);
  const v = new DataView(b.buffer);
  for (let i = 0; i < x.length; i++) v.setInt16(i * 2, Math.max(-32768, Math.min(32767, Math.round(x[i] * 32767))), true);
  let s = "";
  for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode(...b.subarray(i, i + 0x8000));
  return btoa(s);
}

/** 音のサンプル数を変える(線形補間。AudioContext が 16kHz にできなかったとき用) */
function resample(x: Float32Array, from: number, to: number): Float32Array {
  if (from === to) return x;
  const n = Math.floor((x.length * to) / from), out = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    const p = (i * from) / to, a = Math.floor(p), f = p - a;
    out[i] = (x[a] ?? 0) * (1 - f) + (x[a + 1] ?? x[a] ?? 0) * f;
  }
  return out;
}

/** 録音中の表示と、マイクからの取り込み。始まったら onStarted(議事録の id)、止めたら onStopped を呼ぶ */
export function Recorder({ api, opts, limitMs, onStarted, onStopped, onCancel }: {
  api: Api; opts: ProcessOptions; limitMs: number | null; onStarted: (id: number) => void; onStopped: (id: number) => void; onCancel: () => void;
}) {
  const [st, setSt] = useState<RecordStatus | null>(null);
  const [phase, setPhase] = useState<"starting" | "recording" | "stopping">("starting");
  const [err, setErr] = useState<string | null>(null);
  const [askDiscard, setAskDiscard] = useState(false);
  const cleanup = useRef<() => void>(() => undefined);
  const started = useRef(false);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; cleanup.current(); };
  }, []);

  useEffect(() => {
    // 開発時の二重実行(StrictMode)でも、録音は1回だけ始める
    if (started.current) return;
    started.current = true;
    (async () => {
      let stream: MediaStream | null = null;
      try {
        stream = await navigator.mediaDevices.getUserMedia({ audio: { channelCount: 1, echoCancellation: true, noiseSuppression: false, autoGainControl: true } });
        const id = await api.recordStart(opts);
        if (!mounted.current) { stream.getTracks().forEach((t) => t.stop()); api.recordDiscard().catch(() => undefined); return; }
        onStarted(id);
        const ctx = new AudioContext({ sampleRate: 16000 });
        const url = URL.createObjectURL(new Blob([WORKLET], { type: "text/javascript" }));
        await ctx.audioWorklet.addModule(url);
        const src = ctx.createMediaStreamSource(stream);
        const node = new AudioWorkletNode(ctx, "tap");
        let buf: Float32Array[] = [], len = 0, sending = Promise.resolve();
        node.port.onmessage = (e: MessageEvent<Float32Array>) => {
          buf.push(e.data); len += e.data.length;
          if (len >= ctx.sampleRate / 2) {
            const all = new Float32Array(len);
            let o = 0;
            for (const c of buf) { all.set(c, o); o += c.length; }
            buf = []; len = 0;
            const pcm = toBase64Int16(resample(all, ctx.sampleRate, 16000));
            // 順番を守って送る
            sending = sending.then(() => api.recordPush(pcm).then(setSt).catch((x) => setErr(String(x))));
          }
        };
        src.connect(node);
        setPhase("recording");
        const s = stream;
        cleanup.current = () => {
          cleanup.current = () => undefined; // 2回目以降は何もしない(止めたあと、画面が消えるときにも呼ばれる)
          node.port.onmessage = null; src.disconnect(); node.disconnect(); s.getTracks().forEach((t) => t.stop());
          ctx.close().catch(() => undefined); URL.revokeObjectURL(url);
        };
      } catch (e) {
        stream?.getTracks().forEach((t) => t.stop());
        setErr(e instanceof DOMException && e.name === "NotAllowedError"
          ? "マイクを使えません。システム設定 → プライバシーとセキュリティ → マイク で、このアプリを許可してください"
          : String(e));
      }
    })();
  }, [api, opts, onStarted]);

  const stop = async () => {
    setPhase("stopping");
    cleanup.current();
    try { const d = await api.recordStop(); onStopped(d.meeting.id); }
    catch (e) { setErr(String(e)); }
  };
  const discard = async () => {
    cleanup.current();
    try { await api.recordDiscard(); } catch { /* 始まっていない */ }
    onCancel();
  };

  // 無料版は1件の上限で自動的に止める
  useEffect(() => {
    if (limitMs != null && phase === "recording" && (st?.elapsedMs ?? 0) >= limitMs - 600) stop();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [st, limitMs, phase]);

  const lvl = Math.min(1, (st?.level ?? 0) * 8);
  return (
    <div className="recorder card" data-testid="recorder" role="region" aria-label="録音">
      <div className="row">
        <span className={"rec-dot" + (phase === "recording" ? " on" : "")} />
        <b>{phase === "starting" ? "マイクを準備しています…" : phase === "stopping" ? "止めています…" : "録音中"}</b>
        <span className="mono">{hms(st?.elapsedMs ?? 0)}</span>
        <span className="meter" aria-label="音の大きさ"><span style={{ width: `${lvl * 100}%` }} /></span>
      </div>
      {limitMs != null && <p className="note">無料版は1件 {Math.floor(limitMs / 60000)} 分までで、自動的に止まります。</p>}
      <p className="note">文字は 10〜20 秒ほど遅れて、仮の文字(精度は低め)として出ます。止めると、正確なモデルで最初から文字起こしし直します。</p>
      {(st?.pendingChunks ?? 0) > 2 && <p className="note">文字起こしが追いついていません(待ち {st?.pendingChunks})。録音は続いています。</p>}
      {err && <p className="msg err">{err}</p>}
      <div className="row">
        <button className="btn primary" disabled={phase !== "recording"} onClick={stop}>止めて保存</button>
        {!askDiscard
          ? <button className="btn small ghost" disabled={phase === "stopping"} onClick={() => setAskDiscard(true)}>録音を捨てる</button>
          : <span className="ask">録音と文字を消します。<button className="btn small danger" onClick={discard}>捨てる</button><button className="btn small" onClick={() => setAskDiscard(false)}>やめる</button></span>}
        {err && phase !== "recording" && <button className="btn small" onClick={onCancel}>閉じる</button>}
      </div>
    </div>
  );
}
