import { useCallback, useEffect, useState } from "react";
import type { Api } from "./api";
import { Editor, Original } from "./ReviewView";
import type { Receipt, SearchQuery } from "./types";
import { yen } from "./types";

const num = (s: string) => (s.trim() === "" || isNaN(Number(s.replace(/,/g, ""))) ? null : Number(s.replace(/,/g, "")));

export function ListView({ api, version, onChanged }: { api: Api; version: number; onChanged: () => void }) {
  const [text, setText] = useState("");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [min, setMin] = useState("");
  const [max, setMax] = useState("");
  const [status, setStatus] = useState("");
  const [rows, setRows] = useState<Receipt[]>([]);
  const [open, setOpen] = useState<number | null>(null);
  const [delAsk, setDelAsk] = useState<number | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [enc, setEnc] = useState<"utf8" | "sjis">("utf8");

  const load = useCallback(async () => {
    const q: SearchQuery = { text, dateFrom: from || null, dateTo: to || null, minTotal: num(min), maxTotal: num(max), status: status || null };
    setRows(await api.list(q));
  }, [api, text, from, to, min, max, status]);
  useEffect(() => { load().catch((e) => setMsg(String(e))); }, [load, version]);

  const exp = async (kind: "csv" | "sqlite") => {
    try {
      const p = kind === "csv" ? await api.exportCsv(enc) : await api.exportSqlite();
      if (p) setMsg(`書き出しました: ${p}(確定済みの行だけ)`);
    } catch (e) { setMsg(`書き出せませんでした: ${e}`); }
  };

  const item = rows.find((r) => r.id === open) ?? null;
  const sum = rows.filter((r) => r.status === "confirmed").reduce((a, r) => a + (r.total ?? 0), 0);

  return (
    <div className="pane">
      <section className="card filters">
        <input className="grow" placeholder="支払先・摘要・登録番号で検索(空白区切りで絞り込み)" value={text} onChange={(e) => setText(e.target.value)} />
        <label>日付 <input type="date" value={from} onChange={(e) => setFrom(e.target.value)} /> 〜 <input type="date" value={to} onChange={(e) => setTo(e.target.value)} /></label>
        <label>金額 <input className="short" inputMode="numeric" value={min} onChange={(e) => setMin(e.target.value)} /> 〜 <input className="short" inputMode="numeric" value={max} onChange={(e) => setMax(e.target.value)} /> 円</label>
        <select value={status} onChange={(e) => setStatus(e.target.value)}>
          <option value="">すべて</option><option value="confirmed">確定済み</option><option value="draft">下書き</option>
        </select>
      </section>
      <section className="card exports">
        <span>書き出し(確定済みだけ):</span>
        <select value={enc} onChange={(e) => setEnc(e.target.value as "utf8" | "sjis")}>
          <option value="utf8">CSV(UTF-8 BOM付き)</option><option value="sjis">CSV(Shift_JIS)</option>
        </select>
        <button className="btn small" onClick={() => exp("csv")}>CSVで書き出す</button>
        <button className="btn small" onClick={() => exp("sqlite")}>SQLiteで書き出す</button>
      </section>
      {msg && <p className="note">{msg}</p>}
      <p className="note">{rows.length} 件(確定済みの合計 {yen(sum)})</p>
      <div className="table-wrap">
        <table className="table">
          <thead><tr><th>日付</th><th>支払先</th><th className="r">税込合計</th><th>税率</th><th>登録番号</th><th>勘定科目(候補)</th><th>状態</th><th></th></tr></thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.id} className={r.id === open ? "open" : ""}>
                <td>{r.date ?? "—"}</td>
                <td><button className="link" onClick={() => setOpen(open === r.id ? null : r.id)}>{r.vendor || "(支払先なし)"}</button></td>
                <td className="r">{yen(r.total)}</td>
                <td>{r.taxRate ? `${r.taxRate}%` : "—"}</td>
                <td>{r.invoiceNo ?? "—"}</td>
                <td>{r.accountCandidate ?? "—"}</td>
                <td>{r.status === "confirmed" ? "確定" : <span className="badge">下書き</span>}</td>
                <td>
                  {delAsk === r.id ? (
                    <span className="ask">原本のコピーも消えます。
                      <button className="btn small danger" onClick={async () => { await api.remove(r.id); setDelAsk(null); setOpen(null); await load(); onChanged(); }}>消す</button>
                      <button className="btn small" onClick={() => setDelAsk(null)}>やめる</button>
                    </span>
                  ) : <button className="btn small ghost" onClick={() => setDelAsk(r.id)}>削除</button>}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {item && (
        <section className="detail inline">
          <Original api={api} r={item} />
          <Editor api={api} r={item} onSaved={(n) => setRows((rs) => rs.map((x) => (x.id === n.id ? n : x)))} onConfirmed={() => { load(); onChanged(); }} />
        </section>
      )}
    </div>
  );
}
