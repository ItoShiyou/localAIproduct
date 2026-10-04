import { useEffect, useState } from "react";
import type { Api } from "./api";
import type { GlossaryEntry } from "./types";

export function GlossaryView({ api }: { api: Api }) {
  const [items, setItems] = useState<GlossaryEntry[]>([]);
  const [wrong, setWrong] = useState("");
  const [right, setRight] = useState("");
  const [msg, setMsg] = useState<string | null>(null);

  useEffect(() => { api.glossary().then(setItems).catch((e) => setMsg(String(e))); }, [api]);

  const add = async () => {
    try { setItems(await api.addGlossary(wrong, right)); setWrong(""); setRight(""); setMsg(null); }
    catch (e) { setMsg(String(e)); }
  };

  return (
    <div className="pane">
      <section className="card">
        <h2>用語辞書</h2>
        <p className="note">専門用語や固有名詞を、文字起こしの後に正しい表記へ置き換えます。文字起こしの結果そのものは残るので、確認画面で元に戻せます。正しい表記は、文字起こしのヒントにも使います。</p>
        <div className="row">
          <input placeholder="誤りやすい表記(例: やまだ商事)" value={wrong} onChange={(e) => setWrong(e.target.value)} />
          <span>→</span>
          <input placeholder="正しい表記(例: 山田商事)" value={right} onChange={(e) => setRight(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") add(); }} />
          <button className="btn primary" onClick={add}>追加</button>
        </div>
        {msg && <p className="msg err">{msg}</p>}
      </section>
      <table className="table">
        <thead><tr><th>誤りやすい表記</th><th>正しい表記</th><th></th></tr></thead>
        <tbody>
          {items.map((g) => (
            <tr key={g.id}><td>{g.wrong}</td><td>{g.right}</td>
              <td><button className="btn small ghost" onClick={async () => setItems(await api.deleteGlossary(g.id))}>削除</button></td></tr>
          ))}
        </tbody>
      </table>
      {!items.length && <p className="note">まだ登録はありません。</p>}
      <p className="note">登録した後に、既存の議事録へ反映するには、確認画面の「用語辞書を適用」を押してください。</p>
    </div>
  );
}
