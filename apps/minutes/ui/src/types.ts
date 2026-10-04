/** src-tauri/src/commands.rs・store.rs の DTO と同じ形(camelCase)。 */
export interface Meeting {
  id: number;
  title: string;
  heldOn: string | null;
  participantsText: string;
  sourceName: string;
  hasAudio: boolean;
  durationMs: number | null;
  denoise: boolean;
  state: "queued" | "processing" | "done" | "failed";
  error: string | null;
  status: "draft" | "confirmed";
  createdAt: number;
  diarize: boolean;
  numSpeakers: number | null;
  language: Language;
  rangeStartMs: number | null;
  rangeEndMs: number | null;
  agenda: string;
  decisions: string;
  todos: Todo[];
  tags: string[];
  recording: boolean;
}

export type Language = "ja" | "en" | "auto";
export interface Todo { text: string; owner: string; due: string; done: boolean }
export interface ProcessOptions {
  denoise: boolean; diarize: boolean; numSpeakers: number | null; language: Language;
  rangeStartMs: number | null; rangeEndMs: number | null;
}
export interface MeetingFilter { tag?: string | null; sort?: "held_desc" | "held_asc" | "created_desc" | "title" }
export const LANGUAGE_LABEL: Record<Language, string> = { ja: "日本語", en: "英語", auto: "自動判定" };

export interface Segment {
  id: number;
  chunkIdx: number;
  startMs: number;
  endMs: number;
  speaker: string;
  text: string;
  rawText: string;
  confidence: number;
  edited: boolean;
}

export interface Detail { meeting: Meeting; segments: Segment[]; canUndo: boolean; speakers: string[]; lowConfidence: number; provisional: boolean }
export interface RecordStatus { meetingId: number; elapsedMs: number; pendingChunks: number; level: number }
export interface Progress { busy: boolean; pending: number; meetingId: number | null; doneChunks: number; totalChunks: number }
export interface ImportResult { name: string; id: number | null; error: string | null }
export interface SearchHit { meetingId: number; title: string; heldOn: string | null; segmentId: number; startMs: number; text: string }
export interface GlossaryEntry { id: number; wrong: string; right: string }
export interface NetworkEntry { purpose: string; destination: string; content: string; stoppable: boolean; enabled: boolean }
export interface SettingsInfo {
  updateCheck: boolean; keepAudio: boolean; denoiseDefault: boolean; consentShown: boolean;
  network: NetworkEntry[]; dataDir: string; model: string;
  diarizeAvailable: boolean; diarizeError: string | null;
}
export type Flag = "update_check" | "keep_audio" | "denoise_default" | "consent_shown";
export type ExportFormat = "md" | "txt" | "srt" | "docx";

export const hms = (ms: number) => {
  const s = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), x = s % 60;
  return (h ? `${h}:${String(m).padStart(2, "0")}` : `${m}`) + `:${String(x).padStart(2, "0")}`;
};

export const STATE_LABEL: Record<Meeting["state"], string> = { queued: "待ち", processing: "処理中", done: "文字起こし済み", failed: "失敗" };

export interface ModelInfo {
  name: string; installed: boolean; downloaded: number; size: number;
  downloading: boolean; source: "bundled" | "managed" | "env" | "none"; error: string | null;
}

/** "1:02:03" / "62:03" / "45" をミリ秒に。空なら null、読めなければ -1 */
export function parseTime(s: string): number | null {
  const t = s.trim();
  if (!t) return null;
  if (!/^\d+(:\d{1,2}){0,2}$/.test(t)) return -1;
  return t.split(":").map(Number).reduce((a, v) => a * 60 + v, 0) * 1000;
}

/** 話者ごとの色(名前から決める) */
export function speakerColor(name: string): string {
  if (!name) return "transparent";
  const palette = ["#2563eb", "#d97706", "#059669", "#db2777", "#7c3aed", "#0891b2", "#65a30d", "#dc2626"];
  const m = /^話者(\d+)$/.exec(name);
  if (m) return palette[(Number(m[1]) - 1) % palette.length];
  let h = 0;
  for (const c of name) h = (h * 31 + c.codePointAt(0)!) >>> 0;
  return palette[h % palette.length];
}

export interface Plan {
  tier: "free" | "pro";
  accurateModel: boolean; diarize: boolean; denoise: boolean; glossary: boolean; summary: boolean;
  exports: string[];
  totalLimitMs: number | null; meetingLimitMs: number | null;
  usage: { usedMs: number; count: number; tampered: boolean };
  remainingMs: number | null;
}
export const PRO_LABEL = "有料版";
