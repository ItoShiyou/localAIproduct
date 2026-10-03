import { useCallback, useRef, useState } from "react";
import type { Api } from "./api";
import { Camera } from "./Camera";
import { Review, emptyEncounter } from "./Review";
import { Search } from "./Search";
import type { Encounter, FieldKey, Fields, ReadResult } from "./types";
import { emptyFields } from "./types";

export interface QueueItem {
  key: number;
  image: string;
  status: "reading" | "ready" | "confirmed" | "error";
  read?: ReadResult;
  fields: Fields;
  /** 打ち替えの履歴(「元に戻す」用) */
  history: Fields[];
  enc: Encounter;
  error?: string;
}

const readAsDataUrl = (f: File) =>
  new Promise<string>((res, rej) => {
    const r = new FileReader();
    r.onload = () => res(String(r.result));
    r.onerror = () => rej(r.error);
    r.readAsDataURL(f);
  });

export function App({ api }: { api: Api }) {
  const [tab, setTab] = useState<"capture" | "search">("capture");
  const [queue, setQueue] = useState<QueueItem[]>([]);
  const [cur, setCur] = useState<number | null>(null); // 表示中の queue の key。null ならカメラ
  const seq = useRef(1);

  const patch = useCallback((key: number, f: (i: QueueItem) => QueueItem) => setQueue((q) => q.map((i) => (i.key === key ? f(i) : i))), []);

  const addImage = useCallback(async (image: string) => {
    const key = seq.current++;
    setQueue((q) => [...q, { key, image, status: "reading", fields: emptyFields(), history: [], enc: emptyEncounter() }]);
    setCur((c) => c ?? key); // 撮影直後は、その名刺の確認を開く
    try {
      const read = await api.readCard(image);
      patch(key, (i) => ({ ...i, status: "ready", read, fields: { ...read.fields } }));
    } catch (e) {
      patch(key, (i) => ({ ...i, status: "error", error: `読み取れませんでした: ${e instanceof Error ? e.message : String(e)}` }));
    }
  }, [api, patch]);

  const onFiles = useCallback(async (files: File[]) => {
    // 失敗したファイルは飛ばして続行する
    for (const f of files) {
      try { await addImage(await readAsDataUrl(f)); } catch { /* 次へ */ }
    }
  }, [addImage]);

  const item = queue.find((i) => i.key === cur) ?? null;
  const pending = queue.filter((i) => i.status !== "confirmed");

  const goNext = (after: number) => {
    const next = queue.find((i) => i.key !== after && i.status !== "confirmed");
    setCur(next ? next.key : null);
  };

  const edit = (key: number) => (k: FieldKey, v: string) =>
    patch(key, (i) => ({ ...i, history: [...i.history, i.fields], fields: { ...i.fields, [k]: v } }));
  const undo = (key: number) => () =>
    patch(key, (i) => (i.history.length ? { ...i, fields: i.history[i.history.length - 1], history: i.history.slice(0, -1) } : i));
  const resetField = (key: number) => (k: FieldKey) =>
    patch(key, (i) => (i.read ? { ...i, history: [...i.history, i.fields], fields: { ...i.fields, [k]: i.read.fields[k] } } : i));

  const confirm = async (it: QueueItem) => {
    if (!it.read) return;
    const e = it.enc;
    const hasEnc = e.place || e.howMet || e.memo;
    try {
      await api.confirm({ personId: it.read.personId, fields: it.fields, encounter: hasEnc ? e : null, tags: [] });
      patch(it.key, (i) => ({ ...i, status: "confirmed" }));
      goNext(it.key);
    } catch (err) {
      patch(it.key, (i) => ({ ...i, status: "error", error: `確定できませんでした: ${err instanceof Error ? err.message : String(err)}` }));
    }
  };

  const redo = async (it: QueueItem) => {
    if (it.read) await api.discardDraft(it.read.personId).catch(() => undefined);
    setQueue((q) => q.filter((i) => i.key !== it.key));
    goNext(it.key);
  };

  return (
    <div className="app">
      <header className="top">
        <h1>名刺管理(仮称)</h1>
        {api.kind === "mock" && <span className="mock" data-testid="mock-banner">画面確認用のモック(実際のOCRは行っていません)</span>}
        <nav role="tablist">
          <button role="tab" aria-selected={tab === "capture"} onClick={() => setTab("capture")} data-testid="tab-capture">撮影{pending.length ? `(${pending.length})` : ""}</button>
          <button role="tab" aria-selected={tab === "search"} onClick={() => setTab("search")} data-testid="tab-search">検索</button>
        </nav>
      </header>

      {tab === "search" ? (
        <main className="main"><Search api={api} /></main>
      ) : (
        <main className="main capture">
          {queue.length > 0 && (
            <ol className="queue" data-testid="queue" aria-label="撮影した名刺">
              {queue.map((i) => (
                <li key={i.key}>
                  <button className={"thumb" + (i.key === cur ? " current" : "") + ` ${i.status}`} onClick={() => setCur(i.key)} data-testid="queue-item" aria-label={`名刺 ${i.key}(${i.status})`}>
                    <img src={i.image} alt="" />
                    <span>{i.status === "confirmed" ? "確定" : i.status === "reading" ? "読取中" : i.status === "error" ? "失敗" : "確認待ち"}</span>
                  </button>
                </li>
              ))}
              <li><button className="thumb add" onClick={() => setCur(null)} data-testid="add-more">＋ 続けて撮影</button></li>
            </ol>
          )}
          {item === null ? (
            <Camera onCapture={addImage} onFiles={onFiles} />
          ) : (
            <div className="reviewing">
              <div className="photo"><img src={item.image} alt="撮影した名刺" data-testid="photo" /></div>
              <Review item={item} onEdit={edit(item.key)} onResetField={resetField(item.key)} onUndo={undo(item.key)}
                onEncounter={(enc) => patch(item.key, (i) => ({ ...i, enc }))} onConfirm={() => confirm(item)} onRedo={() => redo(item)} />
            </div>
          )}
        </main>
      )}
    </div>
  );
}
