import { useCallback, useEffect, useState } from "react";
import type { Api } from "./api";
import type { FieldKey, Receipt } from "./types";
import { FIELDS, yen } from "./types";

/** 原本の表示(画像はそのまま、PDF は埋め込み表示) */
export function Original({ api, r }: { api: Api; r: Receipt }) {
  const [src, setSrc] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    setSrc(null); setErr(null);
    api.original(r.documentId).then((s) => alive && setSrc(s)).catch((e) => alive && setErr(String(e)));
    return () => { alive = false; };
  }, [api, r.documentId]);
  if (err) return <div className="original err">原本を表示できません: {err}</div>;
  if (!src) return <div className="original">読み込み中…</div>;
  return (
    <div className="original">
      {r.kind === "pdf" ? <iframe title="原本(PDF)" src={src} /> : <img src={src} alt="原本" />}
    </div>
  );
}

/** 1件の確認・修正フォーム(確認画面と一覧の詳細で共用) */
export function Editor({ api, r, onSaved, onConfirmed }: { api: Api; r: Receipt; onSaved: (r: Receipt) => void; onConfirmed: () => void }) {
  const [vals, setVals] = useState<Record<FieldKey, string>>({} as Record<FieldKey, string>);
  const [msg, setMsg] = useState<string | null>(null);

  useEffect(() => {
    const v = {} as Record<FieldKey, string>;
    for (const f of FIELDS) v[f.key] = r[f.prop] == null ? "" : String(r[f.prop]);
    setVals(v);
    setMsg(null);
  }, [r]);

  const save = async (key: FieldKey) => {
    const cur = r[FIELDS.find((f) => f.key === key)!.prop];
    if ((cur == null ? "" : String(cur)) === vals[key]) return;
    try { onSaved(await api.edit(r.id, key, vals[key] || null)); setMsg(null); }
    catch (e) { setMsg(String(e)); }
  };

  const confirm = async () => {
    const rej = await api.confirm([r.id]);
    if (rej.length) setMsg("確定できません。印の付いた項目を直してください。");
    else onConfirmed();
  };

  const issueOf = (k: FieldKey) => r.issues.find((i) => i.field === k);

  return (
    <div className="editor">
      <p className="note">{r.originalName} ・ {r.status === "confirmed" ? "確定済み" : "下書き(まだ確定していません)"}</p>
      {FIELDS.map((f) => {
        const low = r.lowConfidence.includes(f.key);
        const issue = issueOf(f.key);
        return (
          <div key={f.key} className={"field" + (low ? " low" : "") + (issue ? " bad" : "")}>
            <label htmlFor={`f-${f.key}`}>
              {f.label}
              {low && <span className="badge">要確認</span>}
            </label>
            <input id={`f-${f.key}`} value={vals[f.key] ?? ""} placeholder={f.placeholder} inputMode={f.numeric ? "numeric" : undefined}
              onChange={(e) => setVals({ ...vals, [f.key]: e.target.value })}
              onBlur={() => save(f.key)}
              onKeyDown={(e) => { if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }} />
            {issue && <p className="issue">{issue.message}</p>}
          </div>
        );
      })}
      <p className="note">勘定科目の候補: {r.accountCandidate ?? "(候補なし)"}(自動では確定しません)</p>
      {msg && <p className="issue">{msg}</p>}
      <div className="row">
        <button className="btn" onClick={() => api.undo(r.id).then(onSaved)}>元に戻す</button>
        {r.status === "draft"
          ? <button className="btn primary" onClick={confirm} disabled={r.issues.length > 0}>この内容で確定</button>
          : <button className="btn" onClick={() => api.unconfirm(r.id).then(() => api.get(r.id)).then(onSaved)}>下書きに戻す</button>}
      </div>
    </div>
  );
}

export function ReviewView({ api, version, onChanged }: { api: Api; version: number; onChanged: () => void }) {
  const [rows, setRows] = useState<Receipt[]>([]);
  const [cur, setCur] = useState<number | null>(null);
  const [note, setNote] = useState<string | null>(null);

  const load = useCallback(async () => {
    const d = await api.list({ text: "", status: "draft" });
    setRows(d);
    setCur((c) => (c != null && d.some((r) => r.id === c) ? c : d[0]?.id ?? null));
  }, [api]);
  useEffect(() => { load().catch(() => undefined); }, [load, version]);

  const item = rows.find((r) => r.id === cur) ?? null;
  const ready = rows.filter((r) => r.issues.length === 0);

  const confirmAll = async () => {
    const rej = await api.confirm(ready.map((r) => r.id));
    setNote(`${ready.length - rej.length} 件を確定しました${rej.length ? `(${rej.length} 件は項目に問題があり確定していません)` : ""}`);
    await load();
    onChanged();
  };

  if (!rows.length) return <div className="pane empty">確認を待っている読み取り結果はありません。</div>;

  return (
    <div className="review">
      <aside className="side">
        <div className="side-head">
          <span>{rows.length} 件</span>
          <button className="btn small" disabled={!ready.length} onClick={confirmAll} title="問題の指摘が無い下書きだけを確定します">指摘なしの {ready.length} 件を一括確定</button>
        </div>
        {note && <p className="note">{note}</p>}
        <ul className="items">
          {rows.map((r) => (
            <li key={r.id}>
              <button className={r.id === cur ? "current" : ""} onClick={() => setCur(r.id)}>
                <span className="v">{r.vendor || "(支払先なし)"}</span>
                <span className="d">{r.date ?? "日付なし"} ・ {yen(r.total)}</span>
                {(r.issues.length > 0 || r.lowConfidence.length > 0) && <span className="badge">{r.issues.length ? `指摘 ${r.issues.length}` : "要確認"}</span>}
              </button>
            </li>
          ))}
        </ul>
      </aside>
      {item && (
        <section className="detail">
          <Original api={api} r={item} />
          <Editor api={api} r={item}
            onSaved={(n) => setRows((rs) => rs.map((x) => (x.id === n.id ? n : x)))}
            onConfirmed={() => { load(); onChanged(); }} />
        </section>
      )}
    </div>
  );
}
