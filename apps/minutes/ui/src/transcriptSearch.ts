import type { Segment } from "./types";

/** Literal lookup only: never present a similarity guess as evidence for a summary. */
export function findTranscript(segments: Segment[], query: string): Segment[] {
  const words = query.normalize("NFKC").trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return segments;
  return segments.filter(segment => {
    const text = `${segment.speaker} ${segment.text}`.normalize("NFKC").toLocaleLowerCase();
    return words.every(word => text.includes(word));
  });
}
