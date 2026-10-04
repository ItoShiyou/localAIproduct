import { useCallback, useEffect, useRef, useState } from "react";
import type { Api } from "./api";
import { DetailView } from "./DetailView";
import { GlossaryView } from "./GlossaryView";
import { OptionsForm, defaultOptions, optionsValid } from "./OptionsForm";
import { Recorder } from "./Recorder";
import { SearchView } from "./SearchView";
import { SettingsView } from "./SettingsView";
import type { ImportResult, Meeting, MeetingFilter, Plan, ProcessOptions, Progress, SettingsInfo } from "./types";
import { STATE_LABEL, hms } from "./types";

type Tab = "meetings" | "search" | "glossary" | "settings";

export function App({ api }: { api: Api }) {
  const [tab, setTab] = useState<Tab>("meetings");
  const [meetings, setMeetings] = useState<Meeting[]>([]);
  const [cur, setCur] = useState<number | null>(null);
  const [seekTo, setSeekTo] = useState<{ ms: number; n: number } | null>(null);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [settings, setSettings] = useState<SettingsInfo | null>(null);
  const [opts, setOpts] = useState<ProcessOptions>(defaultOptions());
  const [showOpts, setShowOpts] = useState(false);
  const [filter, setFilter] = useState<MeetingFilter>({ tag: null, sort: "held_desc" });
  const [tags, setTags] = useState<[string, number][]>([]);
  const [recordOpts, setRecordOpts] = useState<ProcessOptions | null>(null);
  const [plan, setPlan] = useState<Plan | null>(null);
  const [results, setResults] = useState<ImportResult[]>([]);
  const [askConsent, setAskConsent] = useState<null | (() => void)>(null);
  const [version, setVersion] = useState(0);
  const [jobError, setJobError] = useState<string | null>(null);
  const running = useRef(false);

  const refresh = useCallback(async () => {
    const [ms, p, tg, pl] = await Promise.all([api.meetings(filter), api.progress(), api.allTags(), api.plan()]);
    setPlan(pl);
    setMeetings(ms);
    setProgress(p);
    setTags(tg);
    // 何も選んでいなければ、いちばん新しい議事録を開く
    setCur((c) => (c != null && ms.some((m) => m.id === c) ? c : ms[0]?.id ?? null));
  }, [api, filter]);

  const startJobs = useCallback(async () => {
    if (running.current) return;
    running.current = true;
    const timer = setInterval(() => { refresh().catch(() => undefined); }, 800);
    try {
      setJobError(null);
      await api.runJobs().catch((e) => setJobError(String(e)));
      // 画面を開き直したときなど、すでに裏で処理中なら(runJobs はすぐ戻る)、終わるまで進み具合を見続ける
      while ((await api.progress()).busy) await new Promise((r) => setTimeout(r, 1000));
    } finally {
      clearInterval(timer);
      running.current = false;
      await refresh();
      setVersion((v) => v + 1);
    }
  }, [api, refresh]);

  useEffect(() => {
    api.settings().then((s) => { setSettings(s); setOpts((o) => ({ ...o, denoise: s.denoiseDefault, diarize: o.diarize && s.diarizeAvailable })); }).catch(() => undefined);
    api.progress().then((p) => { if (p.pending > 0) startJobs(); }).catch(() => undefined);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [api]);
  useEffect(() => { refresh().catch(() => undefined); }, [refresh]);

  /** 初回だけ、録音の同意についての注意を出してから取り込む */
  const withConsent = useCallback((go: () => void) => {
    if (settings && !settings.consentShown) setAskConsent(() => go);
    else go();
  }, [settings]);

  const afterImport = useCallback((rs: ImportResult[]) => {
    if (!rs.length) return;
    setResults(rs);
    const first = rs.find((r) => r.id != null);
    if (first?.id != null) setCur(first.id);
    refresh().then(startJobs);
  }, [refresh, startJobs]);

  const pick = () => {
    if (!optionsValid(opts)) { setShowOpts(true); return; }
    withConsent(() => { api.pickAndImport(opts).then(afterImport).catch((e) => setResults([{ name: "", id: null, error: String(e) }])); });
  };

  // ドラッグ&ドロップ(Tauri はファイルの場所を通知する)
  const optsRef = useRef(opts);
  optsRef.current = opts;
  useEffect(() => {
    let off: (() => void) | undefined;
    api.onDrop((paths) => { if (paths.length) withConsent(() => { api.importPaths(paths, optsRef.current).then(afterImport); }); }).then((f) => { off = f; });
    return () => off?.();
  }, [api, withConsent, afterImport]);

  const openAt = (meetingId: number, ms: number) => {
    setTab("meetings");
    setCur(meetingId);
    setSeekTo({ ms, n: Date.now() });
  };

  return (
    <div className="app">
      <header className="top">
        <div className="title">
          <h1>議事録(仮称)</h1>
          {api.kind === "mock" && <span className="mock">画面確認用のモック(実際の文字起こしは行っていません)</span>}
        </div>
        <nav role="tablist">
          <button role="tab" aria-selected={tab === "meetings"} onClick={() => setTab("meetings")}>議事録</button>
          <button role="tab" aria-selected={tab === "search"} onClick={() => setTab("search")}>検索</button>
          <button role="tab" aria-selected={tab === "glossary"} onClick={() => setTab("glossary")}>用語辞書</button>
          <button role="tab" aria-selected={tab === "settings"} onClick={() => setTab("settings")}>設定</button>
        </nav>
      </header>
      <p className="disclaimer">文字起こしには誤りが含まれます。重要な箇所は音声で確認してください。録音の同意を得る責任は利用者にあります。</p>

      {askConsent && (
        <div className="modal" role="dialog" aria-label="録音の同意について">
          <div className="modal-body">
            <h2>録音の同意について</h2>
            <p>会議や面談を録音・文字起こしする前に、参加者の同意を得てください。同意を得る責任は利用者にあります。</p>
            <p className="note">録音はこのパソコンの中で処理し、外部に送信しません。</p>
            <div className="row">
              <button className="btn primary" onClick={async () => { const s = await api.setFlag("consent_shown", true); setSettings(s); const go = askConsent; setAskConsent(null); go(); }}>確認しました</button>
              <button className="btn" onClick={() => setAskConsent(null)}>やめる</button>
            </div>
          </div>
        </div>
      )}

      <main className="main">
        {tab === "meetings" && (
          <div className="split">
            <aside className="side">
              <div className="side-head">
                <button className="btn primary" onClick={pick}>録音を取り込む</button>
                <button className="btn rec" disabled={!!recordOpts} onClick={() => withConsent(() => setRecordOpts({ ...opts, rangeStartMs: null, rangeEndMs: null }))}>● 録音する</button>
                <button className="link" onClick={() => setShowOpts((v) => !v)} aria-expanded={showOpts}>取り込みの設定 {showOpts ? "▲" : "▼"}</button>
              </div>
              {showOpts
                ? <div className="pad"><OptionsForm value={opts} onChange={setOpts} diarizeAvailable={settings?.diarizeAvailable ?? true} plan={plan} /></div>
                : <p className="note pad">{[opts.denoise && (plan?.denoise ?? true) && "ノイズ除去", opts.diarize && (plan?.diarize ?? true) && (settings?.diarizeAvailable ?? true) && `話者の判別(${opts.numSpeakers ? `${opts.numSpeakers}人` : "人数は自動"})`, opts.language !== "ja" && (opts.language === "en" ? "英語" : "言語は自動判定"), (opts.rangeStartMs != null || opts.rangeEndMs != null) && "範囲指定あり"].filter(Boolean).join("・") || "設定なし"}</p>}
              <p className="note pad">m4a / mp3 / wav / mp4 など。ここにドラッグしても取り込めます。元のファイルは変更しません。</p>
              {plan?.tier === "free" && (
                <div className="pad plan-strip" data-testid="plan-strip">
                  <span className="pro">無料版</span>
                  <span>残り {Math.floor((plan.remainingMs ?? 0) / 60000)} 分(累計 {Math.floor((plan.totalLimitMs ?? 0) / 60000)} 分まで・1件 {Math.floor((plan.meetingLimitMs ?? 0) / 60000)} 分まで)</span>
                  <button className="link" onClick={() => setTab("settings")}>有料版について</button>
                </div>
              )}
              {recordOpts && (
                <div className="pad">
                  <Recorder api={api} opts={recordOpts} limitMs={plan?.meetingLimitMs ?? null}
                    onStarted={(id) => { setCur(id); refresh(); }}
                    onStopped={(id) => { setRecordOpts(null); setCur(id); refresh().then(startJobs); }}
                    onCancel={() => { setRecordOpts(null); refresh(); }} />
                </div>
              )}
              {progress && (progress.busy || progress.pending > 0) && (
                <div className="progress pad">
                  <div className="bar"><span style={{ width: `${progress.totalChunks ? (progress.doneChunks / progress.totalChunks) * 100 : 0}%` }} /></div>
                  <p className="note">
                    {progress.busy ? `処理中 ${progress.doneChunks}/${progress.totalChunks} 区間` : "止まっています"}・待ち {progress.pending} 件
                  </p>
                  <div className="row">
                    {progress.busy ? <button className="btn small" onClick={() => api.cancelJobs()}>中断</button> : <button className="btn small" onClick={startJobs}>再開</button>}
                  </div>
                </div>
              )}
              {jobError && (
                <div className="pad" data-testid="job-error">
                  <p className="msg err">{jobError}</p>
                  {/モデル/.test(jobError) && <button className="btn small" onClick={() => setTab("settings")}>設定を開く</button>}
                </div>
              )}
              {results.some((r) => r.error) && (
                <ul className="errors pad">{results.filter((r) => r.error).map((r, i) => <li key={i}>{r.name}: {r.error}</li>)}</ul>
              )}
              <div className="list-tools pad">
                <select aria-label="タグで絞り込み" value={filter.tag ?? ""} onChange={(e) => setFilter({ ...filter, tag: e.target.value || null })}>
                  <option value="">すべてのタグ</option>
                  {tags.map(([t, n]) => <option key={t} value={t}>{t}({n})</option>)}
                </select>
                <select aria-label="並べ替え" value={filter.sort ?? "held_desc"} onChange={(e) => setFilter({ ...filter, sort: e.target.value as MeetingFilter["sort"] })}>
                  <option value="held_desc">日付の新しい順</option>
                  <option value="held_asc">日付の古い順</option>
                  <option value="created_desc">取り込んだ順</option>
                  <option value="title">タイトル順</option>
                </select>
              </div>
              <ul className="items">
                {meetings.map((m) => (
                  <li key={m.id}>
                    <button className={m.id === cur ? "current" : ""} onClick={() => setCur(m.id)}>
                      <span className="v">{m.title}</span>
                      <span className="d">{m.heldOn ?? "日付なし"}{m.durationMs ? ` ・ ${hms(m.durationMs)}` : ""}</span>
                      <span className={`badge s-${m.state}`}>{STATE_LABEL[m.state]}</span>
                      {m.status === "confirmed" && <span className="badge ok">確定</span>}
                      {m.tags.map((t) => <span key={t} className="tag-chip">{t}</span>)}
                    </button>
                  </li>
                ))}
              </ul>
              {!meetings.length && <p className="note pad">まだ議事録はありません。</p>}
            </aside>
            <section className="content">
              {cur != null && meetings.some((m) => m.id === cur)
                ? <DetailView key={cur} api={api} id={cur} version={version} seekTo={seekTo} settings={settings} plan={plan} onChanged={refresh} onReprocessed={() => refresh().then(startJobs)} onDeleted={() => { setCur(null); refresh(); }} onRetry={() => api.retry(cur).then(() => refresh()).then(startJobs)} />
                : <div className="empty">左の一覧から議事録を選ぶか、録音を取り込んでください。</div>}
            </section>
          </div>
        )}
        {tab === "search" && <SearchView api={api} onOpen={openAt} />}
        {tab === "glossary" && <GlossaryView api={api} plan={plan} />}
        {tab === "settings" && <SettingsView api={api} settings={settings} plan={plan} onSettings={(s) => { setSettings(s); }} onDeleted={() => { setCur(null); refresh(); }} onModelReady={() => { refresh().then(startJobs); }} />}
      </main>
    </div>
  );
}
