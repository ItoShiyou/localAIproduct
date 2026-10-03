/**
 * バックエンド(Tauri の invoke)への薄い API 層。画面はこの `Api` だけに依存する。
 * Tauri の中なら `makeTauriApi`(src-tauri/src/tauri_glue.rs のコマンド)、ブラウザだけで開いたときは `makeMockApi`。
 * モックは架空の固定データを返すだけで、実際の読み取りは行わない。
 */
import type { FieldKey, ImportFile, ImportResult, Progress, Receipt, Rejected, SearchQuery, SettingsInfo } from "./types";

export interface Api {
  readonly kind: "tauri" | "mock";
  importFiles(files: ImportFile[]): Promise<ImportResult[]>;
  /** 待ちの読み取りをすべて実行する(終わるまで返らない。進捗は progress で見る) */
  runJobs(): Promise<number>;
  cancelJobs(): Promise<void>;
  retryFailed(): Promise<number>;
  progress(): Promise<Progress>;
  list(query: SearchQuery): Promise<Receipt[]>;
  get(id: number): Promise<Receipt>;
  original(documentId: number): Promise<string>;
  edit(id: number, field: FieldKey, value: string | null): Promise<Receipt>;
  undo(id: number): Promise<Receipt>;
  confirm(ids: number[]): Promise<Rejected[]>;
  unconfirm(id: number): Promise<void>;
  remove(id: number): Promise<void>;
  /** 保存先を選んで書き出す。取り消したら null */
  exportCsv(encoding: "utf8" | "sjis"): Promise<string | null>;
  exportSqlite(): Promise<string | null>;
  settings(): Promise<SettingsInfo>;
  setUpdateCheck(on: boolean): Promise<SettingsInfo>;
  deleteAll(): Promise<number>;
}

type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

function tauriInvoke(): Invoke | null {
  const w = window as unknown as { __TAURI__?: { core?: { invoke?: Invoke } } };
  return w.__TAURI__?.core?.invoke ?? null;
}

export function makeTauriApi(invoke: Invoke): Api {
  return {
    kind: "tauri",
    importFiles: (files) => invoke("import_files", { files }),
    runJobs: () => invoke("run_jobs"),
    cancelJobs: () => invoke("cancel_jobs"),
    retryFailed: () => invoke("retry_failed"),
    progress: () => invoke("progress"),
    list: (query) => invoke("list", { query }),
    get: (id) => invoke("get_receipt", { id }),
    original: (documentId) => invoke("original", { documentId }),
    edit: (id, field, value) => invoke("edit", { id, field, value }),
    undo: (id) => invoke("undo", { id }),
    confirm: (ids) => invoke("confirm", { ids }),
    unconfirm: (id) => invoke("unconfirm", { id }),
    remove: (id) => invoke("delete_receipt", { id }),
    exportCsv: (encoding) => invoke("export_csv", { encoding }),
    exportSqlite: () => invoke("export_sqlite"),
    settings: () => invoke("settings"),
    setUpdateCheck: (on) => invoke("set_update_check", { on }),
    deleteAll: () => invoke("delete_all"),
  };
}

// ---------------- モック(架空データ。実際の読み取りではない) ----------------

const SAMPLES: Partial<Receipt>[] = [
  { date: "2026-04-01", vendor: "有限会社みどり商店", total: 880, taxRate: 8, invoiceNo: "T1234567890123", lowConfidence: [] },
  { date: "2026-04-03", vendor: "株式会社サンプル交通", total: 1320, taxRate: 10, invoiceNo: null, lowConfidence: ["vendor"] },
  { date: null, vendor: "架空書店", total: 2200, taxRate: 10, invoiceNo: "T98765", lowConfidence: ["date", "invoice_no"] },
];

function check(r: Receipt): Receipt["issues"] {
  const out: Receipt["issues"] = [];
  if (!r.date) out.push({ field: "date", message: "日付が読み取れていません" });
  else if (!/^\d{4}-\d{2}-\d{2}$/.test(r.date) || isNaN(Date.parse(r.date))) out.push({ field: "date", message: "日付の形式が正しくありません(YYYY-MM-DD)" });
  if (!r.vendor) out.push({ field: "vendor", message: "支払先が読み取れていません" });
  if (r.total == null) out.push({ field: "total", message: "金額が読み取れていません" });
  if (r.taxRate != null && r.taxRate !== 8 && r.taxRate !== 10) out.push({ field: "tax_rate", message: "税率は8%または10%のはずです" });
  if (r.invoiceNo && !/^T\d{13}$/.test(r.invoiceNo)) out.push({ field: "invoice_no", message: "登録番号は T + 13桁の数字の形式です" });
  return out;
}

export function makeMockApi(): Api {
  const rows = new Map<number, Receipt>();
  const images = new Map<number, string>();
  const history = new Map<number, Receipt[]>();
  let pending: { docId: number; name: string; kind: Receipt["kind"] }[] = [];
  let nextId = 1, done = 0, settings: SettingsInfo = {
    updateCheck: true, dataDir: "(モック)", ocr: "fake",
    network: [
      { purpose: "ライセンス認証・検証", destination: "販売プラットフォームのAPI", content: "ライセンスキー、端末の識別名", stoppable: false, enabled: true },
      { purpose: "更新の確認", destination: "配布元", content: "アプリのバージョン", stoppable: true, enabled: true },
      { purpose: "モデルの取得(操作したときのみ)", destination: "モデルの配布元", content: "モデル名", stoppable: false, enabled: true },
    ],
  };
  const wait = (ms = 300) => new Promise<void>((r) => setTimeout(r, ms));
  const fin = (r: Receipt) => ({ ...r, issues: check(r) });
  const need = (id: number) => { const r = rows.get(id); if (!r) throw new Error("見つかりません"); return r; };
  const propOf: Record<FieldKey, keyof Receipt> = { date: "date", vendor: "vendor", total: "total", tax_rate: "taxRate", invoice_no: "invoiceNo", summary: "summary" };

  return {
    kind: "mock",
    async importFiles(files) {
      return files.map((f) => {
        const kind: Receipt["kind"] = f.dataUrl.startsWith("data:application/pdf") ? "pdf" : f.dataUrl.startsWith("data:image/png") ? "png" : "jpeg";
        if (!/^data:(application\/pdf|image\/(png|jpeg))/.test(f.dataUrl)) return { name: f.name, outcome: "failed" as const, message: "PDF・JPEG・PNG のいずれでもありません" };
        const docId = nextId++;
        images.set(docId, f.dataUrl);
        pending.push({ docId, name: f.name, kind });
        return { name: f.name, outcome: "imported" as const, message: "" };
      });
    },
    async runJobs() {
      const n = pending.length;
      for (const p of pending) {
        await wait(400);
        const s = SAMPLES[(p.docId - 1) % SAMPLES.length];
        const r: Receipt = fin({ id: p.docId, documentId: p.docId, originalName: p.name, kind: p.kind, date: null, vendor: null, total: null, taxRate: null, invoiceNo: null, summary: null, accountCandidate: "消耗品費", status: "draft", lowConfidence: [], issues: [], ...s } as Receipt);
        rows.set(r.id, r);
        done++;
      }
      pending = [];
      return n;
    },
    async cancelJobs() {},
    async retryFailed() { return 0; },
    async progress() { return { pending: pending.length, running: 0, done, failed: 0, busy: false, errors: [] }; },
    async list(q) {
      const t = q.text.trim().normalize("NFKC").toLowerCase();
      return [...rows.values()].filter((r) =>
        (!t || [r.vendor, r.summary, r.invoiceNo].join(" ").normalize("NFKC").toLowerCase().includes(t)) &&
        (!q.status || r.status === q.status) && (!q.dateFrom || (r.date ?? "") >= q.dateFrom) && (!q.dateTo || (r.date ?? "") <= q.dateTo) &&
        (q.minTotal == null || (r.total ?? -1) >= q.minTotal) && (q.maxTotal == null || (r.total ?? Infinity) <= q.maxTotal),
      ).sort((a, b) => (b.date ?? "").localeCompare(a.date ?? "") || b.id - a.id);
    },
    async get(id) { return need(id); },
    async original(documentId) { return images.get(documentId) ?? ""; },
    async edit(id, field, value) {
      const r = need(id);
      history.set(id, [...(history.get(id) ?? []), r]);
      const prop = propOf[field];
      const v = value?.trim() ? (field === "total" || field === "tax_rate" ? Number(value.replace(/,/g, "")) : value.trim()) : null;
      if (typeof v === "number" && isNaN(v)) throw new Error("数値で入力してください");
      const n = fin({ ...r, [prop]: v, status: "draft" });
      rows.set(id, n);
      return n;
    },
    async undo(id) {
      const h = history.get(id) ?? [];
      const prev = h.pop();
      if (prev) rows.set(id, prev);
      return need(id);
    },
    async confirm(ids) {
      const rej: Rejected[] = [];
      for (const id of ids) {
        const r = need(id);
        if (r.issues.length) rej.push({ id, fields: r.issues.map((i) => i.field) });
        else { rows.set(id, { ...r, status: "confirmed" }); history.delete(id); }
      }
      return rej;
    },
    async unconfirm(id) { rows.set(id, { ...need(id), status: "draft" }); },
    async remove(id) { rows.delete(id); },
    async exportCsv() { await wait(); return "(モックでは書き出しません)"; },
    async exportSqlite() { await wait(); return "(モックでは書き出しません)"; },
    async settings() { return settings; },
    async setUpdateCheck(on) {
      settings = { ...settings, updateCheck: on, network: settings.network.map((e) => (e.stoppable ? { ...e, enabled: on } : e)) };
      return settings;
    },
    async deleteAll() { const n = rows.size; rows.clear(); images.clear(); return n; },
  };
}

export function createApi(): Api {
  const inv = tauriInvoke();
  return inv ? makeTauriApi(inv) : makeMockApi();
}
