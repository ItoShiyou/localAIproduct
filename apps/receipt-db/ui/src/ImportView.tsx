import { useRef, useState } from "react";
import type { Api } from "./api";
import type { ImportFile, ImportResult, Progress } from "./types";

const readAsDataUrl = (f: File) =>
  new Promise<string>((res, rej) => {
    const r = new FileReader();
    r.onload = () => res(String(r.result));
    r.onerror = () => rej(r.error);
    r.readAsDataURL(f);
  });

const ACCEPT = /\.(pdf|jpe?g|png)$/i;

/** ドロップされたフォルダを再帰的にたどってファイルを集める(webkitGetAsEntry) */
async function filesFromDrop(dt: DataTransfer): Promise<File[]> {
  const out: File[] = [];
  type Entry = { isFile: boolean; isDirectory: boolean; name: string; file?: (cb: (f: File) => void, err: (e: unknown) => void) => void; createReader?: () => { readEntries: (cb: (es: Entry[]) => void, err: (e: unknown) => void) => void } };
  const walk = async (e: Entry): Promise<void> => {
    if (e.name.startsWith(".")) return;
    if (e.isFile && e.file) {
      out.push(await new Promise<File>((res, rej) => e.file!(res, rej)));
    } else if (e.isDirectory && e.createReader) {
      const reader = e.createReader();
      for (;;) {
        const batch = await new Promise<Entry[]>((res, rej) => reader.readEntries(res, rej));
        if (!batch.length) break;
        for (const c of batch) await walk(c);
      }
    }
  };
  const entries = [...dt.items].map((i) => (i as unknown as { webkitGetAsEntry?: () => Entry | null }).webkitGetAsEntry?.() ?? null);
  if (entries.some((e) => e)) {
    for (const e of entries) if (e) await walk(e);
  } else {
    out.push(...dt.files);
  }
  return out;
}

export function ImportView({ api, progress, onImported, onRefresh, onGoReview }: {
  api: Api; progress: Progress | null; onImported: () => void; onRefresh: () => void; onGoReview: () => void;
}) {
  const [results, setResults] = useState<ImportResult[]>([]);
  const [busy, setBusy] = useState(false);
  const [over, setOver] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);
  const dirRef = useRef<HTMLInputElement>(null);

  const take = async (files: File[], fromFolder: boolean) => {
    // フォルダ内は対応拡張子だけ。直接選んだファイルは中身で判定して理由を出す
    const list = fromFolder ? files.filter((f) => ACCEPT.test(f.name)) : files;
    if (!list.length) return;
    setBusy(true);
    const out: ImportResult[] = [];
    try {
      // 1件ずつ送る(大きいPDFでメモリを使いすぎないため)。1件の失敗で止めない
      for (const f of list) {
        try {
          const item: ImportFile = { name: f.name, dataUrl: await readAsDataUrl(f) };
          out.push(...(await api.importFiles([item])));
        } catch (e) {
          out.push({ name: f.name, outcome: "failed", message: e instanceof Error ? e.message : String(e) });
        }
        setResults([...out]);
      }
    } finally {
      setBusy(false);
      onImported();
    }
  };

  const p = progress;
  const total = p ? p.pending + p.running + p.done + p.failed : 0;
  const imported = results.filter((r) => r.outcome === "imported").length;

  return (
    <div className="pane">
      <div
        className={"drop" + (over ? " over" : "")}
        onDragOver={(e) => { e.preventDefault(); setOver(true); }}
        onDragLeave={() => setOver(false)}
        onDrop={async (e) => {
          e.preventDefault(); setOver(false);
          const direct = e.dataTransfer.files.length; // await の後は dataTransfer が空になるので先に数える
          const fs = await filesFromDrop(e.dataTransfer);
          take(fs, fs.length !== direct);
        }}
      >
        <p className="drop-title">領収書・請求書(PDF・JPEG・PNG)をここにドラッグ</p>
        <p className="note">複数のファイルやフォルダもまとめて取り込めます。元のファイルは変更しません(アプリの中にコピーを保存します)。</p>
        <div className="row">
          <button className="btn primary" disabled={busy} onClick={() => fileRef.current?.click()}>ファイルを選ぶ</button>
          <button className="btn" disabled={busy} onClick={() => dirRef.current?.click()}>フォルダを選ぶ</button>
        </div>
        <input ref={fileRef} type="file" multiple accept=".pdf,.jpg,.jpeg,.png,application/pdf,image/jpeg,image/png" hidden
          onChange={(e) => { take([...(e.target.files ?? [])], false); e.target.value = ""; }} />
        <input ref={dirRef} type="file" multiple hidden {...{ webkitdirectory: "" }}
          onChange={(e) => { take([...(e.target.files ?? [])], true); e.target.value = ""; }} />
      </div>

      {p && total > 0 && (
        <section className="card">
          <h2>読み取りの進み具合</h2>
          <div className="bar" role="progressbar" aria-valuemin={0} aria-valuemax={total} aria-valuenow={p.done + p.failed}>
            <span style={{ width: `${total ? ((p.done + p.failed) / total) * 100 : 0}%` }} />
          </div>
          <p className="note">
            完了 {p.done} 件 / 失敗 {p.failed} 件 / 待ち {p.pending + p.running} 件
            {p.busy ? "(読み取り中。ほかの画面を使っていても続きます)" : p.pending ? "(止まっています)" : ""}
          </p>
          <div className="row">
            {p.busy && <button className="btn" onClick={() => api.cancelJobs().then(onRefresh)}>中断</button>}
            {!p.busy && p.pending > 0 && <button className="btn" onClick={onImported}>再開</button>}
            {!p.busy && p.failed > 0 && <button className="btn" onClick={() => api.retryFailed().then(onImported)}>失敗したものをやり直す</button>}
            <button className="btn secondary" onClick={onGoReview}>確認画面へ</button>
          </div>
          {p.errors.length > 0 && (
            <details>
              <summary>失敗した理由({p.errors.length})</summary>
              <ul className="errors">{p.errors.map((e, i) => <li key={i}>{e}</li>)}</ul>
            </details>
          )}
        </section>
      )}

      {results.length > 0 && (
        <section className="card">
          <h2>取り込み結果({imported} 件を追加)</h2>
          <ul className="results">
            {results.map((r, i) => (
              <li key={i} className={r.outcome}>
                <span className="tag">{r.outcome === "imported" ? "追加" : r.outcome === "duplicate" ? "重複" : "失敗"}</span>
                <span className="name">{r.name}</span>
                {r.message && <span className="msg">{r.message}</span>}
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}
