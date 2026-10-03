import { useCallback, useEffect, useRef, useState } from "react";
import type { Api } from "./api";
import { ImportView } from "./ImportView";
import { ListView } from "./ListView";
import { ReviewView } from "./ReviewView";
import { SettingsView } from "./SettingsView";
import type { Progress } from "./types";

type Tab = "import" | "review" | "list" | "settings";

export function App({ api }: { api: Api }) {
  const [tab, setTab] = useState<Tab>("import");
  const [progress, setProgress] = useState<Progress | null>(null);
  const [drafts, setDrafts] = useState(0);
  const [version, setVersion] = useState(0); // 読み取りが進むたびに増やし、各画面に再読込させる
  const running = useRef(false);

  const refresh = useCallback(async () => {
    const [p, d] = await Promise.all([api.progress(), api.list({ text: "", status: "draft" })]);
    setProgress(p);
    setDrafts(d.length);
  }, [api]);

  /** 待ちの読み取りを実行し、終わるまで進捗を見る */
  const startJobs = useCallback(async () => {
    if (running.current) return;
    running.current = true;
    const timer = setInterval(() => { refresh().then(() => setVersion((v) => v + 1)).catch(() => undefined); }, 700);
    try {
      await api.runJobs();
    } finally {
      clearInterval(timer);
      running.current = false;
      await refresh();
      setVersion((v) => v + 1);
    }
  }, [api, refresh]);

  // 起動時: 前回の続き(待ちのジョブ)があれば再開する
  useEffect(() => {
    refresh().then(() => api.progress()).then((p) => { if (p.pending > 0) startJobs(); }).catch(() => undefined);
  }, [api, refresh, startJobs]);

  return (
    <div className="app">
      <header className="top">
        <div className="title">
          <h1>領収書DB(仮称)</h1>
          {api.kind === "mock" && <span className="mock">画面確認用のモック(実際の読み取りは行っていません)</span>}
        </div>
        <nav role="tablist">
          <button role="tab" aria-selected={tab === "import"} onClick={() => setTab("import")}>取り込み</button>
          <button role="tab" aria-selected={tab === "review"} onClick={() => setTab("review")}>確認{drafts ? `(${drafts})` : ""}</button>
          <button role="tab" aria-selected={tab === "list"} onClick={() => setTab("list")}>一覧・検索</button>
          <button role="tab" aria-selected={tab === "settings"} onClick={() => setTab("settings")}>設定</button>
        </nav>
      </header>
      <p className="disclaimer">読み取り結果は確認が前提です。税務上の判断や申告の責任は利用者にあります。手書きの書類は対象外です。</p>
      <main className="main">
        {tab === "import" && <ImportView api={api} progress={progress} onImported={startJobs} onRefresh={refresh} onGoReview={() => setTab("review")} />}
        {tab === "review" && <ReviewView api={api} version={version} onChanged={refresh} />}
        {tab === "list" && <ListView api={api} version={version} onChanged={refresh} />}
        {tab === "settings" && <SettingsView api={api} onDeleted={() => { refresh(); setVersion((v) => v + 1); }} />}
      </main>
    </div>
  );
}
