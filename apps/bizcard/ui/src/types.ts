/** 項目の定義。Rust 側 `PersonFields`(store.rs)と同じ並び。 */
export const FIELD_KEYS = [
  "name", "nameKana", "company", "department", "title",
  "email", "phone", "mobile", "postalCode", "address", "url",
] as const;
export type FieldKey = (typeof FIELD_KEYS)[number];
export type Fields = Record<FieldKey, string>;

export const FIELD_LABELS: Record<FieldKey, string> = {
  name: "氏名", nameKana: "ふりがな", company: "会社", department: "部署", title: "役職",
  email: "メール", phone: "電話", mobile: "携帯", postalCode: "郵便番号", address: "住所", url: "URL",
};

/** 「要確認」の印を付ける閾値(Rust 側 `REVIEW_THRESHOLD` と同じ 0.7) */
export const REVIEW_THRESHOLD = 0.7;

export const emptyFields = (): Fields =>
  Object.fromEntries(FIELD_KEYS.map((k) => [k, ""])) as Fields;

/** 読み取り結果(Rust `import_card` の戻り値に対応) */
export interface Duplicate {
  existingId: number;
  name: string;
  company: string;
  reasons: ("sameEmail" | "sameNameAndCompany")[];
}
export interface ReadResult {
  personId: number;
  fields: Fields;
  /** 項目ごとの信頼度(0〜1)。検出できなかった項目は無い */
  confidence: Partial<Record<FieldKey, number>>;
  /** 複数あったメールの2つ目以降(選ぶのは利用者) */
  extraEmails: string[];
  /** 同じ人かもしれない確定済みの人。統合はしない */
  duplicates: Duplicate[];
  /** 同じ画像を取り込み済みなら、その人の id */
  alreadyImported?: number;
}
export interface Encounter {
  metOn: string;
  place: string;
  howMet: string;
  memo: string;
}
export interface ConfirmInput {
  personId: number;
  fields: Fields;
  encounter: Encounter | null;
  tags: string[];
}
export interface SearchHit {
  id: number;
  name: string;
  company: string;
  title: string;
  /** 名刺画像(data URL または表示可能な URL) */
  image: string | null;
  latestMemo: string;
}
