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
}

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

export interface Detail { meeting: Meeting; segments: Segment[]; canUndo: boolean; speakers: string[]; lowConfidence: number }
export interface Progress { busy: boolean; pending: number; meetingId: number | null; doneChunks: number; totalChunks: number }
export interface ImportResult { name: string; id: number | null; error: string | null }
export interface SearchHit { meetingId: number; title: string; heldOn: string | null; segmentId: number; startMs: number; text: string }
export interface GlossaryEntry { id: number; wrong: string; right: string }
export interface NetworkEntry { purpose: string; destination: string; content: string; stoppable: boolean; enabled: boolean }
export interface SettingsInfo {
  updateCheck: boolean; keepAudio: boolean; denoiseDefault: boolean; consentShown: boolean;
  network: NetworkEntry[]; dataDir: string; model: string;
}
export type Flag = "update_check" | "keep_audio" | "denoise_default" | "consent_shown";
export type ExportFormat = "md" | "txt" | "srt";

export const hms = (ms: number) => {
  const s = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), x = s % 60;
  return (h ? `${h}:${String(m).padStart(2, "0")}` : `${m}`) + `:${String(x).padStart(2, "0")}`;
};

export const STATE_LABEL: Record<Meeting["state"], string> = { queued: "待ち", processing: "処理中", done: "文字起こし済み", failed: "失敗" };

export interface ModelInfo {
  name: string; installed: boolean; downloaded: number; size: number;
  downloading: boolean; source: "managed" | "env" | "none"; error: string | null;
}
