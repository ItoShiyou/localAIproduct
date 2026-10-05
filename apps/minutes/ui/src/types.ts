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
/** 話者の判別し直しの進み具合。phase は prepare(音声の読み込み)| embed(声の特徴を求める) */
export interface RediarizeStatus { meetingId: number; done: number; total: number; phase: string }
export interface ImportResult { name: string; id: number | null; error: string | null }
export interface SearchHit { meetingId: number; title: string; heldOn: string | null; segmentId: number; startMs: number; text: string }
export interface GlossaryEntry { id: number; wrong: string; right: string }
export interface NetworkEntry { purpose: string; destination: string; content: string; stoppable: boolean; enabled: boolean }
export interface SettingsInfo {
  updateCheck: boolean; keepAudio: boolean; denoiseDefault: boolean; consentShown: boolean;
  network: NetworkEntry[]; dataDir: string; model: string;
  diarizeAvailable: boolean; diarizeError: string | null;
  /** 「ネットワークを使わない」(通信する機能を止めている) */
  offlineMode: boolean;
  /** オフライン版のライセンスのため、切り替えられない */
  offlineForced: boolean;
}
export type Flag = "update_check" | "keep_audio" | "denoise_default" | "consent_shown" | "offline_mode";
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

let speakerOrder: string[] = [];
/** 開いている議事録の話者を、出てきた順に覚える(名前を付け替えても、同じ位置の話者は同じ色のまま。名前の当て推量で色が重ならない) */
export function setSpeakerOrder(names: string[]) { speakerOrder = names; }

/** 話者ごとの色(色はデザイントークンの --spk-1〜8)。開いている議事録では出てきた順、それ以外は名前から決める */
export function speakerColor(name: string): string {
  if (!name) return "transparent";
  const i = speakerOrder.indexOf(name);
  if (i >= 0) return `var(--spk-${(i % 8) + 1})`;
  const m = /^話者(\d+)$/.exec(name);
  if (m) return `var(--spk-${((Number(m[1]) - 1) % 8) + 1})`;
  let h = 0;
  for (const c of name) h = (h * 31 + c.codePointAt(0)!) >>> 0;
  return `var(--spk-${(h % 8) + 1})`;
}

/** 「たった今」「3分前」「昨日」… */
export function ago(sec: number): string {
  const d = Date.now() / 1000 - sec;
  if (d < 60) return "たった今";
  if (d < 3600) return `${Math.floor(d / 60)}分前`;
  if (d < 86400) return `${Math.floor(d / 3600)}時間前`;
  if (d < 172800) return "昨日";
  const t = new Date(sec * 1000);
  return `${t.getMonth() + 1}/${t.getDate()}`;
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

/** 要約(有料版の追加機能)。src-tauri/src/summary.rs の DTO と同じ形 */
export interface SummaryStatus {
  name: string; installed: boolean; downloaded: number; size: number;
  downloading: boolean; source: "managed" | "env" | "none"; error: string | null;
  /** 要約のエンジン(サイドカー)がアプリに入っているか */
  engine: boolean;
  license: string; licenseUrl: string;
  running: boolean; meetingId: number | null; step: number; total: number; phase: string; generated: number;
}
export interface DraftTodo { text: string; owner: string; due: string }
export interface SummaryDraft { summary: string[]; decisions: string[]; todos: DraftTodo[] }
export interface SummaryResult {
  draft: SummaryDraft;
  stats: { chunks: number; promptTokens: number; genTokens: number; prefillSeconds: number; genSeconds: number; seconds: number };
}
export const SUMMARY_NOTE = "要約は自動で作った下書きです。内容を確認してから使ってください。";

/** オフラインのライセンス。src-tauri/src/license.rs・commands.rs の DTO と同じ形(docs/license.md) */
export interface LicenseInfo {
  id: string; licensee: string; edition: "pro" | "pro_offline"; issued: string;
  features: string[]; machineBound: boolean; offline: boolean; dev: boolean;
}
export interface LicenseStatus {
  valid: boolean; info: LicenseInfo | null; error: string | null; machineCode: string | null;
  tier: "free" | "pro"; networkDisabled: boolean;
}
