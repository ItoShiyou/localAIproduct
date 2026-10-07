import { useState } from "react";
import type { Language, Plan, ProcessOptions } from "./types";
import { LANGUAGE_LABEL, hms, parseTime } from "./types";

export const defaultOptions = (denoise = true): ProcessOptions => ({
  denoise, diarize: true, numSpeakers: null, language: "ja", rangeStartMs: null, rangeEndMs: null,
});

const fmt = (ms: number | null) => (ms == null ? "" : hms(ms));

/** 取り込み・やり直しのときの処理の設定(ノイズ除去・話者の判別・言語・範囲) */
export function OptionsForm({ value, onChange, diarizeAvailable = true, plan }: {
  value: ProcessOptions; onChange: (v: ProcessOptions) => void; diarizeAvailable?: boolean; plan?: Plan | null;
}) {
  const canDenoise = plan?.denoise ?? true;
  const canDiarize = (plan?.diarize ?? true) && diarizeAvailable;
  const [from, setFrom] = useState(fmt(value.rangeStartMs));
  const [to, setTo] = useState(fmt(value.rangeEndMs));
  const set = (p: Partial<ProcessOptions>) => onChange({ ...value, ...p });
  const fromMs = parseTime(from), toMs = parseTime(to);
  const bad = fromMs === -1 || toMs === -1 || (fromMs != null && toMs != null && toMs <= fromMs);
  return (
    <div className="opts" data-testid="options">
      {canDenoise && <label className="check"><input type="checkbox" checked={value.denoise} onChange={(e) => set({ denoise: e.target.checked })} /> ノイズ除去</label>}
      {(plan?.diarize ?? false) && <label className={"check" + (canDiarize ? "" : " locked")} title={diarizeAvailable ? "" : "話した人を判別するためのデータが見つかりません"}>
        <input type="checkbox" checked={value.diarize && canDiarize} disabled={!canDiarize} onChange={(e) => set({ diarize: e.target.checked })} /> 話者を判別{!(plan?.diarize ?? true) && <span className="pro">有料版</span>}
      </label>}
      {value.diarize && canDiarize && (
        <label>人数
          <select value={value.numSpeakers ?? ""} onChange={(e) => set({ numSpeakers: e.target.value ? Number(e.target.value) : null })}>
            <option value="">自動</option>
            {[2, 3, 4, 5, 6, 7, 8].map((n) => <option key={n} value={n}>{n}人</option>)}
          </select>
        </label>
      )}
      <label>言語
        <select value={value.language} onChange={(e) => set({ language: e.target.value as Language })}>
          {(Object.keys(LANGUAGE_LABEL) as Language[]).map((l) => <option key={l} value={l}>{LANGUAGE_LABEL[l]}</option>)}
        </select>
      </label>
      <label>範囲
        <input className="time-in" placeholder="最初" value={from} aria-label="範囲の始まり"
          onChange={(e) => { setFrom(e.target.value); const v = parseTime(e.target.value); if (v !== -1) set({ rangeStartMs: v }); }} />
        〜
        <input className="time-in" placeholder="最後" value={to} aria-label="範囲の終わり"
          onChange={(e) => { setTo(e.target.value); const v = parseTime(e.target.value); if (v !== -1) set({ rangeEndMs: v }); }} />
      </label>
      {bad && <span className="msg err">範囲は「分:秒」(例 12:30)で、終わりを始まりより後に</span>}
    </div>
  );
}

export const optionsValid = (o: ProcessOptions) => !(o.rangeStartMs != null && o.rangeEndMs != null && o.rangeEndMs <= o.rangeStartMs);
