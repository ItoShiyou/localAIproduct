/** src-tauri/src/commands.rs の DTO と同じ形(camelCase)。 */
export type FieldKey = "date" | "vendor" | "total" | "tax_rate" | "invoice_no" | "summary";

export interface Receipt {
  id: number;
  documentId: number;
  originalName: string;
  kind: "pdf" | "jpeg" | "png";
  date: string | null;
  vendor: string | null;
  total: number | null;
  taxRate: number | null;
  invoiceNo: string | null;
  summary: string | null;
  accountCandidate: string | null;
  status: "draft" | "confirmed";
  lowConfidence: string[];
  issues: { field: FieldKey; message: string }[];
}

export interface ImportFile { name: string; dataUrl: string }
export interface ImportResult { name: string; outcome: "imported" | "duplicate" | "failed"; message: string }
export interface Progress { pending: number; running: number; done: number; failed: number; busy: boolean; errors: string[] }
export interface SearchQuery { text: string; dateFrom?: string | null; dateTo?: string | null; minTotal?: number | null; maxTotal?: number | null; status?: string | null }
export interface Rejected { id: number; fields: FieldKey[] }
export interface NetworkEntry { purpose: string; destination: string; content: string; stoppable: boolean; enabled: boolean }
export interface SettingsInfo { updateCheck: boolean; network: NetworkEntry[]; dataDir: string; ocr: "real" | "fake" }

export const FIELDS: { key: FieldKey; label: string; prop: keyof Receipt; numeric?: boolean; placeholder?: string }[] = [
  { key: "date", label: "日付", prop: "date", placeholder: "YYYY-MM-DD" },
  { key: "vendor", label: "支払先", prop: "vendor" },
  { key: "total", label: "税込合計(円)", prop: "total", numeric: true },
  { key: "tax_rate", label: "税率(%)", prop: "taxRate", numeric: true, placeholder: "8 / 10(混在なら空欄)" },
  { key: "invoice_no", label: "登録番号", prop: "invoiceNo", placeholder: "T + 13桁" },
  { key: "summary", label: "摘要", prop: "summary" },
];

export const yen = (n: number | null) => (n == null ? "—" : `¥${n.toLocaleString("ja-JP")}`);
