import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { Api } from "./api";
import { OptionsForm, optionsValid } from "./OptionsForm";
import { notify } from "./toast";
import type { Detail, ExportFormat, Plan, ProcessOptions, Progress, Segment, SettingsInfo, Todo } from "./types";
import { LANGUAGE_LABEL, STATE_LABEL, hms, setSpeakerOrder, speakerColor } from "./types";

const SPEEDS = [0.75, 1, 1.25, 1.5, 2];
import { SummaryPanel } from "./SummaryPanel";

type View = "read" | "edit" | "memo" | "summary";

interface RowActions {
  seek(ms: number, play?: boolean): void;
  setSpeaker(id: number, name: string): void;
  editText(id: number, text: string): void;
  split(id: number, at: number): void;
  mergeNext(id: number): void;
  revert(id: number): void;
  typing(): void;
  doneTyping(): void;
}

/** 1つの文。再生位置が変わるたびに全部を描き直さないよう、memo にする */
const Row = memo(function Row({ s, last, playing, hit, low, actions, readOnly }: {
  s: Segment; last: boolean; playing: boolean; hit: boolean; low: boolean; speakers?: string[]; actions: React.MutableRefObject<RowActions>; readOnly: boolean;
}) {
  const ta = useRef<HTMLTextAreaElement>(null);
  // 文の長さに合わせて高さを変える(余白を作らない)
  const fit = () => { const t = ta.current; if (t) { t.style.height = "auto"; t.style.height = `${t.scrollHeight + 2}px`; } };
  useLayoutEffect(fit, [s.text]);
  if (readOnly) {
    return (
      <li id={`seg-${s.id}`} className={"seg provisional" + (playing ? " playing" : "")} data-testid="segment" onClick={() => actions.current.seek(s.startMs, false)}>
        <div className="seg-left"><span className="time">{hms(s.startMs)}</span>{s.chunkIdx < 0 && <span className="badge muted">仮</span>}</div>
        <p className="seg-text">{s.text}</p>
      </li>
    );
  }
  const color = speakerColor(s.speaker);
  return (
    <li id={`seg-${s.id}`} className={"seg" + (playing ? " playing" : "") + (low ? " low" : "") + (hit ? " hit" : "")} data-testid="segment"
      onClick={(e) => {
        // 文のどこを押しても、その位置へ再生位置を移す(入力欄・ボタンを押したときは除く)
        const t = e.target as HTMLElement;
        if (!t.closest("button, input, select")) actions.current.seek(s.startMs, false);
      }}>
      <div className="seg-left">
        <button className="time" onClick={() => actions.current.seek(s.startMs, true)} title="ここから再生">{hms(s.startMs)}</button>
        <label className="spk-pick" style={{ color }}>
          <i style={{ background: s.speaker ? color : "var(--line-strong)" }} />
          <input list="speakers" defaultValue={s.speaker} placeholder="話者" aria-label="話者"
            onBlur={(e) => { if (e.target.value !== s.speaker) actions.current.setSpeaker(s.id, e.target.value); }}
            onKeyDown={(e) => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} />
        </label>
      </div>
      <textarea ref={ta} defaultValue={s.text} rows={1} aria-label={`${hms(s.startMs)} の文`}
        onInput={() => { fit(); actions.current.typing(); }}
        onKeyDown={(e) => { if (e.key === "Escape") (e.target as HTMLTextAreaElement).blur(); }}
        onBlur={(e) => { actions.current.doneTyping(); if (e.target.value !== s.text) actions.current.editText(s.id, e.target.value); }} />
      <div className="seg-right">
        <div className="seg-actions">
          <span className="seg-tools">
            <button className="link" title="カーソルの位置で2つに分けます" onClick={() => {
              const at = ta.current ? [...ta.current.value.slice(0, ta.current.selectionStart)].length : 0;
              actions.current.split(s.id, at);
            }}>ここで分割</button>
            {!last && <button className="link" onClick={() => actions.current.mergeNext(s.id)}>次と結合</button>}
            {s.edited && s.rawText && <button className="link" onClick={() => actions.current.revert(s.id)}>元の結果に戻す</button>}
          </span>
          <button className="listen" onClick={() => actions.current.seek(s.startMs, true)} title="この文から再生">▶ 試聴</button>
        </div>
        {(low || s.edited) && <div className="seg-badges">{low && <span className="badge">要確認</span>}{s.edited && <span className="badge muted">修正済み</span>}</div>}
      </div>
    </li>
  );
});

/** 下部のプレーヤーの波形(山の高さ)と、話者の色の帯。押すとその位置へ */
function Wave({ peaks, durationMs, nowMs, segs, onSeek }: { peaks: number[]; durationMs: number; nowMs: number; segs: Segment[]; onSeek: (ms: number) => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const pos = durationMs > 0 ? Math.min(1, nowMs / durationMs) : 0;
  const bars = peaks.length ? peaks : Array.from({ length: 160 }, () => 0.08);
  return (
    <div className="wave" ref={ref} role="slider" aria-label="再生位置" aria-valuemin={0} aria-valuemax={durationMs} aria-valuenow={nowMs}
      onClick={(e) => { const r = ref.current!.getBoundingClientRect(); onSeek(((e.clientX - r.left) / r.width) * durationMs); }}>
      <div className="bars">{bars.map((v, i) => <i key={i} className={i / bars.length < pos ? "on" : ""} style={{ height: `${Math.max(6, v * 100)}%` }} />)}</div>
      <div className="band">{durationMs > 0 && segs.filter((s) => s.speaker).map((s) => (
        <span key={s.id} style={{ left: `${(s.startMs / durationMs) * 100}%`, width: `${Math.max(0.2, ((s.endMs - s.startMs) / durationMs) * 100)}%`, background: speakerColor(s.speaker) }} />
      ))}</div>
      <div className="head" style={{ left: `${pos * 100}%` }} />
    </div>
  );
}

/** 議題・決定事項・ToDo(手で書く定型欄) */
function Notes({ d, onSave }: { d: Detail; onSave: (agenda: string, decisions: string, todos: Todo[]) => void }) {
  const m = d.meeting;
  const [agenda, setAgenda] = useState(m.agenda);
  const [decisions, setDecisions] = useState(m.decisions);
  const [todos, setTodos] = useState<Todo[]>(m.todos);
  // 入力中の内容を、保存の応答(古いことがある)で上書きしないよう、画面側の値を正とする(議事録を切り替えると作り直される)
  const saved = useRef({ agenda: m.agenda, decisions: m.decisions, todos: JSON.stringify(m.todos) });
  const save = (t = todos) => {
    const now = { agenda, decisions, todos: JSON.stringify(t) };
    if (now.agenda === saved.current.agenda && now.decisions === saved.current.decisions && now.todos === saved.current.todos) return;
    saved.current = now;
    onSave(agenda, decisions, t);
  };
  const setTodo = (i: number, p: Partial<Todo>) => setTodos(todos.map((t, j) => (j === i ? { ...t, ...p } : t)));
  return (
    <div className="notes" data-testid="notes">
      <label>議題<textarea value={agenda} rows={2} placeholder="1行に1つ" onChange={(e) => setAgenda(e.target.value)} onBlur={() => save()} /></label>
      <label>決定事項<textarea value={decisions} rows={2} placeholder="1行に1つ" onChange={(e) => setDecisions(e.target.value)} onBlur={() => save()} /></label>
      <div className="todos">
        <span className="lbl">ToDo</span>
        {todos.map((t, i) => (
          <div key={i} className="todo">
            <input type="checkbox" checked={t.done} aria-label="済み" onChange={(e) => { const n = todos.map((x, j) => (j === i ? { ...x, done: e.target.checked } : x)); setTodos(n); save(n); }} />
            <input className="grow" value={t.text} placeholder="やること" onChange={(e) => setTodo(i, { text: e.target.value })} onBlur={() => save()} />
            <input className="owner" value={t.owner} placeholder="担当" onChange={(e) => setTodo(i, { owner: e.target.value })} onBlur={() => save()} />
            <input type="date" value={t.due} aria-label="期限" onChange={(e) => setTodo(i, { due: e.target.value })} onBlur={() => save()} />
            <button className="link" onClick={() => { const n = todos.filter((_, j) => j !== i); setTodos(n); save(n); }}>削除</button>
          </div>
        ))}
        <button className="btn small" onClick={() => setTodos([...todos, { text: "", owner: "", due: "", done: false }])}>＋ ToDo を追加</button>
      </div>
    </div>
  );
}

export interface PrintOptions { notes: boolean; body: boolean; times: boolean; speakers: boolean }

/** 印刷用(画面では隠し、印刷のときだけ出す。PDF はここから作る) */
function PrintDoc({ d, o }: { d: Detail; o: PrintOptions }) {
  const m = d.meeting;
  const paras: { t: number; sp: string; tx: string }[] = [];
  for (const s of d.segments) {
    if (!s.text.trim()) continue;
    const last = paras[paras.length - 1];
    if (last && last.sp === s.speaker) last.tx += s.text.trim();
    else paras.push({ t: s.startMs, sp: s.speaker, tx: s.text.trim() });
  }
  const lines = (x: string) => x.split("\n").map((l) => l.trim()).filter(Boolean);
  const speakers = [...new Set(d.segments.map((s) => s.speaker).filter(Boolean))];
  const hasNotes = o.notes && (m.agenda.trim() || m.decisions.trim() || m.todos.length > 0);
  return (
    <article className="print-doc" aria-hidden="true" data-testid="print-doc">
      <header className="pd-head">
        <p className="pd-kind">議事録</p>
        <h1>{m.title}</h1>
        <table className="pd-meta"><tbody>
          {m.heldOn && <tr><th>日付</th><td>{m.heldOn}</td></tr>}
          {m.participantsText.trim() && <tr><th>参加者</th><td>{m.participantsText}</td></tr>}
          {m.durationMs != null && <tr><th>録音の長さ</th><td>{hms(m.durationMs)}</td></tr>}
          {o.speakers && speakers.length > 0 && (
            <tr><th>話者</th><td>{speakers.map((sp) => <span key={sp} className="pd-spk"><i style={{ background: speakerColor(sp) }} />{sp}</span>)}</td></tr>
          )}
          {m.tags.length > 0 && <tr><th>タグ</th><td>{m.tags.join("、")}</td></tr>}
        </tbody></table>
      </header>
      {hasNotes && (
        <section className="pd-notes">
          {m.agenda.trim() && <><h2>議題</h2><ol>{lines(m.agenda).map((l, i) => <li key={i}>{l}</li>)}</ol></>}
          {m.decisions.trim() && <><h2>決定事項</h2><ul>{lines(m.decisions).map((l, i) => <li key={i}>{l}</li>)}</ul></>}
          {m.todos.length > 0 && (
            <>
              <h2>ToDo</h2>
              <table className="pd-todo">
                <thead><tr><th className="c">状態</th><th>内容</th><th>担当</th><th>期限</th></tr></thead>
                <tbody>{m.todos.map((t, i) => <tr key={i}><td className="c">{t.done ? "済" : "□"}</td><td>{t.text}</td><td>{t.owner}</td><td>{t.due}</td></tr>)}</tbody>
              </table>
            </>
          )}
        </section>
      )}
      {o.body && (
        <section className="pd-body">
          <h2>発言の記録</h2>
          {paras.map((p, i) => (
            <div key={i} className="pd-row">
              {(o.times || o.speakers) && (
                <div className="pd-who">
                  {o.times && <span className="pd-ts">{hms(p.t)}</span>}
                  {o.speakers && p.sp && <span className="pd-name" style={{ color: speakerColor(p.sp) }}>{p.sp}</span>}
                </div>
              )}
              <p className="pd-text">{p.tx}</p>
            </div>
          ))}
        </section>
      )}
      <footer className="pd-foot">この記録は録音の文字起こしをもとに作成しています。文字起こしには誤りが含まれる場合があります。</footer>
    </article>
  );
}

/** PDF にする前に、含める内容を選ぶ */
function PrintDialog({ value, onChange, onPrint, onClose }: { value: PrintOptions; onChange: (o: PrintOptions) => void; onPrint: () => void; onClose: () => void }) {
  const box = (k: keyof PrintOptions, label: string) => (
    <label className="check"><input type="checkbox" checked={value[k]} onChange={(e) => onChange({ ...value, [k]: e.target.checked })} /> {label}</label>
  );
  return (
    <div className="modal" role="dialog" aria-label="PDF にする">
      <div className="modal-body">
        <h2>PDF にする</h2>
        <div className="col">
          {box("notes", "議題・決定事項・ToDo")}
          {box("body", "発言の記録")}
          {box("times", "発言の時刻")}
          {box("speakers", "話者の名前")}
        </div>
        <p className="note">次に開く印刷の画面で、左下の「PDF」→「PDF として保存」を選んでください(A4 で作ります)。</p>
        <div className="row">
          <button className="btn primary" disabled={!value.notes && !value.body} onClick={onPrint}>印刷の画面を開く</button>
          <button className="btn" onClick={onClose}>やめる</button>
        </div>
      </div>
    </div>
  );
}

/** 1つの議事録の確認画面 */
export function DetailView({ api, id, version, seekTo, settings, plan, progress, jobsActive, jobError, onChanged, onDeleted, onRetry, onReprocessed, onResume, onOpenSettings }: {
  api: Api; id: number; version: number; seekTo: { ms: number; n: number } | null; settings: SettingsInfo | null; plan: Plan | null;
  progress: Progress | null; jobsActive: boolean; jobError: string | null;
  onChanged: () => void; onDeleted: () => void; onRetry: () => void; onReprocessed: () => void; onResume: () => void; onOpenSettings: () => void;
}) {
  const [d, setD] = useState<Detail | null>(null);
  const [audio, setAudio] = useState<string | null>(null);
  const [now, setNow] = useState(0);
  const [msg, setMsg] = useState<string | null>(null);
  const [meta, setMeta] = useState({ title: "", heldOn: "", participants: "", tags: "" });
  const [delAsk, setDelAsk] = useState(false);
  const [follow, setFollow] = useState(true);
  const [autoPause, setAutoPause] = useState(true);
  const [speed, setSpeed] = useState(1);
  const [find, setFind] = useState<{ open: boolean; q: string; r: string; hits: number[]; i: number }>({ open: false, q: "", r: "", hits: [], i: 0 });
  const [rename, setRename] = useState<{ from: string; to: string } | null>(null);
  const [redo, setRedo] = useState<ProcessOptions | null>(null);
  // 話者の判別し直し: 設定のダイアログ(人数)と、実行中の進み具合
  const [diarDlg, setDiarDlg] = useState<{ n: number | null } | null>(null);
  const [diar, setDiar] = useState<{ done: number; total: number; phase: string; cancelling: boolean } | null>(null);
  const diarHere = useRef(false); // この画面で始めたか(別の画面から戻ったときは、終わりを検知して読み直す)
  const [expOpen, setExpOpen] = useState(false);
  const expRef = useRef<HTMLDivElement>(null);
  const [printOpts, setPrintOpts] = useState<PrintOptions>({ notes: true, body: true, times: true, speakers: true });
  const [printAsk, setPrintAsk] = useState(false);
  const [view, setView] = useState<View>("edit");
  const [peaks, setPeaks] = useState<number[]>([]);
  const [paused, setPaused] = useState(true);
  const player = useRef<HTMLAudioElement>(null);
  const pausedByTyping = useRef(false);
  const lastPlaying = useRef<number | undefined>(undefined);
  // 保存の状態(順番待ちの数・最後の結果)
  const [pending, setPending] = useState(0);
  const [saveRes, setSaveRes] = useState<{ ok: true; at: Date } | { ok: false; why: string; retry: () => void } | null>(null);
  const chain = useRef<Promise<unknown>>(Promise.resolve());
  const statusRef = useRef<string | null>(null);
  const expectDraft = useRef(false);
  const fails = useRef(0);

  const apply = useCallback((x: Detail) => {
    // 確定のあとに内容を直すと、下書きに戻る。黙って戻ると「確定が効かない」ように見えるので知らせる
    if (statusRef.current === "confirmed" && x.meeting.status === "draft" && !expectDraft.current) {
      notify("info", "内容を直したため下書きに戻りました。もう一度「確定」を押してください");
    }
    expectDraft.current = false;
    statusRef.current = x.meeting.status;
    setD(x);
    setMeta({ title: x.meeting.title, heldOn: x.meeting.heldOn ?? "", participants: x.meeting.participantsText, tags: x.meeting.tags.join("、") });
  }, []);

  useEffect(() => {
    api.detail(id).then(apply).catch((e) => { setMsg(String(e)); notify("err", `読み込めませんでした: ${e}`); });
    api.audioUrl(id).then(setAudio).catch(() => setAudio(null));
  }, [api, id, version, apply]);

  // 処理中・録音中は少しずつ結果が増えるので、読み直す(録音中は最新の文まで送る)
  useEffect(() => {
    if (!d || !d.meeting.recording) return;
    const last = d.segments[d.segments.length - 1];
    if (last) document.getElementById(`seg-${last.id}`)?.scrollIntoView({ block: "end" });
  }, [d]);
  useEffect(() => {
    if (!d || (d.meeting.state !== "processing" && d.meeting.state !== "queued")) return;
    const t = setInterval(() => api.detail(id).then(apply).catch(() => undefined), 1500);
    return () => clearInterval(t);
  }, [api, id, d, apply]);

  // 話者の判別し直しの進み具合(別の議事録を開いて戻ったときも、続いていれば表示する)
  useEffect(() => {
    let alive = true;
    api.rediarizeStatus().then((s) => { if (alive && s && s.meetingId === id) { diarHere.current = false; setDiar({ done: s.done, total: s.total, phase: s.phase, cancelling: false }); } }).catch(() => undefined);
    return () => { alive = false; };
  }, [api, id]);
  const diarOn = diar !== null;
  useEffect(() => {
    if (!diarOn) return;
    let alive = true;
    const t = setInterval(async () => {
      try {
        const s = await api.rediarizeStatus();
        if (!alive) return;
        if (s && s.meetingId === id) setDiar((x) => x && { ...x, done: s.done, total: s.total, phase: s.phase });
        else if (!diarHere.current) { setDiar(null); api.detail(id).then(apply).catch(() => undefined); notify("ok", "話者を判別し直しました(「元に戻す」で戻せます)"); }
      } catch { /* 次の周期で再確認 */ }
    }, 500);
    return () => { alive = false; clearInterval(t); };
  }, [api, id, diarOn, apply]);
  // 書き出しメニューは、外を押す・Esc で閉じる
  useEffect(() => {
    if (!expOpen) return;
    const down = (e: MouseEvent) => { if (!expRef.current?.contains(e.target as Node)) setExpOpen(false); };
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") setExpOpen(false); };
    document.addEventListener("mousedown", down);
    document.addEventListener("keydown", key);
    return () => { document.removeEventListener("mousedown", down); document.removeEventListener("keydown", key); };
  }, [expOpen]);

  const startRediarize = async (n: number | null) => {
    setDiarDlg(null);
    diarHere.current = true;
    setDiar({ done: 0, total: 0, phase: "prepare", cancelling: false });
    try {
      await flush(); // 入力途中の内容は先に保存する(長い処理は順番待ちの列に入れない)
      const x = await api.rediarize(id, n);
      apply(x);
      onChanged();
      notify("ok", "話者を判別し直しました(「元に戻す」で戻せます)");
    } catch (e) {
      const why = String(e).replace(/^Error:\s*/, "");
      notify(/中断/.test(why) ? "info" : "err", /中断/.test(why) ? "話者の判別を中断しました。話者の表示はそのままです" : `話者を判別し直せませんでした: ${why}`);
    } finally { diarHere.current = false; setDiar(null); }
  };

  const seek = useCallback((ms: number, play = true) => {
    setNow(ms);
    const a = player.current;
    if (!a) return;
    a.currentTime = ms / 1000;
    if (play) a.play().catch(() => undefined);
  }, []);
  useEffect(() => { if (seekTo && d) { seek(seekTo.ms, false); document.getElementById(`seg-${d.segments.find((s) => s.endMs > seekTo.ms)?.id}`)?.scrollIntoView({ block: "center" }); } }, [seekTo, d, seek]);
  useEffect(() => { if (player.current) player.current.playbackRate = speed; }, [speed, audio]);
  // 波形(音声があって、処理が終わっているとき)
  useEffect(() => {
    if (!audio || !d || d.meeting.recording) return;
    let alive = true;
    api.waveform(id, 120).then((v) => alive && setPeaks(v)).catch(() => undefined);
    return () => { alive = false; };
  }, [api, id, audio, d?.meeting.recording, d?.meeting.state]);

  /** 変更を伴う操作は、1本の列に並べて順番に実行する(入力欄を離れた保存と「確定」が前後しないように)。成功したら新しい内容を返す */
  const run = useCallback((f: () => Promise<Detail>, after?: string, opts?: { toDraft?: boolean }): Promise<Detail | null> => {
    setPending((n) => n + 1);
    const job = chain.current.then(async () => {
      try {
        if (opts?.toDraft) expectDraft.current = true;
        const x = await f();
        apply(x);
        setSaveRes({ ok: true, at: new Date() });
        if (after) notify("ok", after);
        onChanged();
        return x;
      } catch (e) {
        expectDraft.current = false;
        fails.current++;
        const why = String(e).replace(/^Error:\s*/, "");
        setSaveRes({ ok: false, why, retry: () => { run(f, after, opts); } });
        notify("err", `保存できませんでした: ${why}`);
        return null;
      } finally { setPending((n) => n - 1); }
    });
    chain.current = job;
    return job;
  }, [apply, onChanged]);

  /** 入力中の欄を確定(blur)させ、その保存を含めて順番待ちが空になるまで待つ */
  const flush = useCallback(async () => {
    (document.activeElement as HTMLElement | null)?.blur?.();
    await new Promise((r) => setTimeout(r, 0));
    await chain.current;
  }, []);

  // 文の操作(memo の Row に渡すため、ref で最新を持つ)
  const actions = useRef<RowActions>(null as unknown as RowActions);
  actions.current = {
    seek,
    setSpeaker: (sid, name) => run(() => api.setSpeaker(sid, name, false)),
    editText: (sid, text) => run(() => api.editText(sid, text)),
    split: (sid, at) => run(() => api.split(sid, at)),
    mergeNext: (sid) => run(() => api.mergeNext(sid)),
    revert: (sid) => run(() => api.revertSegment(sid)),
    typing: () => {
      const a = player.current;
      if (autoPause && a && !a.paused) { a.pause(); pausedByTyping.current = true; }
    },
    doneTyping: () => {
      const a = player.current;
      if (pausedByTyping.current && a) { a.currentTime = Math.max(0, a.currentTime - 2); a.play().catch(() => undefined); }
      pausedByTyping.current = false;
    },
  };

  const segs = d?.segments ?? [];
  const playingIdx = segs.findIndex((s) => now >= s.startMs && now < s.endMs);
  const playing = playingIdx >= 0 ? segs[playingIdx].id : undefined;
  const lowIds = useMemo(() => segs.filter((s) => s.confidence < (d?.lowConfidence ?? 0)).map((s) => s.id), [segs, d?.lowConfidence]);
  const hitSet = useMemo(() => new Set(find.hits), [find.hits]);
  const counts = useMemo(() => {
    const c = new Map<string, number>();
    for (const s of segs) if (s.speaker) c.set(s.speaker, (c.get(s.speaker) ?? 0) + 1);
    return c;
  }, [segs]);

  // 再生に合わせて、読んでいる文を画面の中ほどに(入力中は動かさない)
  useEffect(() => {
    if (!follow || playing === undefined || playing === lastPlaying.current) return;
    lastPlaying.current = playing;
    const el = document.activeElement;
    if (el && (el.tagName === "TEXTAREA" || el.tagName === "INPUT")) return;
    if (player.current && !player.current.paused) document.getElementById(`seg-${playing}`)?.scrollIntoView({ block: "center", behavior: "smooth" });
  }, [playing, follow]);

  const goTo = useCallback((sid: number, focus = false) => {
    const s = segs.find((x) => x.id === sid);
    if (!s) return;
    seek(s.startMs, false);
    const li = document.getElementById(`seg-${sid}`);
    li?.scrollIntoView({ block: "center" });
    if (focus) li?.querySelector("textarea")?.focus();
  }, [segs, seek]);

  /** 要確認の文を順に(dir: 1 で次、-1 で前) */
  const nextLow = useCallback((dir: 1 | -1) => {
    if (!lowIds.length) return;
    const curIdx = playingIdx >= 0 ? playingIdx : -1;
    const order = segs.map((s, i) => ({ s, i })).filter((x) => lowIds.includes(x.s.id));
    const pick = dir === 1 ? order.find((x) => x.i > curIdx) ?? order[0] : [...order].reverse().find((x) => x.i < curIdx) ?? order[order.length - 1];
    goTo(pick.s.id, true);
  }, [lowIds, segs, playingIdx, goTo]);

  // キー操作
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const a = player.current;
      const tgt = e.target as HTMLElement;
      const editing = tgt.tagName === "TEXTAREA" || tgt.tagName === "INPUT" || tgt.tagName === "SELECT";
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key === "Enter") { e.preventDefault(); if (a) (a.paused ? a.play().catch(() => undefined) : a.pause()); return; }
      if (mod && e.key.toLowerCase() === "f") { e.preventDefault(); setFind((f) => ({ ...f, open: true })); setTimeout(() => document.getElementById("find-q")?.focus()); return; }
      if (editing) return;
      if (e.key === " ") { e.preventDefault(); if (a) (a.paused ? a.play().catch(() => undefined) : a.pause()); }
      else if (e.key === "ArrowLeft" && a) { e.preventDefault(); a.currentTime = Math.max(0, a.currentTime - 5); }
      else if (e.key === "ArrowRight" && a) { e.preventDefault(); a.currentTime += 5; }
      else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        const i = Math.min(segs.length - 1, Math.max(0, (playingIdx >= 0 ? playingIdx : -1) + (e.key === "ArrowDown" ? 1 : -1)));
        if (segs[i]) goTo(segs[i].id);
      } else if (e.key === "n") nextLow(1);
      else if (e.key === "p") nextLow(-1);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [segs, playingIdx, goTo, nextLow]);

  if (!d) return <div className="empty">{msg ?? "読み込み中…"}</div>;
  const m = d.meeting;
  const done = m.state === "done";
  setSpeakerOrder([...new Set(segs.map((s) => s.speaker).filter(Boolean))]);

  const saveMeta = () => {
    const tags = meta.tags.split(/[、,，\s]+/).map((t) => t.trim()).filter(Boolean);
    if (tags.join("\u0000") !== m.tags.join("\u0000")) run(() => api.setTags(id, tags));
    if (meta.title === m.title && meta.heldOn === (m.heldOn ?? "") && meta.participants === m.participantsText) return;
    run(() => api.updateMeta(id, meta.title, meta.heldOn || null, meta.participants));
  };

  const confirm = async () => {
    const before = fails.current;
    await flush();
    if (fails.current !== before) { notify("err", "直した内容を保存できなかったため、確定しませんでした。保存し直してから「確定」を押してください"); return; }
    const x = await run(() => api.confirm(id));
    if (!x) return;
    if (x.meeting.status === "confirmed") notify("ok", "確定しました。書き出せます");
    else notify("err", "確定できませんでした。下書きのままです。もう一度「確定」を押してください");
  };

  const exp = async (f: ExportFormat | "wav" | "pdf") => {
    try {
      await flush();
      if (f === "pdf") { setPrintAsk(true); return; }
      const p = f === "wav" ? await api.exportDenoised(id) : await api.exportAs(id, f);
      if (p) notify("ok", `書き出しました: ${p}`);
    } catch (e) { notify("err", `書き出せませんでした: ${String(e).replace(/^Error:\s*/, "")}`); }
  };

  const doFind = async (q: string) => {
    const hits = q.trim() ? await api.findIn(id, q) : [];
    setFind((f) => ({ ...f, q, hits, i: 0 }));
    if (hits.length) goTo(hits[0]);
  };
  const stepFind = (dir: 1 | -1) => {
    if (!find.hits.length) return;
    const i = (find.i + dir + find.hits.length) % find.hits.length;
    setFind({ ...find, i });
    goTo(find.hits[i]);
  };

  const lowPos = playing !== undefined ? lowIds.indexOf(playing) : -1;

  const ic = {
    clock: <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="12" cy="12" r="9" /><path d="M12 7v5l3 2" /></svg>,
    globe: <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="12" cy="12" r="9" /><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" /></svg>,
    file: <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M6 3h9l4 4v14H6z" /></svg>,
  };
  const views: [View, string, string?][] = [["read", "記録"], ["edit", "編集"], ["memo", "メモ", "議題・決定事項・ToDo"], ["summary", "要約"]];
  const busyHere = !!progress && progress.busy && progress.meetingId === id;
  const confirmWhy: string | null =
    m.recording ? "録音中です。録音を止めて、文字起こしが終わると確定できます"
    : m.state === "failed" ? `処理できませんでした: ${m.error ?? "理由は不明です"}。「やり直す」を押してください`
    : m.state === "processing" ? `文字起こし中のため、終わるまで確定できません${busyHere && progress!.totalChunks ? `(${progress!.doneChunks}/${progress!.totalChunks} 区間)` : ""}`
    : m.state === "queued" ? (progress?.busy ? "ほかの議事録を文字起こし中です。順番が来ると始まります。終わるまで確定できません" : "待ちの状態です。左の「再開」で文字起こしを始めてください")
    : diar ? "話者を判別している間は確定できません。終わるまでお待ちください(中断もできます)"
    : d.provisional ? "正確なモデルで文字起こし中です(仮の文字が残っています)。置き換わるまで確定できません"
    : null;
  // 処理が止まっているとき: 理由と次にすることを、スクロールしても見える所(ヘッダー)に出す
  const limitHit = /有料版/.test(m.error ?? "");
  const stall: { kind: "err" | "wait"; text: string; action: string; go: () => void; settings?: boolean } | null =
    m.state === "failed"
      ? {
        kind: "err",
        text: `文字起こしが止まりました: ${m.error ?? "理由は不明です"}。${limitHit ? "有料版にすると、続きから文字起こしできます。" : /モデル/.test(m.error ?? "") ? "設定でモデルを用意してから、やり直してください。" : "音声は残っています。やり直すと、最初から文字起こしします。"}`,
        action: limitHit ? "続きから文字起こし" : "やり直す", go: onRetry, settings: /モデル/.test(m.error ?? ""),
      }
      : m.state === "queued" && !m.recording && !jobsActive && progress && !progress.busy && progress.pending > 0
        ? {
          kind: "wait",
          text: jobError ? `文字起こしを始められませんでした: ${jobError}` : "文字起こしの順番待ちで止まっています(待ち " + progress.pending + " 件)。「再開」を押すと始まります。",
          action: "再開", go: onResume, settings: /モデル/.test(jobError ?? ""),
        }
        : null;
  const durMs = m.durationMs ?? (segs.length ? segs[segs.length - 1].endMs : 0);
  const readOnly = d.provisional || m.recording;
  const paras: { id: number; t: number; sp: string; tx: string; end: number }[] = [];
  for (const s of segs) {
    if (!s.text.trim()) continue;
    const last = paras[paras.length - 1];
    if (last && last.sp === s.speaker) { last.tx += s.text.trim(); last.end = s.endMs; } else paras.push({ id: s.id, t: s.startMs, sp: s.speaker, tx: s.text.trim(), end: s.endMs });
  }

  return (
    <div className="detail">
      <header className="d-head">
        <div className="d-top">
          <input className="d-title" aria-label="タイトル" value={meta.title} onChange={(e) => setMeta({ ...meta, title: e.target.value })} onBlur={saveMeta}
            onKeyDown={(e) => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} />
          <div className="d-actions">
            <span className={"save-ind " + (pending > 0 ? "saving" : saveRes && !saveRes.ok ? "err" : "")} aria-live="polite" data-testid="save-ind">
              {pending > 0 ? "保存中…" : saveRes?.ok ? `保存済み ${saveRes.at.getHours()}:${String(saveRes.at.getMinutes()).padStart(2, "0")}` : saveRes ? <>保存できませんでした({saveRes.why})<button className="link" onClick={saveRes.retry}>再試行</button></> : ""}
            </span>
            <span className={`badge s-${m.state}`}>{m.recording ? "録音中" : STATE_LABEL[m.state]}</span>
            <span className={"badge " + (m.status === "confirmed" ? "ok" : "muted")}>{m.status === "confirmed" ? "確定" : "下書き"}</span>
            {m.status === "draft"
              ? <button className="btn primary" disabled={!!confirmWhy} title={confirmWhy ?? "内容を確認できたら押してください"} aria-describedby={confirmWhy ? "confirm-why" : undefined} onClick={confirm}>確定</button>
              : <button className="btn" onClick={() => run(() => api.unconfirm(id), undefined, { toDraft: true })}>下書きに戻す</button>}
            <div className="exp" ref={expRef}>
              <button className="btn" aria-label="書き出し" aria-haspopup="menu" aria-expanded={expOpen} disabled={m.status !== "confirmed"}
                title={m.status !== "confirmed" ? "確定すると書き出せます" : ""} onClick={() => setExpOpen((o) => !o)}>⤓ 書き出し ▾</button>
              {expOpen && (
                <div className="menu exp-menu" role="menu" aria-label="書き出す形式">
                  {([["docx", "Word(.docx)"], ["pdf", "PDF(印刷から保存)"], ["md", "Markdown"], ["txt", "テキスト"], ["srt", "字幕(SRT)"], ...(m.hasAudio ? [["wav", "ノイズ除去後の音声(WAV)"]] : [])] as [string, string][]).map(([v, label]) => {
                    const ok = plan?.exports.includes(v) ?? true;
                    return <button key={v} role="menuitem" className="btn small" disabled={!ok} onClick={() => { setExpOpen(false); exp(v as ExportFormat | "wav" | "pdf"); }}>{label}{ok ? "" : "(有料版)"}</button>;
                  })}
                </div>
              )}
            </div>
          </div>
        </div>
        {m.status === "draft" && confirmWhy && <p className="d-why" id="confirm-why" data-testid="confirm-why">{confirmWhy}</p>}
        {stall && (
          <div className={"stall" + (stall.kind === "wait" ? " wait" : "")} role="alert" data-testid="stall">
            <span className="grow">{stall.text}</span>
            {stall.settings && <button className="btn small" onClick={onOpenSettings}>設定を開く</button>}
            <button className="btn small primary" onClick={stall.go}>{stall.action}</button>
          </div>
        )}
        <div className="meta-row">
          <span className="mi">{ic.clock}{durMs ? hms(durMs) : "--:--"}</span>
          <span className="mi">{ic.globe}{LANGUAGE_LABEL[m.language]}</span>
          <span className="mi">{ic.file}{m.sourceName}{m.denoise ? "・ノイズ除去" : ""}{m.rangeStartMs != null || m.rangeEndMs != null ? `・範囲 ${m.rangeStartMs != null ? hms(m.rangeStartMs) : "最初"}〜${m.rangeEndMs != null ? hms(m.rangeEndMs) : "最後"}` : ""}</span>
          <label>日付<input type="date" aria-label="日付" value={meta.heldOn} onChange={(e) => setMeta({ ...meta, heldOn: e.target.value })} onBlur={saveMeta} /></label>
          <label>参加者<input aria-label="参加者" value={meta.participants} placeholder="例: 佐藤、鈴木" onChange={(e) => setMeta({ ...meta, participants: e.target.value })} onBlur={saveMeta} /></label>
          <label>タグ<input aria-label="タグ" value={meta.tags} placeholder="例: 定例、案件A" onChange={(e) => setMeta({ ...meta, tags: e.target.value })} onBlur={saveMeta} /></label>
        </div>
        <div className="views" role="tablist" aria-label="表示">
          {views.map(([v, label, full]) => (
            <button key={v} role="tab" aria-selected={view === v} aria-label={full ? `${label}(${full})` : label} onClick={() => setView(v)}>
              {label}{v === "summary" && !plan?.summary && <span className="pro">有料版</span>}{v === "memo" && (m.agenda || m.decisions || m.todos.length) ? " ●" : ""}
            </button>
          ))}
        </div>
        {(view === "edit" || view === "read") && (
          <div className="toolbar">
            {(counts.size > 0 || (done && m.diarize)) && (
              <div className="speakers" data-testid="speakers">
                <span className="lbl">話者</span>
                {[...counts.entries()].map(([name, n]) => (
                  rename?.from === name ? (
                    <span key={name} className="rename">
                      <input autoFocus value={rename.to} aria-label={`${name} の新しい名前`} onChange={(e) => setRename({ ...rename, to: e.target.value })}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") { run(() => api.renameSpeaker(id, name, rename.to)); setRename(null); }
                          if (e.key === "Escape") setRename(null);
                        }} />
                      <button className="btn small" onClick={() => { run(() => api.renameSpeaker(id, name, rename.to)); setRename(null); }}>変更</button>
                    </span>
                  ) : (
                    <button key={name} className="spk" style={{ color: speakerColor(name) }} title="押すと名前を変えられます(この議事録の中をまとめて)"
                      onClick={() => setRename({ from: name, to: name.startsWith("話者") ? "" : name })}>
                      <i style={{ background: speakerColor(name) }} /><span style={{ color: "var(--ink)" }}>{name}</span><small>{n}</small>
                    </button>
                  )
                ))}
                {diar ? (
                  <span className="diar-run" role="status" data-testid="diar-run">
                    <span>話者を判別しています…{diar.total > 0 ? ` ${diar.done}/${diar.total}` : "(音声を読み込み中)"}</span>
                    <progress max={diar.total || 1} value={diar.total ? diar.done : undefined} aria-label="話者を判別している進み具合" />
                    <button className="btn small" disabled={diar.cancelling} onClick={() => { setDiar({ ...diar, cancelling: true }); api.cancelRediarize().catch(() => undefined); }}>{diar.cancelling ? "中断しています…" : "中断"}</button>
                  </span>
                ) : done && (settings?.diarizeAvailable ?? true) && (plan?.diarize ?? true) && (
                  <button className="btn small" onClick={() => setDiarDlg({ n: m.numSpeakers })}>話者を判別し直す…</button>
                )}
              </div>
            )}
            <span className="grow" />
            <button className="btn small" disabled={!d.canUndo} onClick={() => run(() => api.undo(id))} title="元に戻す">↶ 元に戻す</button>
            <button className="btn small" onClick={() => setFind((f) => ({ ...f, open: !f.open }))}>検索・置換</button>
            <button className="btn small" disabled={!done || !(plan?.glossary ?? true)} title={plan?.glossary ?? true ? "" : "有料版の機能です"} onClick={async () => { try { const [n, x] = await api.reapplyGlossary(id); apply(x); notify("ok", `用語辞書で ${n} 件を置き換えました(手で直した文は変えません)`); onChanged(); } catch (e) { notify("err", `用語辞書を適用できませんでした: ${e}`); } }}>用語辞書を適用</button>
            <details className="more">
              <summary className="btn small icon" aria-label="その他">⋯</summary>
              <div className="menu">
                <button className="btn small" disabled={!m.hasAudio || m.state === "processing"} title={m.hasAudio ? "" : "音声を残していないため、やり直せません"}
                  onClick={() => setRedo({ denoise: m.denoise, diarize: m.diarize, numSpeakers: m.numSpeakers, language: m.language, rangeStartMs: m.rangeStartMs, rangeEndMs: m.rangeEndMs })}>設定を変えてやり直す</button>
                {!delAsk ? <button className="btn small ghost" onClick={() => setDelAsk(true)}>削除</button> : (
                  <span className="ask">音声と文字をまとめて消します。
                    <button className="btn small danger" onClick={async () => { try { await flush(); await api.deleteMeeting(id); onDeleted(); } catch (e) { notify("err", `削除できませんでした: ${e}`); setDelAsk(false); } }}>消す</button>
                    <button className="btn small" onClick={() => setDelAsk(false)}>やめる</button>
                  </span>
                )}
              </div>
            </details>
          </div>
        )}
      </header>

      {diarDlg && (
        <div className="modal" role="dialog" aria-label="話者を判別し直す" onKeyDown={(e) => { if (e.key === "Escape") setDiarDlg(null); }}>
          <div className="modal-body">
            <h2>話者を判別し直す</h2>
            <label className="field">話者の人数
              <select aria-label="話者の人数" value={diarDlg.n ?? ""} onChange={(e) => setDiarDlg({ n: e.target.value ? Number(e.target.value) : null })}>
                <option value="">自動</option>
                {[2, 3, 4, 5, 6, 7, 8].map((n) => <option key={n} value={n}>{n}人</option>)}
              </select>
            </label>
            <p className="note">声の特徴から話者を分け直します。録音の長さによって数十秒〜数分かかります。名前を付けた話者も付け直します(「元に戻す」で戻せます)。処理中も他の操作はできます。</p>
            <div className="row">
              <button className="btn primary" autoFocus onClick={() => startRediarize(diarDlg.n)}>実行</button>
              <button className="btn" onClick={() => setDiarDlg(null)}>やめる</button>
            </div>
          </div>
        </div>
      )}
      {redo && (
        <div className="modal" role="dialog" aria-label="設定を変えてやり直す">
          <div className="modal-body">
            <h2>設定を変えて、文字起こしをやり直す</h2>
            <OptionsForm value={redo} onChange={setRedo} diarizeAvailable={settings?.diarizeAvailable ?? true} plan={plan} />
            <p className="msg err">これまでの修正(文・話者・結合・分割)は消えます。タイトル・日付・参加者・議題などは残ります。</p>
            <div className="row">
              <button className="btn danger" disabled={!optionsValid(redo)} onClick={async () => {
                try { await flush(); await api.reprocess(id, redo); setRedo(null); notify("ok", "やり直しを始めました"); onReprocessed(); api.detail(id).then(apply); }
                catch (e) { notify("err", `やり直せませんでした: ${e}`); }
              }}>やり直す</button>
              <button className="btn" onClick={() => setRedo(null)}>やめる</button>
            </div>
          </div>
        </div>
      )}

      <div className="d-body">
        <div className="d-inner">
          {readOnly && (
            <p className="banner" data-testid="provisional">
              {m.recording ? "録音中です。" : "正確なモデルで文字起こししています。"}
              いま表示しているのは録音中の仮の文字(精度は低め)で、正確な文字起こしができた所から順に置き換わります。置き換わるまで編集できません。
            </p>
          )}
          {m.status !== "confirmed" && done && !d.provisional && view === "edit" && <p className="note">内容を確認して「確定」すると書き出せます。</p>}
          {plan?.tier === "free" && done && <p className="note">無料版は小さなモデルで文字起こししています。有料版では、より正確なモデルで文字起こしし直せます。</p>}

          {find.open && (
            <div className="findbar card" data-testid="findbar">
              <input id="find-q" placeholder="この議事録の中を検索" value={find.q} onChange={(e) => doFind(e.target.value)}
                onKeyDown={(e) => { if (e.key === "Enter") stepFind(e.shiftKey ? -1 : 1); if (e.key === "Escape") setFind({ ...find, open: false }); }} />
              <span className="note">{find.q.trim() ? (find.hits.length ? `${find.i + 1} / ${find.hits.length} 件` : "見つかりません") : ""}</span>
              <button className="btn small" disabled={!find.hits.length} onClick={() => stepFind(-1)}>前へ</button>
              <button className="btn small" disabled={!find.hits.length} onClick={() => stepFind(1)}>次へ</button>
              <input placeholder="置き換える語" value={find.r} onChange={(e) => setFind({ ...find, r: e.target.value })} />
              <button className="btn small" disabled={!find.q} onClick={async () => {
                try { await flush(); const [n, x] = await api.replaceIn(id, find.q, find.r); apply(x); notify("ok", `${n} 件の文で置き換えました(「元に戻す」で戻せます)`); setFind({ ...find, hits: [], i: 0 }); onChanged(); }
                catch (e) { notify("err", `置き換えられませんでした: ${e}`); }
              }}>すべて置換</button>
              <button className="link" onClick={() => setFind({ ...find, open: false, hits: [] })}>閉じる</button>
            </div>
          )}

          {view === "edit" && (
            <>
              <ol className="segs">
                {segs.map((s, i) => (
                  <Row key={`${s.id}-${s.text}-${s.speaker}`} s={s} last={i + 1 >= segs.length} playing={s.id === playing} hit={hitSet.has(s.id) && find.hits[find.i] === s.id}
                    low={s.confidence < d.lowConfidence} speakers={d.speakers} actions={actions} readOnly={readOnly} />
                ))}
              </ol>
              {!segs.length && <p className="empty">{done ? "文字にできる音声がありませんでした。" : "文字起こしの結果はここに表示されます。"}</p>}
              <p className="note keys">Space 再生/停止 ・ ←→ 5秒 ・ ↑↓ 前後の文 ・ n / p 次/前の要確認 ・ ⌘(Ctrl)+Enter 入力中でも再生/停止 ・ ⌘(Ctrl)+F 検索</p>
            </>
          )}
          {view === "read" && (
            <article className="reading" data-testid="reading">
              {paras.map((p) => (
                <div key={p.id} className={"p" + (now >= p.t && now < p.end ? " playing" : "")} onClick={() => seek(p.t, false)}>
                  <div className="who"><span><i style={{ background: p.sp ? speakerColor(p.sp) : "var(--line-strong)" }} />{p.sp || "—"}</span><small>{hms(p.t)}</small></div>
                  <div>{p.tx}</div>
                </div>
              ))}
              {!paras.length && <p className="empty">まだ文字がありません。</p>}
            </article>
          )}
          {view === "memo" && <div className="card"><Notes d={d} onSave={(a, dc, t) => run(() => api.updateNotes(id, a, dc, t))} /></div>}
          {/* 要約は、タブを切り替えても下書き(確認前)が消えないよう、表示を隠すだけにする */}
          <div hidden={view !== "summary"}>
            <SummaryPanel api={api} d={d} plan={plan} onOpenSettings={() => onOpenSettings?.()}
              onImport={async (n, count) => { await flush(); const x = await run(() => api.updateNotes(id, n.agenda, n.decisions, n.todos), `${count} 件をメモに追加しました(「メモ」で確認・修正できます)`); if (!x) throw new Error("メモに追加できませんでした"); }} />
          </div>
        </div>
      </div>

      <div className="player-bar" data-testid="player">
        {audio && <audio ref={player} src={audio} onTimeUpdate={(e) => setNow(e.currentTarget.currentTime * 1000)} onPlay={() => setPaused(false)} onPause={() => setPaused(true)} />}
        <div className="transport">
          <button disabled={!audio} aria-label="前の文" onClick={() => { const i = Math.max(0, (playingIdx >= 0 ? playingIdx : 0) - 1); if (segs[i]) goTo(segs[i].id); }}>
            <svg viewBox="0 0 24 24" fill="currentColor"><path d="M6 5h2v14H6zM20 5v14L9 12z" /></svg></button>
          <button className="play" disabled={!audio} aria-label={paused ? "再生" : "一時停止"} onClick={() => { const a = player.current; if (a) (a.paused ? a.play().catch(() => undefined) : a.pause()); }}>
            {paused ? <svg viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z" /></svg> : <svg viewBox="0 0 24 24" fill="currentColor"><path d="M7 5h4v14H7zM13 5h4v14h-4z" /></svg>}</button>
          <button disabled={!audio} aria-label="次の文" onClick={() => { const i = Math.min(segs.length - 1, (playingIdx >= 0 ? playingIdx : -1) + 1); if (segs[i]) goTo(segs[i].id); }}>
            <svg viewBox="0 0 24 24" fill="currentColor"><path d="M16 5h2v14h-2zM4 5v14l11-7z" /></svg></button>
        </div>
        <span className="clock">{hms(now)}</span>
        {audio ? <Wave peaks={peaks} durationMs={durMs} nowMs={now} segs={segs} onSeek={(ms) => seek(ms, false)} /> : <span className="note">{m.hasAudio ? "音声を読み込んでいます…" : "音声を残さない設定のため、再生できません。"}</span>}
        <span className="clock end">{hms(durMs)}</span>
        <div className="player-opts">
          <select value={speed} onChange={(e) => setSpeed(Number(e.target.value))} aria-label="再生速度">
            {SPEEDS.map((v) => <option key={v} value={v}>{v}x</option>)}
          </select>
          <button className="toggle" aria-pressed={follow} onClick={() => setFollow(!follow)} title="再生に合わせて表示を追う">追従</button>
          <button className="toggle" aria-pressed={autoPause} onClick={() => setAutoPause(!autoPause)} title="文の入力中は一時停止し、終えると少し前から再開">入力で停止</button>
          <span className="lownav note" data-testid="low-nav" title="自信の低い文(要確認)を順に">要確認 {lowIds.length}{lowPos >= 0 ? `(${lowPos + 1})` : ""}
            <button disabled={!lowIds.length} aria-label="前の要確認" onClick={() => nextLow(-1)}>‹</button>
            <button disabled={!lowIds.length} aria-label="次の要確認" onClick={() => nextLow(1)}>›</button>
          </span>
        </div>
      </div>
      <datalist id="speakers">{d.speakers.map((s) => <option key={s} value={s} />)}</datalist>
      {printAsk && (
        <PrintDialog value={printOpts} onChange={setPrintOpts} onClose={() => setPrintAsk(false)}
          onPrint={async () => { setPrintAsk(false); try { await api.printPage(); notify("ok", "印刷の画面で「PDF として保存」を選ぶと、PDF にできます"); } catch (e) { notify("err", `印刷の画面を開けませんでした: ${e}`); } }} />
      )}
      <PrintDoc d={d} o={printOpts} />
    </div>
  );
}
