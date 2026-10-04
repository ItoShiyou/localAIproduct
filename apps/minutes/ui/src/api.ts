/**
 * バックエンド(Tauri の invoke)への薄い API 層。画面はこの `Api` だけに依存する。
 * Tauri の中なら `makeTauriApi`(src-tauri/src/tauri_glue.rs)、ブラウザだけで開いたときは `makeMockApi`(架空の固定データ)。
 */
import type { Detail, ExportFormat, Flag, GlossaryEntry, ImportResult, Meeting, MeetingFilter, ModelInfo, Plan, ProcessOptions, Progress, RecordStatus, SearchHit, Segment, SettingsInfo, Todo } from "./types";

export interface Api {
  readonly kind: "tauri" | "mock";
  /** OS のダイアログで選んで取り込む */
  pickAndImport(opts: ProcessOptions): Promise<ImportResult[]>;
  importPaths(paths: string[], opts: ProcessOptions): Promise<ImportResult[]>;
  /** ドラッグ&ドロップされたファイルの場所を受け取る(Tauri のときだけ)。戻り値は解除の関数 */
  onDrop(cb: (paths: string[]) => void): Promise<() => void>;
  runJobs(): Promise<number>;
  cancelJobs(): Promise<void>;
  retry(id: number): Promise<void>;
  progress(): Promise<Progress>;
  meetings(filter?: MeetingFilter): Promise<Meeting[]>;
  allTags(): Promise<[string, number][]>;
  setTags(id: number, tags: string[]): Promise<Detail>;
  updateNotes(id: number, agenda: string, decisions: string, todos: Todo[]): Promise<Detail>;
  /** 話者を判別し直す(人数 null は自動)。名前を付けた話者も上書きする(元に戻せる) */
  rediarize(id: number, numSpeakers: number | null): Promise<Detail>;
  renameSpeaker(id: number, oldName: string, newName: string): Promise<Detail>;
  findIn(id: number, query: string): Promise<number[]>;
  replaceIn(id: number, find: string, replace: string): Promise<[number, Detail]>;
  /** 設定を変えて最初から文字起こしし直す(それまでの修正は消える) */
  reprocess(id: number, opts: ProcessOptions): Promise<void>;
  /** 画面を印刷する(印刷用の表示にしてから呼ぶ) */
  printPage(): Promise<void>;
  exportGlossary(): Promise<string | null>;
  importGlossary(): Promise<[number, string[], GlossaryEntry[]] | null>;
  /** マイクの録音を始める(議事録の id) */
  recordStart(opts: ProcessOptions): Promise<number>;
  /** 16kHz モノラル i16 を Base64 にした音を足す */
  recordPush(pcmB64: string): Promise<RecordStatus>;
  /** 止める(正確な文字起こしを待ちに入れる。そのあと runJobs を呼ぶ) */
  recordStop(): Promise<Detail>;
  recordDiscard(): Promise<void>;
  /** 無料版/有料版と、無料版の使った量 */
  plan(): Promise<Plan>;
  detail(id: number): Promise<Detail>;
  /** 再生用の URL(音声を残していなければ null) */
  audioUrl(id: number): Promise<string | null>;
  updateMeta(id: number, title: string, heldOn: string | null, participants: string): Promise<Detail>;
  editText(segmentId: number, text: string): Promise<Detail>;
  setSpeaker(segmentId: number, speaker: string, following: boolean): Promise<Detail>;
  mergeNext(segmentId: number): Promise<Detail>;
  split(segmentId: number, at: number): Promise<Detail>;
  revertSegment(segmentId: number): Promise<Detail>;
  undo(id: number): Promise<Detail>;
  reapplyGlossary(id: number): Promise<[number, Detail]>;
  confirm(id: number): Promise<Detail>;
  unconfirm(id: number): Promise<Detail>;
  deleteMeeting(id: number): Promise<void>;
  exportAs(id: number, format: ExportFormat): Promise<string | null>;
  exportDenoised(id: number): Promise<string | null>;
  search(query: string): Promise<SearchHit[]>;
  glossary(): Promise<GlossaryEntry[]>;
  addGlossary(wrong: string, right: string): Promise<GlossaryEntry[]>;
  deleteGlossary(id: number): Promise<GlossaryEntry[]>;
  settings(): Promise<SettingsInfo>;
  setFlag(key: Flag, on: boolean): Promise<SettingsInfo>;
  deleteAll(): Promise<number>;
  modelStatus(): Promise<ModelInfo>;
  /** モデルを取得する(利用者が押したときだけ)。終わるまで返らない */
  downloadModel(): Promise<ModelInfo>;
  cancelModelDownload(): Promise<void>;
  deleteModel(): Promise<ModelInfo>;
  /** 第三者のソフトウェア・モデルのライセンス表記(全文) */
  thirdPartyNotices(): Promise<string>;
}

type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
interface TauriGlobal {
  core?: { invoke?: Invoke; convertFileSrc?: (p: string) => string };
  event?: { listen?: <T>(name: string, cb: (e: { payload: T }) => void) => Promise<() => void> };
}

function tauri(): TauriGlobal | null {
  const w = window as unknown as { __TAURI__?: TauriGlobal };
  return w.__TAURI__?.core?.invoke ? w.__TAURI__ : null;
}

export function makeTauriApi(t: TauriGlobal): Api {
  const invoke = t.core!.invoke!;
  return {
    kind: "tauri",
    pickAndImport: (opts) => invoke("pick_and_import", { opts }),
    importPaths: (paths, opts) => invoke("import_paths", { paths, opts }),
    async onDrop(cb) {
      const listen = t.event?.listen;
      if (!listen) return () => undefined;
      return listen<{ paths: string[] }>("tauri://drag-drop", (e) => cb(e.payload.paths ?? []));
    },
    runJobs: () => invoke("run_jobs"),
    cancelJobs: () => invoke("cancel_jobs"),
    retry: (id) => invoke("retry", { id }),
    progress: () => invoke("progress"),
    meetings: (filter) => invoke("meetings", { filter: filter ?? null }),
    allTags: () => invoke("all_tags"),
    setTags: (id, tags) => invoke("set_tags", { id, tags }),
    updateNotes: (id, agenda, decisions, todos) => invoke("update_notes", { id, agenda, decisions, todos }),
    rediarize: (id, numSpeakers) => invoke("rediarize", { id, numSpeakers }),
    renameSpeaker: (id, oldName, newName) => invoke("rename_speaker", { id, old: oldName, new: newName }),
    findIn: (id, query) => invoke("find_in", { id, query }),
    replaceIn: (id, find, replace) => invoke("replace_in", { id, find, replace }),
    reprocess: (id, opts) => invoke("reprocess", { id, opts }),
    printPage: () => invoke("print_page"),
    exportGlossary: () => invoke("export_glossary"),
    importGlossary: () => invoke("import_glossary"),
    recordStart: (opts) => invoke("record_start", { opts }),
    recordPush: (pcm) => invoke("record_push", { pcm }),
    recordStop: () => invoke("record_stop"),
    recordDiscard: () => invoke("record_discard"),
    plan: () => invoke("plan"),
    detail: (id) => invoke("detail", { id }),
    async audioUrl(id) {
      const p = await invoke<string | null>("audio_path", { id });
      return p && t.core?.convertFileSrc ? t.core.convertFileSrc(p) : null;
    },
    updateMeta: (id, title, heldOn, participants) => invoke("update_meta", { id, title, heldOn, participants }),
    editText: (segmentId, text) => invoke("edit_text", { segmentId, text }),
    setSpeaker: (segmentId, speaker, following) => invoke("set_speaker", { segmentId, speaker, following }),
    mergeNext: (segmentId) => invoke("merge_next", { segmentId }),
    split: (segmentId, at) => invoke("split", { segmentId, at }),
    revertSegment: (segmentId) => invoke("revert_segment", { segmentId }),
    undo: (id) => invoke("undo", { id }),
    reapplyGlossary: (id) => invoke("reapply_glossary", { id }),
    confirm: (id) => invoke("confirm", { id }),
    unconfirm: (id) => invoke("unconfirm", { id }),
    deleteMeeting: (id) => invoke("delete_meeting", { id }),
    exportAs: (id, format) => invoke("export", { id, format }),
    exportDenoised: (id) => invoke("export_denoised", { id }),
    search: (query) => invoke("search", { query }),
    glossary: () => invoke("glossary"),
    addGlossary: (wrong, right) => invoke("add_glossary", { wrong, right }),
    deleteGlossary: (id) => invoke("delete_glossary", { id }),
    settings: () => invoke("settings"),
    setFlag: (key, on) => invoke("set_flag", { key, on }),
    deleteAll: () => invoke("delete_all"),
    modelStatus: () => invoke("model_status"),
    downloadModel: () => invoke("download_model"),
    cancelModelDownload: () => invoke("cancel_model_download"),
    deleteModel: () => invoke("delete_model"),
    thirdPartyNotices: () => invoke("third_party_notices"),
  };
}

// ---------------- モック(架空データ。実際の文字起こしではない) ----------------

const SCRIPT = [
  ["", "それでは定例会議をはじめます。"], ["", "本日の議題は二つです。"], ["", "まず、新しい見積もりの件について、やまだ商事から回答がありました。"],
  ["", "金額は前回より一割ほど下がっています。"], ["", "次に、来月の展示会の準備状況です。"], ["", "ブースの設営は外部に依頼済みです。"],
];

export function makeMockApi(): Api {
  let meetings: Meeting[] = [];
  const segs = new Map<number, Segment[]>();
  const hist = new Map<number, Segment[][]>();
  let glossary: GlossaryEntry[] = [];
  let nextId = 1, nextSeg = 1, pending: number[] = [], printed = 0;
  let recording: { id: number; pushes: number } | null = null;
  let usedMs = 0;
  let settings: SettingsInfo = {
    updateCheck: true, keepAudio: true, denoiseDefault: true, consentShown: false, dataDir: "(モック)", model: "mock",
    diarizeAvailable: true, diarizeError: null,
    network: [
      { purpose: "ライセンス認証・検証", destination: "販売プラットフォームのAPI", content: "ライセンスキー、端末の識別名", stoppable: false, enabled: true },
      { purpose: "更新の確認", destination: "配布元", content: "アプリのバージョン", stoppable: true, enabled: true },
      { purpose: "モデルの取得(操作したときのみ)", destination: "モデルの配布元", content: "モデル名", stoppable: false, enabled: true },
    ],
  };
  let model: ModelInfo = { name: "Whisper large-v3-turbo(q5_0)", installed: true, downloaded: 574041195, size: 574041195, downloading: false, source: "managed", error: null };
  let current: Progress = { busy: false, pending: 0, meetingId: null, doneChunks: 0, totalChunks: 0 };
  const wait = (ms = 200) => new Promise<void>((r) => setTimeout(r, ms));
  const gl = (t: string) => glossary.reduce((s, g) => s.split(g.wrong).join(g.right), t);
  const m = (id: number) => { const x = meetings.find((y) => y.id === id); if (!x) throw new Error("見つかりません"); return x; };
  const draft = (id: number) => { meetings = meetings.map((x) => (x.id === id ? { ...x, status: "draft" } : x)); };
  const cp = (id: number) => { hist.set(id, [...(hist.get(id) ?? []), (segs.get(id) ?? []).map((s) => ({ ...s }))]); draft(id); };
  const owner = (sid: number) => { for (const [id, ss] of segs) if (ss.some((s) => s.id === sid)) return id; throw new Error("文が見つかりません"); };
  const det = async (id: number): Promise<Detail> => {
    const ss = segs.get(id) ?? [];
    return { meeting: m(id), segments: ss, canUndo: (hist.get(id) ?? []).length > 0, speakers: [...new Set(ss.map((s) => s.speaker).filter(Boolean))], lowConfidence: 0.6, provisional: ss.some((s) => s.chunkIdx < 0) };
  };
  const add = (names: string[], o: ProcessOptions): ImportResult[] => names.map((name) => {
    if (!/\.(m4a|mp3|wav|mp4|aac|flac|ogg|mov|m4v)$/i.test(name)) return { name, id: null, error: "対応している形式は m4a / mp3 / wav / mp4 などです" };
    const id = nextId++;
    meetings = [{
      id, title: name.replace(/\.[^.]+$/, ""), heldOn: null, participantsText: "", sourceName: name, hasAudio: true, durationMs: null,
      denoise: o.denoise, state: "queued", error: null, status: "draft", createdAt: Date.now() / 1000,
      diarize: o.diarize, numSpeakers: o.numSpeakers, language: o.language, rangeStartMs: o.rangeStartMs, rangeEndMs: o.rangeEndMs,
      agenda: "", decisions: "", todos: [], tags: [], recording: false,
    }, ...meetings];
    pending.push(id);
    return { name, id, error: null };
  });

  return {
    kind: "mock",
    async pickAndImport(o) { return add([`定例会議-${nextId}.m4a`], o); },
    async importPaths(paths, o) { return add(paths.map((p) => p.split(/[\\/]/).pop()!), o); },
    async onDrop() { return () => undefined; },
    async runJobs() {
      const ids = pending; pending = [];
      for (const id of ids) {
        meetings = meetings.map((x) => (x.id === id ? { ...x, state: "processing" } : x));
        for (let c = 1; c <= 3; c++) { current = { busy: true, pending: ids.length, meetingId: id, doneChunks: c, totalChunks: 3 }; await wait(250); }
        const mm = m(id);
        segs.set(id, SCRIPT.map(([, t], i) => ({
          id: nextSeg++, chunkIdx: Math.floor(i / 2), startMs: i * 4000, endMs: i * 4000 + 3500,
          speaker: mm.diarize ? `話者${[1, 2, 1, 2, 3, 3][i]}` : "", text: gl(t), rawText: t, confidence: i === 2 || i === 4 ? 0.45 : 0.9, edited: false,
        })));
        meetings = meetings.map((x) => (x.id === id ? { ...x, state: "done", durationMs: 24_000, hasAudio: settings.keepAudio } : x));
        usedMs += 24_000;
      }
      current = { busy: false, pending: 0, meetingId: null, doneChunks: 0, totalChunks: 0 };
      return ids.length;
    },
    async cancelJobs() {},
    async retry(id) { pending.push(id); },
    async progress() { return { ...current, pending: pending.length + (current.busy ? 1 : 0) }; },
    async meetings(f) {
      let ms = f?.tag ? meetings.filter((x) => x.tags.includes(f.tag!)) : [...meetings];
      if (f?.sort === "title") ms = ms.sort((a, b) => a.title.localeCompare(b.title));
      if (f?.sort === "held_asc") ms = ms.sort((a, b) => (a.heldOn ?? "9999").localeCompare(b.heldOn ?? "9999"));
      return ms;
    },
    async allTags() {
      const c = new Map<string, number>();
      for (const x of meetings) for (const t of x.tags) c.set(t, (c.get(t) ?? 0) + 1);
      return [...c.entries()].sort();
    },
    async setTags(id, tags) { meetings = meetings.map((x) => (x.id === id ? { ...x, tags: [...new Set(tags.map((t) => t.trim()).filter(Boolean))].sort() } : x)); return det(id); },
    async updateNotes(id, agenda, decisions, todos) {
      if (todos.some((t) => t.due && !/^\d{4}-\d{2}-\d{2}$/.test(t.due))) throw new Error("ToDo の期限は YYYY-MM-DD の形で入力してください");
      meetings = meetings.map((x) => (x.id === id ? { ...x, agenda, decisions, todos: todos.filter((t) => t.text.trim()), status: "draft" } : x));
      return det(id);
    },
    async rediarize(id, n) {
      cp(id);
      const k = n ?? 3;
      segs.set(id, (segs.get(id) ?? []).map((s, i) => ({ ...s, speaker: `話者${(i % k) + 1}` })));
      return det(id);
    },
    async renameSpeaker(id, oldName, newName) {
      if (!newName.trim()) throw new Error("名前を入力してください");
      cp(id); segs.set(id, (segs.get(id) ?? []).map((s) => (s.speaker === oldName ? { ...s, speaker: newName.trim() } : s))); return det(id);
    },
    async findIn(id, q) { const t = q.trim().toLowerCase(); return t ? (segs.get(id) ?? []).filter((s) => s.text.toLowerCase().includes(t)).map((s) => s.id) : []; },
    async replaceIn(id, f, r) {
      if (!f) throw new Error("置き換える語を入力してください");
      const hits = (segs.get(id) ?? []).filter((s) => s.text.includes(f)).length;
      if (hits) { cp(id); segs.set(id, (segs.get(id) ?? []).map((s) => (s.text.includes(f) ? { ...s, text: s.text.split(f).join(r), edited: true } : s))); }
      return [hits, await det(id)];
    },
    async reprocess(id, o) {
      meetings = meetings.map((x) => (x.id === id ? { ...x, ...o, state: "queued", status: "draft" } : x));
      segs.delete(id); hist.delete(id); pending.push(id);
    },
    async printPage() { printed++; },
    async exportGlossary() { return "(モックでは書き出しません)"; },
    async recordStart(o) {
      const [r] = add(["マイク録音.wav"], o);
      const id = r.id!;
      pending = pending.filter((x) => x !== id);
      meetings = meetings.map((x) => (x.id === id ? { ...x, title: "録音", sourceName: "マイク録音", state: "processing", recording: true } : x));
      segs.set(id, []);
      recording = { id, pushes: 0 };
      return id;
    },
    async recordPush() {
      if (!recording) throw new Error("録音していません");
      recording.pushes++;
      const id = recording.id;
      if (recording.pushes % 6 === 0) {
        const i = recording.pushes / 6 - 1;
        segs.set(id, [...(segs.get(id) ?? []), { id: nextSeg++, chunkIdx: -(i + 1), startMs: i * 3000, endMs: i * 3000 + 2800, speaker: "", text: `仮の文字 ${i + 1}`, rawText: "", confidence: 0.5, edited: false }]);
      }
      return { meetingId: id, elapsedMs: recording.pushes * 500, pendingChunks: 0, level: 0.05 };
    },
    async recordStop() {
      if (!recording) throw new Error("録音していません");
      const id = recording.id;
      recording = null;
      meetings = meetings.map((x) => (x.id === id ? { ...x, state: "queued", recording: false, durationMs: 24_000 } : x));
      pending.push(id);
      return det(id);
    },
    async plan() {
      const free = typeof location !== "undefined" && location.search.includes("free");
      const used = free ? usedMs : 0;
      return free
        ? { tier: "free", accurateModel: false, diarize: false, denoise: false, glossary: false, summary: false, exports: ["txt"], totalLimitMs: 3_600_000, meetingLimitMs: 900_000, usage: { usedMs: used, count: 0, tampered: false }, remainingMs: Math.max(0, 3_600_000 - used) }
        : { tier: "pro", accurateModel: true, diarize: true, denoise: true, glossary: true, summary: true, exports: ["docx", "pdf", "md", "txt", "srt", "wav"], totalLimitMs: null, meetingLimitMs: null, usage: { usedMs: 0, count: 0, tampered: false }, remainingMs: null };
    },
    async recordDiscard() { if (recording) { const id = recording.id; meetings = meetings.filter((x) => x.id !== id); segs.delete(id); recording = null; } },
    async importGlossary() { glossary = [...glossary, { id: nextId++, wrong: "くらうど", right: "クラウド" }]; return [1, [], glossary]; },
    detail: det,
    async audioUrl() { return null; },
    async updateMeta(id, title, heldOn, participants) {
      if (!title.trim()) throw new Error("タイトルを入力してください");
      meetings = meetings.map((x) => (x.id === id ? { ...x, title, heldOn, participantsText: participants, status: "draft" } : x));
      return det(id);
    },
    async editText(sid, text) { const id = owner(sid); cp(id); segs.set(id, segs.get(id)!.map((s) => (s.id === sid ? { ...s, text, edited: true } : s))); return det(id); },
    async setSpeaker(sid, speaker, following) {
      const id = owner(sid); cp(id);
      const ss = segs.get(id)!; const i = ss.findIndex((s) => s.id === sid); const old = ss[i].speaker;
      let j = i; const out = ss.map((s) => ({ ...s }));
      out[j].speaker = speaker;
      while (following && ++j < out.length && out[j].speaker === old) out[j].speaker = speaker;
      segs.set(id, out); return det(id);
    },
    async mergeNext(sid) {
      const id = owner(sid); const ss = segs.get(id)!; const i = ss.findIndex((s) => s.id === sid);
      if (i + 1 >= ss.length) throw new Error("次の文がありません");
      cp(id); const a = ss[i], b = ss[i + 1];
      segs.set(id, [...ss.slice(0, i), { ...a, endMs: b.endMs, text: a.text + b.text, rawText: a.rawText + b.rawText, edited: true }, ...ss.slice(i + 2)]);
      return det(id);
    },
    async split(sid, at) {
      const id = owner(sid); const ss = segs.get(id)!; const i = ss.findIndex((s) => s.id === sid); const a = ss[i];
      const ch = [...a.text]; if (at <= 0 || at >= ch.length) throw new Error("分ける位置は文の途中にしてください");
      cp(id); const t = a.startMs + Math.round(((a.endMs - a.startMs) * at) / ch.length);
      segs.set(id, [...ss.slice(0, i), { ...a, text: ch.slice(0, at).join(""), endMs: t, edited: true }, { ...a, id: nextSeg++, text: ch.slice(at).join(""), rawText: "", startMs: t, edited: true }, ...ss.slice(i + 1)]);
      return det(id);
    },
    async revertSegment(sid) { const id = owner(sid); cp(id); segs.set(id, segs.get(id)!.map((s) => (s.id === sid ? { ...s, text: gl(s.rawText), edited: false } : s))); return det(id); },
    async undo(id) { const h = hist.get(id) ?? []; const last = h.pop(); if (last) segs.set(id, last); draft(id); return det(id); },
    async reapplyGlossary(id) {
      let n = 0; const ss = (segs.get(id) ?? []).map((s) => { if (s.edited) return s; const t = gl(s.rawText); if (t !== s.text) n++; return { ...s, text: t }; });
      if (n) { cp(id); segs.set(id, ss); }
      return [n, await det(id)];
    },
    async confirm(id) {
      if (m(id).state !== "done") throw new Error("文字起こしが終わってから確定してください");
      meetings = meetings.map((x) => (x.id === id ? { ...x, status: "confirmed" } : x)); return det(id);
    },
    async unconfirm(id) { draft(id); return det(id); },
    async deleteMeeting(id) { meetings = meetings.filter((x) => x.id !== id); segs.delete(id); },
    async exportAs(id) { if (m(id).status !== "confirmed") throw new Error("確定した議事録だけを書き出せます。確認画面で確定してください"); await wait(); return "(モックでは書き出しません)"; },
    async exportDenoised() { await wait(); return "(モックでは書き出しません)"; },
    async search(q) {
      const t = q.trim(); if (!t) return [];
      const out: SearchHit[] = [];
      for (const x of meetings) for (const s of segs.get(x.id) ?? []) if (s.text.includes(t)) out.push({ meetingId: x.id, title: x.title, heldOn: x.heldOn, segmentId: s.id, startMs: s.startMs, text: s.text });
      return out;
    },
    async glossary() { return glossary; },
    async addGlossary(wrong, right) {
      if (!wrong.trim() || !right.trim()) throw new Error("誤りやすい表記と正しい表記の両方を入力してください");
      glossary = [...glossary.filter((g) => g.wrong !== wrong), { id: nextId++, wrong, right }]; return glossary;
    },
    async deleteGlossary(id) { glossary = glossary.filter((g) => g.id !== id); return glossary; },
    async settings() { return settings; },
    async setFlag(key, on) {
      const k = ({ update_check: "updateCheck", keep_audio: "keepAudio", denoise_default: "denoiseDefault", consent_shown: "consentShown" } as const)[key];
      settings = { ...settings, [k]: on, network: key === "update_check" ? settings.network.map((e) => (e.stoppable ? { ...e, enabled: on } : e)) : settings.network };
      return settings;
    },
    async deleteAll() { const n = meetings.length; meetings = []; segs.clear(); glossary = []; return n; },
    async modelStatus() { return model; },
    async downloadModel() {
      model = { ...model, downloading: true, downloaded: 0, installed: false };
      for (let i = 1; i <= 5; i++) { await wait(150); model = { ...model, downloaded: (model.size * i) / 5 }; }
      model = { ...model, downloading: false, installed: true, source: "managed" };
      return model;
    },
    async cancelModelDownload() {},
    async deleteModel() { model = { ...model, installed: false, downloaded: 0, source: "none" }; return model; },
    async thirdPartyNotices() { return "THIRD-PARTY SOFTWARE NOTICES(モック)\n\nWhisper large-v3-turbo — MIT\n..."; },
  };
}

export function createApi(): Api {
  const t = tauri();
  return t ? makeTauriApi(t) : makeMockApi();
}
