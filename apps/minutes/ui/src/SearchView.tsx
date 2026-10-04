import { useEffect, useState } from "react";
import type { Api } from "./api";
import type { SearchHit } from "./types";
import { hms } from "./types";
import { PageHead } from "./PageHead";

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
      <PageHead label="Search" title="検索">過去の議事録の文を、言葉で探します。結果を押すと、その位置から開いて再生できます。</PageHead>
      <input className="search" autoFocus placeholder="過去の議事録を全文検索(空白区切りで絞り込み)" value={q} onChange={(e) => setQ(e.target.value)} />
      {msg && <p className="msg err">{msg}</p>}
      {q.trim() && <p className="note">{hits.length} 件</p>}
      {!q.trim() && <p className="hint">言葉を入れると、すべての議事録の文から探します。空白で区切ると、すべてを含む文に絞り込みます。</p>}
      {q.trim() && !hits.length && !msg && <p className="hint">見つかりませんでした。言葉を短くするか、別の言い方で試してください。</p>}
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
