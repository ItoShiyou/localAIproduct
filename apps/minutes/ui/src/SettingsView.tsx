import { useEffect, useState } from "react";
import type { Api } from "./api";
import type { Flag, ModelInfo, Plan, SettingsInfo } from "./types";

const mb = (n: number) => `${Math.round(n / 1024 / 1024)}MB`;

/** 文字起こしのモデルの取得・削除(取得は利用者が押したときだけ通信する) */
function ModelSection({ api, onChanged, plan }: { api: Api; onChanged: () => void; plan?: Plan | null }) {
  const [m, setM] = useState<ModelInfo | null>(null);
  const [delAsk, setDelAsk] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  useEffect(() => { api.modelStatus().then(setM).catch((e) => setMsg(String(e))); }, [api]);
  useEffect(() => {
    if (!m?.downloading) return;
    const t = setInterval(() => api.modelStatus().then(setM).catch(() => undefined), 500);
    return () => clearInterval(t);
  }, [api, m?.downloading]);
  if (!m) return null;
  const start = async () => {
    setMsg(null);
    setM({ ...m, downloading: true, error: null });
    try { const r = await api.downloadModel(); setM(r); if (r.installed) { setMsg("取得しました。待ちの文字起こしがあれば始まります"); onChanged(); } }
    catch (e) { setMsg(String(e)); setM(await api.modelStatus()); }
  };
  return (
    <section className="card" data-testid="model">
      <h2>文字起こしのモデル</h2>
      {plan?.tier === "free" && (
        <p className="note" data-testid="model-tier">
          いまは<b>標準モデル(Whisper small)</b>で文字起こししています。下の高精度モデルは有料版で使えます。
        </p>
      )}
      <p className="note">高精度モデル: {m.name}・{mb(m.size)}。文字起こしはこのパソコンの中で行います。</p>
      {m.source === "env" && <p className="note">開発用の設定(環境変数)で指定されたモデルを使っています。</p>}
      {m.source === "bundled" && <p className="msg">アプリに同梱済み(追加の取得は不要です)</p>}
      {m.source === "managed" && <p className="msg">取得済み</p>}
      {m.source === "none" && <p className="note">アプリに同梱したモデルが見つかりません。インストールし直すか、ここから取得してください。</p>}
      {!m.installed && !m.downloading && (
        <>
          <p className="note">まだ取得していません。取得には約 {mb(m.size)} の通信と空き容量が要ります。送るのはモデルの名前(取得先のアドレス)だけで、録音や文字は送りません。{m.downloaded > 0 && ` 途中まで取得済み(${mb(m.downloaded)})なので、続きから再開します。`}</p>
          <button className="btn primary" onClick={start}>{m.downloaded > 0 ? "続きから取得する" : "取得する"}</button>
        </>
      )}
      {m.downloading && (
        <div className="progress">
          <div className="bar"><span style={{ width: `${(m.downloaded / m.size) * 100}%` }} /></div>
          <p className="note">取得中 {mb(m.downloaded)} / {mb(m.size)}</p>
          <button className="btn small" onClick={() => api.cancelModelDownload()}>中断</button>
        </div>
      )}
      {m.error && <p className="msg err">{m.error}</p>}
      {msg && <p className="msg">{msg}</p>}
      {m.installed && m.source === "managed" && (!delAsk
        ? <button className="btn small ghost" onClick={() => setDelAsk(true)}>モデルを削除…</button>
        : <span className="ask">削除すると、文字起こしには再取得が必要です。
            <button className="btn small danger" onClick={async () => { try { setM(await api.deleteModel()); } catch (e) { setMsg(String(e)); } setDelAsk(false); }}>削除する</button>
            <button className="btn small" onClick={() => setDelAsk(false)}>やめる</button>
          </span>)}
    </section>
  );
}

/** 無料版と有料版の比較(有料版の購入・ライセンスの有効化は準備中) */
function PlanCard({ plan }: { plan: Plan }) {
  const min = (ms: number | null) => (ms == null ? "" : `${Math.floor(ms / 60000)} 分`);
  const rows: [string, string, string][] = [
    ["文字起こしの精度", "標準(小さなモデル)", "高精度(大きなモデル)"],
    ["文字起こしできる時間", `累計 ${min(plan.totalLimitMs ?? 3_600_000)}・1件 ${min(plan.meetingLimitMs ?? 900_000)}まで`, "制限なし"],
    ["マイク録音・取り込み・確認と修正・検索", "○", "○"],
    ["書き出し", "テキストのみ(末尾に無料版の表示)", "Word・PDF・Markdown・テキスト・字幕・音声"],
    ["話者の判別・ノイズ除去・用語辞書", "—", "○"],
    ["要約(後から追加ダウンロード)", "—", "○(準備中)"],
  ];
  return (
    <section className="card" data-testid="plan-card">
      <h2>ご利用のプラン: {plan.tier === "pro" ? "有料版" : "無料版"}</h2>
      {plan.tier === "free" && (
        <p className="note">
          無料版はお試し用です。これまでに文字起こしした時間 {min(plan.usage.usedMs)} / {min(plan.totalLimitMs)}(議事録を消しても戻りません)。
          {plan.usage.tampered && " 使った量の記録が読めなかったため、上限に達した扱いになっています。"}
        </p>
      )}
      <table className="table">
        <thead><tr><th></th><th>無料版</th><th>有料版(買い切り)</th></tr></thead>
        <tbody>{rows.map(([a, b, c]) => <tr key={a}><td>{a}</td><td>{b}</td><td>{c}</td></tr>)}</tbody>
      </table>
      {plan.tier === "free" && <p className="note">有料版の購入とライセンスの有効化は準備中です。</p>}
    </section>
  );
}

export function SettingsView({ api, settings: s, plan, onSettings, onDeleted, onModelReady }: {
  api: Api; settings: SettingsInfo | null; plan?: Plan | null; onSettings: (s: SettingsInfo) => void; onDeleted: () => void; onModelReady: () => void;
}) {
  const [ask, setAsk] = useState(false);
  const [typed, setTyped] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [notices, setNotices] = useState<string | null>(null);
  if (!s) return <div className="pane">読み込み中…</div>;
  const set = async (k: Flag, on: boolean) => { try { onSettings(await api.setFlag(k, on)); } catch (e) { setMsg(String(e)); } };

  return (
    <div className="pane">
      {notices != null && (
        <div className="modal" role="dialog" aria-label="ライセンスの全文">
          <div className="modal-body wide">
            <h2>第三者のソフトウェアとモデルのライセンス</h2>
            <pre className="notices">{notices}</pre>
            <button className="btn" onClick={() => setNotices(null)}>閉じる</button>
          </div>
        </div>
      )}
      {plan && <PlanCard plan={plan} />}
      <ModelSection api={api} plan={plan} onChanged={() => { api.settings().then(onSettings); onModelReady(); }} />
      <section className="card">
        <h2>処理の設定</h2>
        <label className="check"><input type="checkbox" checked={s.denoiseDefault} onChange={(e) => set("denoise_default", e.target.checked)} /> 取り込むとき、ノイズ除去を既定でオンにする</label>
        <p className="note">ノイズ除去で文字起こしの精度が下がる録音もあります。結果が良くないときは、オフにして取り込み直してください。</p>
        <label className="check"><input type="checkbox" checked={s.keepAudio} onChange={(e) => set("keep_audio", e.target.checked)} /> 文字起こしの後も音声のコピーを残す(確認画面で再生するために必要)</label>
        <p className="note">オフにすると、文字起こしが終わった時点で音声のコピーを消し、文字だけを残します。</p>
      </section>

      <section className="card">
        <h2>通信一覧</h2>
        <p className="note">録音・文字起こしの内容を送る通信はありません。通信するのは次の場面だけです。</p>
        <table className="table">
          <thead><tr><th>目的</th><th>送信先</th><th>送る内容</th><th>状態</th></tr></thead>
          <tbody>
            {s.network.map((e) => (
              <tr key={e.purpose}><td>{e.purpose}</td><td>{e.destination}</td><td>{e.content}</td>
                <td>{e.stoppable ? <label><input type="checkbox" checked={e.enabled} onChange={(ev) => set("update_check", ev.target.checked)} /> {e.enabled ? "オン" : "オフ"}</label> : "止められません"}</td></tr>
            ))}
          </tbody>
        </table>
        <p className="note">ライセンスの有効化と更新の確認は、まだ接続していません(開発中)。</p>
      </section>

      <section className="card">
        <h2>録音の同意</h2>
        <p className="note">会議や面談を録音・文字起こしする前に、参加者の同意を得てください。同意を得る責任は利用者にあります。</p>
      </section>

      <section className="card">
        <h2>データの保存場所</h2>
        <p className="mono">{s.dataDir}</p>
        <p className="note">音声のコピー・議事録・用語辞書・設定はこのフォルダに保存します。アプリ内では暗号化していません。パソコンのディスク暗号化(macOS は FileVault、Windows は BitLocker)を有効にしておくことをおすすめします。</p>
      </section>

      <section className="card">
        <h2>使っているソフトウェアとモデル</h2>
        <table className="table">
          <thead><tr><th>名前</th><th>用途</th><th>ライセンス</th></tr></thead>
          <tbody>
            {[
              ["Whisper large-v3-turbo(OpenAI)/ whisper.cpp 形式", "文字起こしのモデル", "MIT"],
              ["whisper.cpp", "文字起こしの実行", "MIT"],
              ["whisper-rs", "whisper.cpp の Rust 束ね", "Unlicense"],
              ["nnnoiseless(RNNoise の移植)", "ノイズ除去", "BSD-3-Clause"],
              ["Symphonia", "音声ファイルの読み込み", "MPL-2.0"],
              ["SQLite / rusqlite", "データベース", "パブリックドメイン / MIT"],
              ["Tauri", "アプリの枠組み", "Apache-2.0 または MIT"],
              ["React", "画面", "MIT"],
            ].map(([n, u, l]) => <tr key={n}><td>{n}</td><td>{u}</td><td>{l}</td></tr>)}
          </tbody>
        </table>
        <p className="note">ここに挙げたもののほか、多数のオープンソースのライブラリを使っています。すべての著作権表示とライセンスの全文は、次のボタンから読めます(アプリにも同梱しています)。</p>
        <button className="btn small" onClick={async () => { try { setNotices(await api.thirdPartyNotices()); } catch (e) { setMsg(String(e)); } }}>ライセンスの全文を表示</button>
      </section>

      <section className="card danger-zone">
        <h2>全データを削除</h2>
        <p className="note">音声のコピー・議事録・用語辞書・設定と、取得した文字起こしのモデルをすべて消します。元に戻せません。取り込み元の録音ファイルは消えません。</p>
        {!ask ? <button className="btn danger" onClick={() => setAsk(true)}>全データを削除…</button> : (
          <div className="row">
            <input placeholder="「削除」と入力" value={typed} onChange={(e) => setTyped(e.target.value)} />
            <button className="btn danger" disabled={typed !== "削除"} onClick={async () => {
              try { await api.deleteAll(); setMsg("すべて削除しました"); setAsk(false); setTyped(""); onSettings(await api.settings()); onDeleted(); }
              catch (e) { setMsg(String(e)); }
            }}>削除する</button>
            <button className="btn" onClick={() => { setAsk(false); setTyped(""); }}>やめる</button>
          </div>
        )}
        {msg && <p className="note">{msg}</p>}
      </section>
    </div>
  );
}
