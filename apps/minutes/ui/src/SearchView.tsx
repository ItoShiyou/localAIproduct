import { useEffect, useState } from "react";
import type { Api } from "./api";
import type { SearchHit } from "./types";
import { hms } from "./types";

export function SearchView({ api, onOpen }: { api: Api; onOpen: (meetingId: number, ms: number) => void }) {
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [msg, setMsg] = useState<string | null>(null);

  useEffect(() => {
    const t = setTimeout(() => { api.search(q).then((h) => { setHits(h); setMsg(null); }).catch((e) => setMsg(String(e))); }, 250);
    return () => clearTimeout(t);
  }, [api, q]);

  return (
    <div className="pane">
      <input className="search" autoFocus placeholder="過去の議事録を全文検索(空白区切りで絞り込み)" value={q} onChange={(e) => setQ(e.target.value)} />
      {msg && <p className="msg err">{msg}</p>}
      {q.trim() && <p className="note">{hits.length} 件</p>}
      <ul className="hits">
        {hits.map((h) => (
          <li key={h.segmentId}>
            <button onClick={() => onOpen(h.meetingId, h.startMs)}>
              <span className="v">{h.title}</span>
              <span className="d">{h.heldOn ?? "日付なし"} ・ {hms(h.startMs)}</span>
              <span className="t">{h.text}</span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
