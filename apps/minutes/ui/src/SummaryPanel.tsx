import { useEffect, useRef, useState } from "react";
import type { Api } from "./api";
import type { Detail, DraftTodo, Meeting, Plan, SummaryStatus, Todo } from "./types";
import { PRO_LABEL, SUMMARY_NOTE } from "./types";

/** 取り込みの対象(チェックを付けたものだけ議事録のメモに入れる) */
export interface Picked { summary: string[]; decisions: string[]; todos: DraftTodo[] }

const norm = (s: string) => s.replace(/[\s。、.,]/g, "");
const DATE = /^\d{4}-\d{2}-\d{2}$/;

/** 1行に1つの欄(議題・決定事項)に、まだ無い項目だけを足す。すでにある内容は変えない */
export function addLines(existing: string, items: string[]): string {
  const have = new Set(existing.split("\n").map(norm).filter(Boolean));
  const add: string[] = [];
  for (const raw of items) {
    const t = raw.trim();
    const k = norm(t);
    if (!k || have.has(k)) continue;
    have.add(k);
    add.push(t);
  }
  return add.length ? [existing.replace(/\n+$/, ""), ...add].filter((x) => x !== "").join("\n") : existing;
}

/**
 * 選んだ項目を、いまのメモ(議題・決定事項・ToDo)に足した結果を返す。保存はしない(呼び出し側が確認のあと updateNotes で保存する)。
 * 要点 → 議題、決定事項 → 決定事項、ToDo → ToDo。ToDo の期限は日付(YYYY-MM-DD)のときだけ期限の欄に入れ、
 * 「来週の月曜日まで」のような話した表現は、日付に直さず ToDo の文の末尾に添える。
 */
export function mergeIntoNotes(m: Pick<Meeting, "agenda" | "decisions" | "todos">, p: Picked): { agenda: string; decisions: string; todos: Todo[] } {
  const todos = [...m.todos];
  const have = new Set(todos.map((t) => norm(t.text)));
  for (const t of p.todos) {
    const text = t.text.trim();
    if (!text) continue;
    const due = t.due.trim();
    const full = due && !DATE.test(due) ? `${text}(期限: ${due})` : text;
    if (have.has(norm(full)) || have.has(norm(text))) continue;
    have.add(norm(full));
    todos.push({ text: full, owner: t.owner.trim(), due: DATE.test(due) ? due : "", done: false });
  }
  return { agenda: addLines(m.agenda, p.summary), decisions: addLines(m.decisions, p.decisions), todos };
}

interface Row<T> { v: T; on: boolean }
const rows = <T,>(xs: T[]): Row<T>[] => xs.map((v) => ({ v, on: true }));

/** 「要約」タブ。下書きを作る → 確認・修正(チェック)→ 確認のあとで議題・決定事項・ToDo に取り込む。自動では保存しない */
export function SummaryPanel({ api, d, plan, onImport, onOpenSettings }: {
  api: Api; d: Detail; plan: Plan | null;
  onImport: (picked: Picked, count: number) => Promise<void>;
  onOpenSettings: () => void;
}) {
  const m = d.meeting;
  const [st, setSt] = useState<SummaryStatus | null>(null);
  const [running, setRunning] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const [err, setErr] = useState<string | null>(null);
  const [draft, setDraft] = useState<{ summary: Row<string>[]; decisions: Row<string>[]; todos: Row<DraftTodo>[] } | null>(null);
  const [ask, setAsk] = useState(false);
  const [stats, setStats] = useState<string | null>(null);
  const alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => { api.summaryStatus().then(setSt).catch(() => undefined); }, [api]);

  // 要約中は、進み具合を見に行く(要約の呼び出しは終わるまで返らない)
  useEffect(() => {
    if (!running) return;
    const t0 = Date.now();
    const t = setInterval(() => {
      setElapsed(Math.floor((Date.now() - t0) / 1000));
      api.summaryStatus().then((s) => alive.current && setSt(s)).catch(() => undefined);
    }, 500);
    return () => clearInterval(t);
  }, [api, running]);

  const pro = plan?.summary ?? false;
  const ready = !!st?.installed;
  const done = m.state === "done" && !d.provisional && !m.recording && d.segments.some((s) => s.text.trim());

  const start = async () => {
    setErr(null); setStats(null); setAsk(false); setElapsed(0); setRunning(true);
    try {
      const r = await api.summarize(m.id);
      if (!alive.current) return;
      setDraft({ summary: rows(r.draft.summary), decisions: rows(r.draft.decisions), todos: rows(r.draft.todos) });
      setStats(`${r.stats.seconds.toFixed(0)} 秒・${r.stats.chunks > 1 ? `${r.stats.chunks} 区間に分けて整理` : "一度で整理"}`);
    } catch (e) { if (alive.current) setErr(String(e).replace(/^Error: /, "")); }
    finally { if (alive.current) { setRunning(false); api.summaryStatus().then(setSt).catch(() => undefined); } }
  };

  const picked: Picked | null = draft && {
    summary: draft.summary.filter((r) => r.on).map((r) => r.v),
    decisions: draft.decisions.filter((r) => r.on).map((r) => r.v),
    todos: draft.todos.filter((r) => r.on).map((r) => r.v),
  };
  const count = picked ? picked.summary.length + picked.decisions.length + picked.todos.length : 0;

  const doImport = async () => {
    if (!picked) return;
    setAsk(false);
    try { await onImport(picked, count); setDraft(null); setStats(null); }
    catch (e) { setErr(String(e).replace(/^Error: /, "")); }
  };

  const setRow = <K extends "summary" | "decisions">(k: K, i: number, p: Partial<Row<string>>) =>
    setDraft((x) => x && { ...x, [k]: x[k].map((r, j) => (j === i ? { ...r, ...p } : r)) });
  const setTodo = (i: number, p: Partial<Row<DraftTodo>>, v?: Partial<DraftTodo>) =>
    setDraft((x) => x && { ...x, todos: x.todos.map((r, j) => (j === i ? { ...r, ...p, v: { ...r.v, ...v } } : r)) });

  const list = (k: "summary" | "decisions", label: string, hint: string) => draft && (
    <div className="sum-group">
      <h3>{label}<small>{hint}</small></h3>
      {draft[k].length === 0 && <p className="note">(ありません)</p>}
      {draft[k].map((r, i) => (
        <div key={i} className="sum-row">
          <input type="checkbox" checked={r.on} aria-label={`${label}を取り込む`} onChange={(e) => setRow(k, i, { on: e.target.checked })} />
          <textarea rows={2} value={r.v} aria-label={label} onChange={(e) => setRow(k, i, { v: e.target.value })} />
        </div>
      ))}
    </div>
  );

  return (
    <div className="card sum" data-testid="summary">
      <div className="sum-head">
        <h2>要約{!pro && <span className="pro">{PRO_LABEL}</span>}</h2>
        {pro && ready && !running && <button className="btn primary" data-testid="summary-run" disabled={!done} onClick={start}>{draft ? "もう一度要約を作る" : "要約を作る"}</button>}
        {running && <button className="btn" data-testid="summary-cancel" onClick={() => api.cancelSummarize()}>中断</button>}
      </div>
      <p className="note sum-note" data-testid="summary-note">{SUMMARY_NOTE}</p>

      {!pro && (
        <p className="note" data-testid="summary-locked">要約は有料版の機能です。会話から要点・決定事項・やることの下書きを作ります。有料版にすると、設定から要約を追加できます。購入キーをお持ちの方は、設定の「ライセンス」に入力してください。</p>
      )}
      {pro && st && !ready && (
        <div data-testid="summary-nomodel">
          <p className="note">要約を使うための準備がまだ済んでいません。設定の「要約(追加機能)」から取得するか、ファイルから取り込むと使えます(取得するときだけ通信します。録音や文字は送りません)。</p>
          <button className="btn small" onClick={onOpenSettings}>設定を開く</button>
        </div>
      )}
      {pro && ready && st && !st.engine && <p className="msg err">このアプリには要約のエンジンが入っていません。アプリを入れ直してください。</p>}
      {pro && ready && !done && !running && <p className="note">文字起こしが終わると、要約を作れます。</p>}

      {running && (
        <div className="progress" data-testid="summary-progress">
          <div className="bar indeterminate"><span /></div>
          <p className="note">
            {st?.phase || "準備しています"}
            {st && st.total > 1 ? `(${st.step} / ${st.total})` : ""}
            {st && st.generated > 0 ? `・${st.generated} トークン生成` : ""}
            ・{elapsed} 秒経過。長い会議では数分かかります。中断もできます。
          </p>
        </div>
      )}
      {err && <p className="msg err" data-testid="summary-error">{err}</p>}

      {draft && !running && (
        <div data-testid="summary-draft">
          <p className="note">内容を読んで、直したり、取り込まない項目のチェックを外したりしてください。「取り込む」を押して確認するまで、議事録には何も保存されません。{stats ? `(${stats})` : ""}</p>
          {list("summary", "要点", "→ 議題に入ります")}
          {list("decisions", "決定事項", "→ 決定事項に入ります")}
          <div className="sum-group">
            <h3>ToDo<small>→ ToDo に入ります</small></h3>
            {draft.todos.length === 0 && <p className="note">(ありません)</p>}
            {draft.todos.map((r, i) => (
              <div key={i} className="sum-row todo">
                <input type="checkbox" checked={r.on} aria-label="ToDo を取り込む" onChange={(e) => setTodo(i, { on: e.target.checked })} />
                <input className="grow" value={r.v.text} aria-label="やること" onChange={(e) => setTodo(i, {}, { text: e.target.value })} />
                <input className="owner" value={r.v.owner} placeholder="担当" aria-label="担当" onChange={(e) => setTodo(i, {}, { owner: e.target.value })} />
                <input className="due" value={r.v.due} placeholder="期限" aria-label="期限" onChange={(e) => setTodo(i, {}, { due: e.target.value })} />
              </div>
            ))}
          </div>
          <div className="sum-actions">
            {!ask ? (
              <>
                <button className="btn primary" data-testid="summary-import" disabled={count === 0} onClick={() => setAsk(true)}>議題・決定事項・ToDo に取り込む</button>
                <button className="btn" onClick={() => { setDraft(null); setStats(null); }}>下書きを捨てる</button>
              </>
            ) : (
              <span className="ask" data-testid="summary-confirm">
                選んだ {count} 件を、この議事録の「メモ」に追加します(いまのメモは消えません。あとから「メモ」で直せます)。よろしいですか?
                <button className="btn small primary" data-testid="summary-confirm-yes" onClick={doImport}>追加する</button>
                <button className="btn small" onClick={() => setAsk(false)}>やめる</button>
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
