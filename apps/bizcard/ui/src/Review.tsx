import { useState } from "react";
import type { Encounter, FieldKey, Fields } from "./types";
import { FIELD_KEYS, FIELD_LABELS, REVIEW_THRESHOLD } from "./types";
import type { QueueItem } from "./App";

interface Props {
  item: QueueItem;
  onEdit: (key: FieldKey, value: string) => void;
  onResetField: (key: FieldKey) => void;
  onUndo: () => void;
  onEncounter: (e: Encounter) => void;
  onConfirm: () => void;
  onRedo: () => void;
}

const today = () => new Date().toISOString().slice(0, 10);
export const emptyEncounter = (): Encounter => ({ metOn: today(), place: "", howMet: "", memo: "" });

/** 画面下部の確認枠。項目ごとに、読み取り結果をその場で打ち替えられる。 */
export function Review({ item, onEdit, onResetField, onUndo, onEncounter, onConfirm, onRedo }: Props) {
  const [open, setOpen] = useState(true);
  const read = item.read;
  if (item.status === "reading") return <section className="sheet" data-testid="sheet"><p className="reading" data-testid="reading">読み取っています…</p></section>;
  if (item.status === "error") return <section className="sheet"><p className="err">{item.error}</p><button className="btn secondary" onClick={onRedo}>やり直し</button></section>;
  if (!read) return null;
  const done = item.status === "confirmed";
  const changed = (k: FieldKey) => item.fields[k] !== read.fields[k];

  return (
    <section className="sheet" data-testid="sheet" aria-label="読み取り結果の確認">
      <header>
        <h2>読み取り結果{done ? "(確定済み)" : ""}</h2>
        <p className="note">読み取り結果は確認が前提です。違っていれば、その場で打ち替えてください。</p>
      </header>
      {read.duplicates.length > 0 && (
        <div className="dup" role="note" data-testid="dup">
          <b>同じ人かもしれません</b>
          {read.duplicates.map((d) => (
            <div key={d.existingId}>
              {d.name}({d.company}) — {d.reasons.map((r) => (r === "sameEmail" ? "メールが同じ" : "氏名と会社が同じ")).join("・")}
            </div>
          ))}
          <small>自動では統合しません。別の人として確定できます。</small>
        </div>
      )}
      <div className="fields">
        {FIELD_KEYS.map((k) => {
          const conf = read.confidence[k];
          const found = read.fields[k] !== "";
          const low = found && conf !== undefined && conf < REVIEW_THRESHOLD;
          return (
            <div key={k} className={"field" + (low ? " low" : "") + (changed(k) ? " edited" : "")} data-testid={`field-${k}`}>
              <label htmlFor={`f-${k}`}>
                {FIELD_LABELS[k]}
                {low && <span className="badge" data-testid={`low-${k}`}>要確認</span>}
                {!found && !changed(k) && <span className="badge none">未検出</span>}
                {changed(k) && (
                  <button type="button" className="link" onClick={() => onResetField(k)} aria-label={`${FIELD_LABELS[k]}を読み取り結果に戻す`}>
                    ↺ 戻す
                  </button>
                )}
              </label>
              <input id={`f-${k}`} value={item.fields[k]} disabled={done} inputMode={k === "email" ? "email" : k === "phone" || k === "mobile" || k === "postalCode" ? "tel" : "text"}
                autoComplete="off" onChange={(e) => onEdit(k, e.target.value)} />
              {k === "company" && !done && (() => {
                const cands = read.companyCandidates.filter((c) => c.value !== item.fields.company);
                return cands.length > 0 ? (
                  <div className="extra" data-testid="company-candidates">
                    会社名の候補(タップで選択、自由入力もできます):
                    <div className="chips">
                      {cands.map((c) => (
                        <button key={c.value} type="button" className="chip" data-testid="company-candidate" onClick={() => onEdit("company", c.value)}>
                          {c.value}<small>{Math.round(c.confidence * 100)}%</small>
                        </button>
                      ))}
                    </div>
                  </div>
                ) : null;
              })()}
              {k === "email" && read.extraEmails.length > 0 && !done && (
                <div className="extra">他のメール: {read.extraEmails.map((m) => (
                  <button key={m} type="button" className="chip" onClick={() => onEdit("email", m)}>{m}</button>
                ))}</div>
              )}
            </div>
          );
        })}
      </div>
      <details open={open} onToggle={(e) => setOpen((e.target as HTMLDetailsElement).open)} className="enc" data-testid="encounter">
        <summary>あとで思い出すための記録(空欄のままでも確定できます)</summary>
        <div className="grid2">
          <label>会った日<input type="date" value={item.enc.metOn} disabled={done} onChange={(e) => onEncounter({ ...item.enc, metOn: e.target.value })} data-testid="enc-metOn" /></label>
          <label>場所<input value={item.enc.place} disabled={done} onChange={(e) => onEncounter({ ...item.enc, place: e.target.value })} data-testid="enc-place" /></label>
        </div>
        <label>きっかけ<input value={item.enc.howMet} disabled={done} onChange={(e) => onEncounter({ ...item.enc, howMet: e.target.value })} data-testid="enc-howMet" /></label>
        <label>メモ<textarea rows={3} value={item.enc.memo} disabled={done} onChange={(e) => onEncounter({ ...item.enc, memo: e.target.value })} data-testid="enc-memo" /></label>
      </details>
      {!done && (
        <footer className="bar">
          <button className="btn ghost" onClick={onRedo} data-testid="redo">やり直し</button>
          <button className="btn secondary" onClick={onUndo} disabled={item.history.length === 0} data-testid="undo">元に戻す</button>
          <button className="btn primary" onClick={onConfirm} data-testid="confirm">確定</button>
        </footer>
      )}
    </section>
  );
}

export type { Fields };
