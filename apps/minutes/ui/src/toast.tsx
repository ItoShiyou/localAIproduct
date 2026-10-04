import { useSyncExternalStore } from "react";

/** 画面の右下(プレーヤーの上)に出す短いお知らせ。成功は数秒で消え、エラーは閉じるまで残る */
export type ToastKind = "ok" | "info" | "err";
export interface ToastItem { id: number; kind: ToastKind; text: string }

let items: ToastItem[] = [];
let seq = 1;
const subs = new Set<() => void>();
const emit = () => subs.forEach((f) => f());

export function dismissToast(id: number) {
  items = items.filter((t) => t.id !== id);
  emit();
}

export function notify(kind: ToastKind, text: string) {
  // 同じ文言が出ているときは重ねない(続けて同じ失敗が起きても山にならないように)
  if (items.some((t) => t.kind === kind && t.text === text)) return;
  const id = seq++;
  items = [...items, { id, kind, text }].slice(-4);
  emit();
  if (kind !== "err") setTimeout(() => dismissToast(id), kind === "info" ? 7000 : 4000);
}

const subscribe = (f: () => void) => { subs.add(f); return () => { subs.delete(f); }; };
const snapshot = () => items;

export function Toasts() {
  const list = useSyncExternalStore(subscribe, snapshot);
  return (
    <div className="toasts">
      {list.map((t) => (
        <div key={t.id} className={`toast ${t.kind}`} role={t.kind === "err" ? "alert" : "status"} data-testid="toast">
          <span className="toast-text">{t.text}</span>
          {t.kind === "err" && <button className="toast-x" aria-label="閉じる" onClick={() => dismissToast(t.id)}>×</button>}
        </div>
      ))}
    </div>
  );
}
