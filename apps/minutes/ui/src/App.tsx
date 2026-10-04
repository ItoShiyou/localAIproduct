import { useCallback, useEffect, useRef, useState } from "react";
import type { Api } from "./api";
import { DetailView } from "./DetailView";
import { GlossaryView } from "./GlossaryView";
import { SearchView } from "./SearchView";
import { SettingsView } from "./SettingsView";
import type { ImportResult, Meeting, Progress, SettingsInfo } from "./types";
import { STATE_LABEL, hms } from "./types";

type Tab = "meetings" | "search" | "glossary" | "settings";

export function App({ api }: { api: Api }) {
  const [tab, setTab] = useState<Tab>("meetings");
  const [meetings, setMeetings] = useState<Meeting[]>([]);
  const [cur, setCur] = useState<number | null>(null);
  const [seekTo, setSeekTo] = useState<{ ms: number; n: number } | null>(null);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [settings, setSettings] = useState<SettingsInfo | null>(null);
  const [denoise, setDenoise] = useState(true);
  const [results, setResults] = useState<ImportResult[]>([]);
  const [askConsent, setAskConsent] = useState<null | (() => void)>(null);
  const [version, setVersion] = useState(0);
  const [jobError, setJobError] = useState<string | null>(null);
  const running = useRef(false);

  const refresh = useCallback(async () => {
    const [ms, p] = await Promise.all([api.meetings(), api.progress()]);
    setMeetings(ms);
    setProgress(p);
    // 何も選んでいなければ、いちばん新しい議事録を開く
    setCur((c) => (c != null && ms.some((m) => m.id === c) ? c : ms[0]?.id ?? null));
  }, [api]);

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
    api.settings().then((s) => { setSettings(s); setDenoise(s.denoiseDefault); }).catch(() => undefined);
    refresh().then(() => api.progress()).then((p) => { if (p.pending > 0) startJobs(); }).catch(() => undefined);
  }, [api, refresh, startJobs]);

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

  const pick = () => withConsent(() => { api.pickAndImport(denoise).then(afterImport).catch((e) => setResults([{ name: "", id: null, error: String(e) }])); });

  // ドラッグ&ドロップ(Tauri はファイルの場所を通知する)
  const denoiseRef = useRef(denoise);
  denoiseRef.current = denoise;
  useEffect(() => {
    let off: (() => void) | undefined;
    api.onDrop((paths) => { if (paths.length) withConsent(() => { api.importPaths(paths, denoiseRef.current).then(afterImport); }); }).then((f) => { off = f; });
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
                <label className="check"><input type="checkbox" checked={denoise} onChange={(e) => setDenoise(e.target.checked)} /> ノイズ除去</label>
              </div>
              <p className="note pad">m4a / mp3 / wav / mp4 など。ここにドラッグしても取り込めます。元のファイルは変更しません。</p>
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
              <ul className="items">
                {meetings.map((m) => (
                  <li key={m.id}>
                    <button className={m.id === cur ? "current" : ""} onClick={() => setCur(m.id)}>
                      <span className="v">{m.title}</span>
                      <span className="d">{m.heldOn ?? "日付なし"}{m.durationMs ? ` ・ ${hms(m.durationMs)}` : ""}</span>
                      <span className={`badge s-${m.state}`}>{STATE_LABEL[m.state]}</span>
                      {m.status === "confirmed" && <span className="badge ok">確定</span>}
                    </button>
                  </li>
                ))}
              </ul>
              {!meetings.length && <p className="note pad">まだ議事録はありません。</p>}
            </aside>
            <section className="content">
              {cur != null && meetings.some((m) => m.id === cur)
                ? <DetailView key={cur} api={api} id={cur} version={version} seekTo={seekTo} onChanged={refresh} onDeleted={() => { setCur(null); refresh(); }} onRetry={() => api.retry(cur).then(() => refresh()).then(startJobs)} />
                : <div className="empty">左の一覧から議事録を選ぶか、録音を取り込んでください。</div>}
            </section>
          </div>
        )}
        {tab === "search" && <SearchView api={api} onOpen={openAt} />}
        {tab === "glossary" && <GlossaryView api={api} />}
        {tab === "settings" && <SettingsView api={api} settings={settings} onSettings={(s) => { setSettings(s); }} onDeleted={() => { setCur(null); refresh(); }} onModelReady={() => { refresh().then(startJobs); }} />}
      </main>
    </div>
  );
}
