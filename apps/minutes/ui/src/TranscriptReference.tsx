import { useMemo, useState } from "react";
import type { Segment } from "./types";
import { hms } from "./types";
import { findTranscript } from "./transcriptSearch";

export function TranscriptReference({ segments, audio, onListen, onEdit }: {
  segments: Segment[]; audio: boolean;
  onListen: (segment: Segment) => void; onEdit: (segment: Segment) => void;
}) {
  const [query, setQuery] = useState("");
  const hits = useMemo(() => findTranscript(segments, query), [segments, query]);
  return <aside className="source-transcript" aria-label="元の発言">
    <h3>元の発言を確かめる</h3>
    <label className="source-search">発言・話者を検索
      <input type="search" value={query} onChange={event => setQuery(event.target.value)} placeholder="人名、数字、気になる言葉" />
    </label>
    <p className="note" role="status">{query.trim() ? `${hits.length} / ${segments.length} 文` : `${segments.length} 文`}</p>
    <p className="note">要約の根拠を自動で判定するものではありません。前後の発言も確認してください。</p>
    {hits.map(segment => <div key={segment.id}>
      <button className="listen" disabled={!audio} onClick={() => onListen(segment)}>{hms(segment.startMs)} {segment.speaker || "話者未設定"} ▶ この部分を聞く</button>
      <p>{segment.text}</p>
      <button className="link" onClick={() => onEdit(segment)} aria-label={`${hms(segment.startMs)} の文を修正する`}>この文を修正する</button>
    </div>)}
    {!hits.length && <p className="note">{segments.length ? "一致する発言がありません。別の言葉で検索してください。" : "まだ発言がありません。"}</p>}
    {query && <button className="btn small" onClick={() => setQuery("")}>検索を解除して前後を見る</button>}
  </aside>;
}
