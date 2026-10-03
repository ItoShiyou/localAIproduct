/**
 * バックエンド(Tauri の invoke)への薄い API 層。画面はこの `Api` だけに依存する。
 * Tauri の中なら `tauriApi`(src-tauri/src/commands.rs の invoke コマンド)、
 * ブラウザだけで動かすとき(この環境の Playwright など)は `mockApi` に切り替わる。
 * モックは固定の架空データを返すだけで、実際のOCRは行わない。
 */
import type { ConfirmInput, Duplicate, Fields, FieldKey, ReadResult, SearchHit } from "./types";
import { emptyFields } from "./types";

export interface Api {
  readonly kind: "tauri" | "mock";
  /** 画像(data URL)を取り込み、OCRと項目抽出をして下書きを作る */
  readCard(imageDataUrl: string, side?: "front" | "back"): Promise<ReadResult>;
  /** 確認画面で直した内容で確定する(ここで初めて検索に出る) */
  confirm(input: ConfirmInput): Promise<void>;
  /** 下書きを捨てる(やり直し) */
  discardDraft(personId: number): Promise<void>;
  search(query: string): Promise<SearchHit[]>;
}

type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

function tauriInvoke(): Invoke | null {
  const w = window as unknown as { __TAURI__?: { core?: { invoke?: Invoke } } };
  return w.__TAURI__?.core?.invoke ?? null;
}

export function makeTauriApi(invoke: Invoke): Api {
  return {
    kind: "tauri",
    readCard: (imageDataUrl, side = "front") => invoke("import_card", { imageDataUrl, side }),
    confirm: (input) => invoke("confirm_card", { input }),
    discardDraft: (personId) => invoke("discard_draft", { personId }),
    search: (query) => invoke("search", { query }),
  };
}

// ---------------- モック(架空データ。実OCRではない) ----------------

interface Sample {
  fields: Partial<Fields>;
  confidence: Partial<Record<FieldKey, number>>;
  extraEmails?: string[];
}

const SAMPLES: Sample[] = [
  {
    fields: {
      name: "青木 遥", nameKana: "あおき はるか", company: "株式会社ひなた工房", department: "制作部 デザイン課", title: "課長",
      email: "aoki@hinata-kobo.example.com", phone: "03-0000-1101", mobile: "090-0000-1101",
      postalCode: "000-0001", address: "東京都千代田区1-2-3 サンプルビル5F", url: "https://hinata-kobo.example.com",
    },
    confidence: { name: 0.93, nameKana: 0.82, company: 0.9, department: 0.84, title: 0.88, email: 0.95, phone: 0.95, mobile: 0.95, postalCode: 0.94, address: 0.81, url: 0.9 },
  },
  {
    fields: {
      name: "井上 健太", nameKana: "イノウエヶンタ", company: "合同会社みなと設計", department: "営業部", title: "部長",
      email: "inoue@minato-sekkei.example.org", phone: "045-000-2201", postalCode: "000-0002", address: "神奈川県横浜市中区4-5-6", url: "www.minato-sekkei.example.org",
    },
    confidence: { name: 0.9, nameKana: 0.55, company: 0.9, department: 0.62, title: 0.85, email: 0.95, phone: 0.95, postalCode: 0.9, address: 0.5, url: 0.88 },
  },
  {
    fields: {
      name: "Jordan Ellis", company: "Larkspur Analytics Inc.", title: "Director of Operations",
      email: "jordan.ellis@larkspur-analytics.example.com", phone: "(555) 010-0147", address: "1200 Example Avenue, Suite 400 Springfield, ST 00000",
      url: "www.larkspur-analytics.example.com",
    },
    confidence: { name: 0.88, company: 0.9, title: 0.8, email: 0.97, phone: 0.9, address: 0.78, url: 0.9 },
    extraEmails: ["ellis.j@larkspur-analytics.example.com"],
  },
];

interface MockPerson {
  id: number;
  fields: Fields;
  image: string;
  confirmed: boolean;
  memos: { metOn: string; place: string; howMet: string; memo: string }[];
  tags: string[];
}

export function makeMockApi(delayMs = 500): Api {
  const people = new Map<number, MockPerson>();
  let nextId = 1;
  let sampleIdx = 0;
  const wait = () => new Promise<void>((r) => setTimeout(r, delayMs));
  const norm = (s: string) => s.normalize("NFKC").replace(/\s+/g, "").toLowerCase();

  return {
    kind: "mock",
    async readCard(imageDataUrl) {
      await wait();
      const s = SAMPLES[sampleIdx++ % SAMPLES.length];
      const fields = { ...emptyFields(), ...s.fields };
      const id = nextId++;
      people.set(id, { id, fields, image: imageDataUrl, confirmed: false, memos: [], tags: [] });
      const duplicates: Duplicate[] = [];
      for (const p of people.values()) {
        if (!p.confirmed) continue;
        const reasons: Duplicate["reasons"] = [];
        if (fields.email && norm(p.fields.email) === norm(fields.email)) reasons.push("sameEmail");
        if (fields.name && norm(p.fields.name) === norm(fields.name) && norm(p.fields.company) === norm(fields.company)) reasons.push("sameNameAndCompany");
        if (reasons.length) duplicates.push({ existingId: p.id, name: p.fields.name, company: p.fields.company, reasons });
      }
      return { personId: id, fields, confidence: s.confidence, extraEmails: s.extraEmails ?? [], duplicates };
    },
    async confirm(input) {
      const p = people.get(input.personId);
      if (!p) throw new Error("その人が見つかりません");
      p.fields = input.fields;
      p.confirmed = true;
      p.tags = input.tags;
      if (input.encounter) p.memos.unshift(input.encounter);
    },
    async discardDraft(personId) {
      const p = people.get(personId);
      if (p && !p.confirmed) people.delete(personId);
    },
    async search(query) {
      const terms = norm(query).length ? query.normalize("NFKC").toLowerCase().split(/\s+/).filter(Boolean) : [];
      if (!terms.length) return [];
      const out: SearchHit[] = [];
      for (const p of people.values()) {
        if (!p.confirmed) continue;
        const hay = [p.fields.name, p.fields.nameKana, p.fields.company, p.fields.department, p.fields.title,
          ...p.memos.flatMap((m) => [m.memo, m.howMet, m.place]), ...p.tags].join(" ").normalize("NFKC").toLowerCase().replace(/\s+/g, "");
        const hay2 = hay;
        if (terms.every((t) => hay2.includes(t.replace(/\s+/g, "")))) {
          out.push({ id: p.id, name: p.fields.name, company: p.fields.company, title: p.fields.title, image: p.image, latestMemo: p.memos.find((m) => m.memo)?.memo ?? "" });
        }
      }
      return out;
    },
  };
}

export function createApi(): Api {
  const inv = tauriInvoke();
  return inv ? makeTauriApi(inv) : makeMockApi();
}
