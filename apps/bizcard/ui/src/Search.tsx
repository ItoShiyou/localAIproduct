import { useEffect, useState } from "react";
import type { Api } from "./api";
import type { SearchHit } from "./types";

/** 1つの検索窓。結果は名刺の画像とメモを大きく見せる。 */
export function Search({ api }: { api: Api }) {
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<SearchHit[] | null>(null);

  useEffect(() => {
    let alive = true;
    const t = setTimeout(() => {
      if (!q.trim()) { setHits(null); return; }
      api.search(q).then((h) => alive && setHits(h)).catch(() => alive && setHits([]));
    }, 150);
    return () => { alive = false; clearTimeout(t); };
  }, [q, api]);

  return (
    <div className="search" data-testid="search">
      <input type="search" className="searchbox" placeholder="氏名・会社・役職・メモ・場所・タグ" value={q}
        onChange={(e) => setQ(e.target.value)} data-testid="search-input" aria-label="検索" autoFocus />
      {hits === null && <p className="hint">「この人、誰だっけ」と思ったら、名前の一部やメモの言葉を入れてください。</p>}
      {hits?.length === 0 && <p className="hint" data-testid="no-hit">見つかりませんでした。</p>}
      <ul className="hits">
        {hits?.map((h) => (
          <li key={h.id} className="hit" data-testid="hit">
            {h.image && <img src={h.image} alt={`${h.name}の名刺`} />}
            <div className="who">
              <b>{h.name}</b>
              <span>{h.company}{h.title ? ` / ${h.title}` : ""}</span>
            </div>
            <p className="memo" data-testid="hit-memo">{h.latestMemo || "(メモなし)"}</p>
          </li>
        ))}
      </ul>
    </div>
  );
}
