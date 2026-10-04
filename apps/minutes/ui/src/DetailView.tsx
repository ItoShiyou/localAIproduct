import { useCallback, useEffect, useRef, useState } from "react";
import type { Api } from "./api";
import type { Detail, ExportFormat, Segment } from "./types";
import { STATE_LABEL, hms } from "./types";

/** 1つの議事録の確認画面: メタ情報、再生、文の修正・結合・分割・話者、元に戻す、確定、書き出し */
export function DetailView({ api, id, version, seekTo, onChanged, onDeleted, onRetry }: {
  api: Api; id: number; version: number; seekTo: { ms: number; n: number } | null;
  onChanged: () => void; onDeleted: () => void; onRetry: () => void;
}) {
  const [d, setD] = useState<Detail | null>(null);
  const [audio, setAudio] = useState<string | null>(null);
  const [now, setNow] = useState(0);
  const [msg, setMsg] = useState<string | null>(null);
  const [meta, setMeta] = useState({ title: "", heldOn: "", participants: "" });
  const [delAsk, setDelAsk] = useState(false);
  const [follow, setFollow] = useState(true);
  const player = useRef<HTMLAudioElement>(null);
  const rows = useRef<Map<number, HTMLTextAreaElement>>(new Map());

  const apply = useCallback((x: Detail) => {
    setD(x);
    setMeta({ title: x.meeting.title, heldOn: x.meeting.heldOn ?? "", participants: x.meeting.participantsText });
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
    const a = player.current;
    if (!a) return;
    a.currentTime = ms / 1000;
    if (play) a.play().catch(() => undefined);
  }, []);
  useEffect(() => { if (seekTo && d) { seek(seekTo.ms, false); setNow(seekTo.ms); } }, [seekTo, d, seek]);

  const run = async (f: () => Promise<Detail>, after?: string) => {
    try { apply(await f()); setMsg(after ?? null); onChanged(); }
    catch (e) { setMsg(String(e)); }
  };

  if (!d) return <div className="empty">{msg ?? "読み込み中…"}</div>;
  const m = d.meeting;
  const playing = d.segments.find((s) => now >= s.startMs && now < s.endMs)?.id;
  const done = m.state === "done";

  const saveMeta = () => {
    if (meta.title === m.title && meta.heldOn === (m.heldOn ?? "") && meta.participants === m.participantsText) return;
    run(() => api.updateMeta(id, meta.title, meta.heldOn || null, meta.participants));
  };

  const exp = async (f: ExportFormat | "wav") => {
    try {
      const p = f === "wav" ? await api.exportDenoised(id) : await api.exportAs(id, f);
      if (p) setMsg(`書き出しました: ${p}`);
    } catch (e) { setMsg(String(e)); }
  };

  // コンポーネントにせず関数で描く(再描画のたびに作り直されて、入力中のフォーカスが外れないように)
  const row = (s: Segment, i: number) => {
    const low = s.confidence < d.lowConfidence;
    return (
      <li key={`${s.id}-${s.text}-${s.speaker}`} className={"seg" + (s.id === playing ? " playing" : "") + (low ? " low" : "")} data-testid="segment">
        <div className="seg-head">
          <button className="time" onClick={() => seek(s.startMs)} title="ここから再生" disabled={!audio}>{hms(s.startMs)}</button>
          <input className="speaker" list="speakers" defaultValue={s.speaker} placeholder="話者" aria-label="話者"
            onBlur={(e) => { if (e.target.value !== s.speaker) run(() => api.setSpeaker(s.id, e.target.value, follow)); }}
            onKeyDown={(e) => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} />
          {d.speakers.filter((x) => x !== s.speaker).slice(0, 4).map((x) => (
            <button key={x} className="chip" onClick={() => run(() => api.setSpeaker(s.id, x, follow))}>{x}</button>
          ))}
          {low && <span className="badge">要確認</span>}
          {s.edited && <span className="badge muted">修正済み</span>}
          <span className="grow" />
          <button className="link" title="カーソルの位置で2つに分けます" onClick={() => {
            const ta = rows.current.get(s.id);
            const at = ta ? [...ta.value.slice(0, ta.selectionStart)].length : 0;
            run(() => api.split(s.id, at));
          }}>ここで分割</button>
          {i + 1 < d.segments.length && <button className="link" onClick={() => run(() => api.mergeNext(s.id))}>次と結合</button>}
          {s.edited && s.rawText && <button className="link" onClick={() => run(() => api.revertSegment(s.id))}>文字起こしの結果に戻す</button>}
        </div>
        <textarea ref={(el) => { if (el) rows.current.set(s.id, el); else rows.current.delete(s.id); }}
          defaultValue={s.text} rows={Math.max(1, Math.ceil(s.text.length / 60))} aria-label={`${hms(s.startMs)} の文`}
          onFocus={() => setNow(s.startMs)}
          onBlur={(e) => { if (e.target.value !== s.text) run(() => api.editText(s.id, e.target.value)); }} />
      </li>
    );
  };

  return (
    <div className="detail">
      <div className="meta card">
        <label>タイトル<input value={meta.title} onChange={(e) => setMeta({ ...meta, title: e.target.value })} onBlur={saveMeta} /></label>
        <label>日付<input type="date" value={meta.heldOn} onChange={(e) => setMeta({ ...meta, heldOn: e.target.value })} onBlur={saveMeta} /></label>
        <label className="grow">参加者<input value={meta.participants} placeholder="例: 佐藤、鈴木" onChange={(e) => setMeta({ ...meta, participants: e.target.value })} onBlur={saveMeta} /></label>
      </div>

      <div className="toolbar card">
        <span className={`badge s-${m.state}`}>{STATE_LABEL[m.state]}</span>
        <span className={"badge " + (m.status === "confirmed" ? "ok" : "muted")}>{m.status === "confirmed" ? "確定" : "下書き"}</span>
        <span className="note">{m.sourceName}{m.denoise ? "・ノイズ除去あり" : ""}</span>
        <span className="grow" />
        <button className="btn small" disabled={!d.canUndo} onClick={() => run(() => api.undo(id))}>元に戻す</button>
        <button className="btn small" disabled={!done} onClick={async () => { try { const [n, x] = await api.reapplyGlossary(id); apply(x); setMsg(`用語辞書で ${n} 件を置き換えました(手で直した文は変えません)`); } catch (e) { setMsg(String(e)); } }}>用語辞書を適用</button>
        {m.status === "draft"
          ? <button className="btn small primary" disabled={!done} onClick={() => run(() => api.confirm(id), "確定しました。書き出せます")}>確定</button>
          : <button className="btn small" onClick={() => run(() => api.unconfirm(id))}>下書きに戻す</button>}
        <select aria-label="書き出し" value="" disabled={m.status !== "confirmed"} onChange={(e) => { if (e.target.value) exp(e.target.value as ExportFormat | "wav"); }}>
          <option value="">書き出し…</option>
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
      {m.status !== "confirmed" && done && <p className="note">内容を確認して「確定」すると書き出せます。</p>}
      {msg && <p className="msg">{msg}</p>}
      {m.state === "failed" && <p className="msg err">処理できませんでした: {m.error} <button className="btn small" onClick={onRetry}>やり直す</button></p>}

      {audio ? (
        <audio ref={player} controls src={audio} className="player" onTimeUpdate={(e) => setNow(e.currentTarget.currentTime * 1000)} />
      ) : (
        <p className="note">{m.hasAudio ? "" : "音声を残さない設定のため、再生できません。"}</p>
      )}
      <label className="check note"><input type="checkbox" checked={follow} onChange={(e) => setFollow(e.target.checked)} /> 話者を付けるとき、同じ話者が続く文にもまとめて付ける</label>
      <datalist id="speakers">{d.speakers.map((s) => <option key={s} value={s} />)}</datalist>

      <ol className="segs">
        {d.segments.map(row)}
      </ol>
      {!d.segments.length && <p className="empty">{done ? "文字にできる音声がありませんでした。" : "文字起こしの結果はここに表示されます。"}</p>}
    </div>
  );
}
