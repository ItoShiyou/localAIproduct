import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Api } from "./api";
import { OptionsForm, optionsValid } from "./OptionsForm";
import type { Detail, ExportFormat, ProcessOptions, Segment, SettingsInfo, Todo } from "./types";
import { LANGUAGE_LABEL, STATE_LABEL, hms, speakerColor } from "./types";

const SPEEDS = [0.75, 1, 1.25, 1.5, 2];

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
const Row = memo(function Row({ s, last, playing, hit, low, speakers, actions }: {
  s: Segment; last: boolean; playing: boolean; hit: boolean; low: boolean; speakers: string[]; actions: React.MutableRefObject<RowActions>;
}) {
  const ta = useRef<HTMLTextAreaElement>(null);
  return (
    <li id={`seg-${s.id}`} className={"seg" + (playing ? " playing" : "") + (low ? " low" : "") + (hit ? " hit" : "")} data-testid="segment"
      style={{ borderLeftColor: speakerColor(s.speaker) }}
      onClick={(e) => {
        // 文のどこを押しても、その位置へ再生位置を移す(入力欄・ボタンを押したときは除く)
        const t = e.target as HTMLElement;
        if (!t.closest("button, input, select")) actions.current.seek(s.startMs, false);
      }}>
      <div className="seg-head">
        <button className="time" onClick={() => actions.current.seek(s.startMs, true)} title="ここから再生">▶ {hms(s.startMs)}</button>
        <input className="speaker" list="speakers" defaultValue={s.speaker} placeholder="話者" aria-label="話者" style={{ color: speakerColor(s.speaker) }}
          onBlur={(e) => { if (e.target.value !== s.speaker) actions.current.setSpeaker(s.id, e.target.value); }}
          onKeyDown={(e) => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} />
        {speakers.filter((x) => x !== s.speaker).slice(0, 5).map((x) => (
          <button key={x} className="chip" style={{ borderColor: speakerColor(x), color: speakerColor(x) }} onClick={() => actions.current.setSpeaker(s.id, x)}>{x}</button>
        ))}
        {low && <span className="badge">要確認</span>}
        {s.edited && <span className="badge muted">修正済み</span>}
        <span className="grow" />
        <button className="link" title="カーソルの位置で2つに分けます" onClick={() => {
          const at = ta.current ? [...ta.current.value.slice(0, ta.current.selectionStart)].length : 0;
          actions.current.split(s.id, at);
        }}>ここで分割</button>
        {!last && <button className="link" onClick={() => actions.current.mergeNext(s.id)}>次と結合</button>}
        {s.edited && s.rawText && <button className="link" onClick={() => actions.current.revert(s.id)}>文字起こしの結果に戻す</button>}
      </div>
      <textarea ref={ta} defaultValue={s.text} rows={Math.max(1, Math.ceil(s.text.length / 60))} aria-label={`${hms(s.startMs)} の文`}
        onInput={() => actions.current.typing()}
        onKeyDown={(e) => { if (e.key === "Escape") (e.target as HTMLTextAreaElement).blur(); }}
        onBlur={(e) => { actions.current.doneTyping(); if (e.target.value !== s.text) actions.current.editText(s.id, e.target.value); }} />
    </li>
  );
});

/** 議題・決定事項・ToDo(手で書く定型欄) */
function Notes({ d, onSave }: { d: Detail; onSave: (agenda: string, decisions: string, todos: Todo[]) => void }) {
  const m = d.meeting;
  const [agenda, setAgenda] = useState(m.agenda);
  const [decisions, setDecisions] = useState(m.decisions);
  const [todos, setTodos] = useState<Todo[]>(m.todos);
  useEffect(() => { setAgenda(m.agenda); setDecisions(m.decisions); setTodos(m.todos); }, [m.agenda, m.decisions, m.todos]);
  const save = (t = todos) => {
    if (agenda === m.agenda && decisions === m.decisions && JSON.stringify(t) === JSON.stringify(m.todos)) return;
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

/** 印刷用(画面では隠し、印刷のときだけ出す。PDF はここから作る) */
function PrintDoc({ d }: { d: Detail }) {
  const m = d.meeting;
  const paras: { t: number; sp: string; tx: string }[] = [];
  for (const s of d.segments) {
    if (!s.text.trim()) continue;
    const last = paras[paras.length - 1];
    if (last && last.sp === s.speaker) last.tx += s.text.trim();
    else paras.push({ t: s.startMs, sp: s.speaker, tx: s.text.trim() });
  }
  const lines = (x: string) => x.split("\n").map((l) => l.trim()).filter(Boolean);
  return (
    <article className="print-doc" aria-hidden="true">
      <h1>{m.title}</h1>
      {m.heldOn && <p>日付: {m.heldOn}</p>}
      {m.participantsText.trim() && <p>参加者: {m.participantsText}</p>}
      {m.agenda.trim() && <><h2>議題</h2><ul>{lines(m.agenda).map((l, i) => <li key={i}>{l}</li>)}</ul></>}
      {m.decisions.trim() && <><h2>決定事項</h2><ul>{lines(m.decisions).map((l, i) => <li key={i}>{l}</li>)}</ul></>}
      {m.todos.length > 0 && <><h2>ToDo</h2><ul>{m.todos.map((t, i) => <li key={i}>{t.done ? "[済] " : ""}{t.text}{t.owner && `(担当: ${t.owner})`}{t.due && `(期限: ${t.due})`}</li>)}</ul></>}
      <h2>内容</h2>
      {paras.map((p, i) => <p key={i}>{p.sp && <b>{p.sp} </b>}<span className="ts">[{hms(p.t)}]</span> {p.tx}</p>)}
    </article>
  );
}

/** 1つの議事録の確認画面 */
export function DetailView({ api, id, version, seekTo, settings, onChanged, onDeleted, onRetry, onReprocessed }: {
  api: Api; id: number; version: number; seekTo: { ms: number; n: number } | null; settings: SettingsInfo | null;
  onChanged: () => void; onDeleted: () => void; onRetry: () => void; onReprocessed: () => void;
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
  const [showNotes, setShowNotes] = useState(false);
  const [find, setFind] = useState<{ open: boolean; q: string; r: string; hits: number[]; i: number }>({ open: false, q: "", r: "", hits: [], i: 0 });
  const [rename, setRename] = useState<{ from: string; to: string } | null>(null);
  const [redo, setRedo] = useState<ProcessOptions | null>(null);
  const player = useRef<HTMLAudioElement>(null);
  const pausedByTyping = useRef(false);
  const lastPlaying = useRef<number | undefined>(undefined);

  const apply = useCallback((x: Detail) => {
    setD(x);
    setMeta({ title: x.meeting.title, heldOn: x.meeting.heldOn ?? "", participants: x.meeting.participantsText, tags: x.meeting.tags.join("、") });
  }, []);

  useEffect(() => {
    api.detail(id).then(apply).catch((e) => setMsg(String(e)));
    api.audioUrl(id).then(setAudio).catch(() => setAudio(null));
  }, [api, id, version, apply]);

  // 処理中は少しずつ結果が増えるので、読み直す
  useEffect(() => {
    if (!d || (d.meeting.state !== "processing" && d.meeting.state !== "queued")) return;
    const t = setInterval(() => api.detail(id).then(apply).catch(() => undefined), 1500);
    return () => clearInterval(t);
  }, [api, id, d, apply]);

  const seek = useCallback((ms: number, play = true) => {
    setNow(ms);
    const a = player.current;
    if (!a) return;
    a.currentTime = ms / 1000;
    if (play) a.play().catch(() => undefined);
  }, []);
  useEffect(() => { if (seekTo && d) { seek(seekTo.ms, false); document.getElementById(`seg-${d.segments.find((s) => s.endMs > seekTo.ms)?.id}`)?.scrollIntoView({ block: "center" }); } }, [seekTo, d, seek]);
  useEffect(() => { if (player.current) player.current.playbackRate = speed; }, [speed, audio]);

  const run = useCallback(async (f: () => Promise<Detail>, after?: string) => {
    try { apply(await f()); setMsg(after ?? null); onChanged(); }
    catch (e) { setMsg(String(e)); }
  }, [apply, onChanged]);

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

  const saveMeta = () => {
    const tags = meta.tags.split(/[、,，\s]+/).map((t) => t.trim()).filter(Boolean);
    if (tags.join("\u0000") !== m.tags.join("\u0000")) run(() => api.setTags(id, tags));
    if (meta.title === m.title && meta.heldOn === (m.heldOn ?? "") && meta.participants === m.participantsText) return;
    run(() => api.updateMeta(id, meta.title, meta.heldOn || null, meta.participants));
  };

  const exp = async (f: ExportFormat | "wav" | "pdf") => {
    try {
      if (f === "pdf") { await api.printPage(); setMsg("印刷の画面で「PDF として保存」を選ぶと、PDF にできます"); return; }
      const p = f === "wav" ? await api.exportDenoised(id) : await api.exportAs(id, f);
      if (p) setMsg(`書き出しました: ${p}`);
    } catch (e) { setMsg(String(e)); }
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

  return (
    <div className="detail">
      <div className="meta card">
        <label>タイトル<input value={meta.title} onChange={(e) => setMeta({ ...meta, title: e.target.value })} onBlur={saveMeta} /></label>
        <label>日付<input type="date" value={meta.heldOn} onChange={(e) => setMeta({ ...meta, heldOn: e.target.value })} onBlur={saveMeta} /></label>
        <label className="grow">参加者<input value={meta.participants} placeholder="例: 佐藤、鈴木" onChange={(e) => setMeta({ ...meta, participants: e.target.value })} onBlur={saveMeta} /></label>
        <label className="grow">タグ<input value={meta.tags} placeholder="例: 定例、案件A(読点で区切る)" onChange={(e) => setMeta({ ...meta, tags: e.target.value })} onBlur={saveMeta} /></label>
      </div>

      <div className="card">
        <button className="link" onClick={() => setShowNotes((v) => !v)} aria-expanded={showNotes}>議題・決定事項・ToDo {showNotes ? "▲" : "▼"}{!showNotes && (m.agenda || m.decisions || m.todos.length) ? "(記入あり)" : ""}</button>
        {showNotes && <Notes d={d} onSave={(a, dc, t) => run(() => api.updateNotes(id, a, dc, t))} />}
      </div>

      <div className="toolbar card">
        <span className={`badge s-${m.state}`}>{STATE_LABEL[m.state]}</span>
        <span className={"badge " + (m.status === "confirmed" ? "ok" : "muted")}>{m.status === "confirmed" ? "確定" : "下書き"}</span>
        <span className="note">{m.sourceName}・{LANGUAGE_LABEL[m.language]}{m.denoise ? "・ノイズ除去" : ""}{m.rangeStartMs != null || m.rangeEndMs != null ? `・範囲 ${m.rangeStartMs != null ? hms(m.rangeStartMs) : "最初"}〜${m.rangeEndMs != null ? hms(m.rangeEndMs) : "最後"}` : ""}</span>
        <span className="grow" />
        <button className="btn small" disabled={!d.canUndo} onClick={() => run(() => api.undo(id))}>元に戻す</button>
        <button className="btn small" onClick={() => setFind((f) => ({ ...f, open: !f.open }))}>検索・置換</button>
        <button className="btn small" disabled={!done} onClick={async () => { try { const [n, x] = await api.reapplyGlossary(id); apply(x); setMsg(`用語辞書で ${n} 件を置き換えました(手で直した文は変えません)`); } catch (e) { setMsg(String(e)); } }}>用語辞書を適用</button>
        <button className="btn small" disabled={!m.hasAudio || m.state === "processing"} title={m.hasAudio ? "" : "音声を残していないため、やり直せません"}
          onClick={() => setRedo({ denoise: m.denoise, diarize: m.diarize, numSpeakers: m.numSpeakers, language: m.language, rangeStartMs: m.rangeStartMs, rangeEndMs: m.rangeEndMs })}>設定を変えてやり直す</button>
        {m.status === "draft"
          ? <button className="btn small primary" disabled={!done} onClick={() => run(() => api.confirm(id), "確定しました。書き出せます")}>確定</button>
          : <button className="btn small" onClick={() => run(() => api.unconfirm(id))}>下書きに戻す</button>}
        <select aria-label="書き出し" value="" disabled={m.status !== "confirmed"} onChange={(e) => { if (e.target.value) exp(e.target.value as ExportFormat | "wav" | "pdf"); }}>
          <option value="">書き出し…</option>
          <option value="docx">Word(.docx)</option>
          <option value="pdf">PDF(印刷から保存)</option>
          <option value="md">Markdown</option>
          <option value="txt">テキスト</option>
          <option value="srt">字幕(SRT)</option>
          {m.hasAudio && <option value="wav">ノイズ除去後の音声(WAV)</option>}
        </select>
        {!delAsk ? <button className="btn small ghost" onClick={() => setDelAsk(true)}>削除</button> : (
          <span className="ask">音声と文字をまとめて消します。
            <button className="btn small danger" onClick={async () => { try { await api.deleteMeeting(id); onDeleted(); } catch (e) { setMsg(String(e)); setDelAsk(false); } }}>消す</button>
            <button className="btn small" onClick={() => setDelAsk(false)}>やめる</button>
          </span>
        )}
      </div>

      {redo && (
        <div className="modal" role="dialog" aria-label="設定を変えてやり直す">
          <div className="modal-body">
            <h2>設定を変えて、文字起こしをやり直す</h2>
            <OptionsForm value={redo} onChange={setRedo} diarizeAvailable={settings?.diarizeAvailable ?? true} />
            <p className="msg err">これまでの修正(文・話者・結合・分割)は消えます。タイトル・日付・参加者・議題などは残ります。</p>
            <div className="row">
              <button className="btn danger" disabled={!optionsValid(redo)} onClick={async () => {
                try { await api.reprocess(id, redo); setRedo(null); setMsg("やり直しを始めました"); onReprocessed(); api.detail(id).then(apply); }
                catch (e) { setMsg(String(e)); }
              }}>やり直す</button>
              <button className="btn" onClick={() => setRedo(null)}>やめる</button>
            </div>
          </div>
        </div>
      )}

      {m.status !== "confirmed" && done && <p className="note">内容を確認して「確定」すると書き出せます。</p>}
      {msg && <p className="msg">{msg}</p>}
      {m.state === "failed" && <p className="msg err">処理できませんでした: {m.error} <button className="btn small" onClick={onRetry}>やり直す</button></p>}

      {find.open && (
        <div className="findbar card" data-testid="findbar">
          <input id="find-q" placeholder="この議事録の中を検索" value={find.q} onChange={(e) => doFind(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") stepFind(e.shiftKey ? -1 : 1); if (e.key === "Escape") setFind({ ...find, open: false }); }} />
          <span className="note">{find.q.trim() ? (find.hits.length ? `${find.i + 1} / ${find.hits.length} 件` : "見つかりません") : ""}</span>
          <button className="btn small" disabled={!find.hits.length} onClick={() => stepFind(-1)}>前へ</button>
          <button className="btn small" disabled={!find.hits.length} onClick={() => stepFind(1)}>次へ</button>
          <input placeholder="置き換える語" value={find.r} onChange={(e) => setFind({ ...find, r: e.target.value })} />
          <button className="btn small" disabled={!find.q} onClick={async () => {
            try { const [n, x] = await api.replaceIn(id, find.q, find.r); apply(x); setMsg(`${n} 件の文で置き換えました(「元に戻す」で戻せます)`); setFind({ ...find, hits: [], i: 0 }); onChanged(); }
            catch (e) { setMsg(String(e)); }
          }}>すべて置換</button>
          <button className="link" onClick={() => setFind({ ...find, open: false, hits: [] })}>閉じる</button>
        </div>
      )}

      <div className="playerbar card">
        {audio ? (
          <audio ref={player} controls src={audio} className="player" onTimeUpdate={(e) => setNow(e.currentTarget.currentTime * 1000)} />
        ) : (
          <p className="note">{m.hasAudio ? "" : "音声を残さない設定のため、再生できません。"}</p>
        )}
        <div className="row">
          <button className="btn small" disabled={!audio} onClick={() => { const a = player.current; if (a) a.currentTime = Math.max(0, a.currentTime - 5); }}>−5秒</button>
          <button className="btn small" disabled={!audio} onClick={() => { const a = player.current; if (a) a.currentTime += 5; }}>＋5秒</button>
          <label>速度
            <select value={speed} onChange={(e) => setSpeed(Number(e.target.value))} aria-label="再生速度">
              {SPEEDS.map((v) => <option key={v} value={v}>{v}倍</option>)}
            </select>
          </label>
          <label className="check"><input type="checkbox" checked={follow} onChange={(e) => setFollow(e.target.checked)} /> 再生に合わせて表示を追う</label>
          <label className="check"><input type="checkbox" checked={autoPause} onChange={(e) => setAutoPause(e.target.checked)} /> 入力中は一時停止</label>
          <span className="grow" />
          <span className="note" data-testid="low-nav">要確認 {lowIds.length} 件{lowPos >= 0 ? `(${lowPos + 1} 件目)` : ""}</span>
          <button className="btn small" disabled={!lowIds.length} onClick={() => nextLow(-1)}>前の要確認</button>
          <button className="btn small" disabled={!lowIds.length} onClick={() => nextLow(1)}>次の要確認</button>
        </div>
        <p className="note keys">キー: Space 再生/停止・←→ 5秒・↑↓ 前後の文・n / p 次/前の要確認・⌘(Ctrl)+Enter 入力中でも再生/停止・⌘(Ctrl)+F 検索。文を押すとその位置へ移ります。</p>
      </div>

      {(counts.size > 0 || (done && m.diarize)) && (
        <div className="speakers card" data-testid="speakers">
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
              <button key={name} className="spk" style={{ borderColor: speakerColor(name) }} title="押すと名前を変えられます(この議事録の中をまとめて)"
                onClick={() => setRename({ from: name, to: name.startsWith("話者") ? "" : name })}>
                <i style={{ background: speakerColor(name) }} />{name}<small>{n}</small>
              </button>
            )
          ))}
          <span className="grow" />
          {done && (settings?.diarizeAvailable ?? true) && (
            <label>判別し直す
              <select value="" aria-label="話者を判別し直す" onChange={(e) => {
                if (!e.target.value) return;
                const n = e.target.value === "auto" ? null : Number(e.target.value);
                run(() => api.rediarize(id, n), "話者を判別し直しました(「元に戻す」で戻せます)");
              }}>
                <option value="">人数を選ぶ…</option>
                <option value="auto">自動</option>
                {[2, 3, 4, 5, 6, 7, 8].map((n) => <option key={n} value={n}>{n}人</option>)}
              </select>
            </label>
          )}
        </div>
      )}
      <datalist id="speakers">{d.speakers.map((s) => <option key={s} value={s} />)}</datalist>

      <ol className="segs">
        {segs.map((s, i) => (
          <Row key={`${s.id}-${s.text}-${s.speaker}`} s={s} last={i + 1 >= segs.length} playing={s.id === playing} hit={hitSet.has(s.id) && find.hits[find.i] === s.id}
            low={s.confidence < d.lowConfidence} speakers={d.speakers} actions={actions} />
        ))}
      </ol>
      {!segs.length && <p className="empty">{done ? "文字にできる音声がありませんでした。" : "文字起こしの結果はここに表示されます。"}</p>}
      <PrintDoc d={d} />
    </div>
  );
}
