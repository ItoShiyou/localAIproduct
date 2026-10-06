import { useEffect, useState } from "react";
import type { Api } from "./api";
import type { Flag, LicenseStatus, ModelInfo, Plan, SettingsInfo, SummaryStatus } from "./types";
import { PRO_LABEL } from "./types";
import { PageHead } from "./PageHead";

const mb = (n: number) => `${Math.round(n / 1024 / 1024)}MB`;
const gb = (n: number) => `${(n / 1024 / 1024 / 1024).toFixed(1)}GB`;

/** 文字起こしのモデルの取得・削除(取得は利用者が押したときだけ通信する) */
function ModelSection({ api, onChanged, plan, offline }: { api: Api; onChanged: () => void; plan?: Plan | null; offline: boolean }) {
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
      <h2>文字起こしの準備</h2>
      {plan?.tier === "free" && (
        <p className="note" data-testid="model-tier">
          いまは<b>標準の文字起こし</b>を使っています。より精度の高い文字起こしは有料版で使えます。
        </p>
      )}
      <p className="note">高精度の文字起こしに必要な容量は約 {mb(m.size)} です。音声はこのパソコンの中で処理します。</p>
      {m.source === "bundled" && <p className="msg">アプリに同梱済み(追加の取得は不要です)</p>}
      {m.source === "managed" && <p className="msg">取得済み</p>}
      {m.source === "none" && <p className="note">文字起こしに必要なデータが見つかりません。アプリを入れ直すか、ここからダウンロードしてください。</p>}
      {!m.installed && !m.downloading && (
        <>
          <p className="note">まだ取得していません。取得には約 {mb(m.size)} の通信と空き容量が要ります。ダウンロード先と通信しますが、録音や会話の内容は送りません。{m.downloaded > 0 && ` 途中まで取得済み(${mb(m.downloaded)})なので、続きから再開します。`}</p>
          {offline && <p className="note">ネットワークを使わない設定のため、取得できません。</p>}
          <button className="btn primary" disabled={offline} onClick={start}>{m.downloaded > 0 ? "続きから取得する" : "取得する"}</button>
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
        ? <button className="btn small ghost" onClick={() => setDelAsk(true)}>追加データを削除…</button>
        : <span className="ask">削除すると、文字起こしには再取得が必要です。
            <button className="btn small danger" onClick={async () => { try { setM(await api.deleteModel()); } catch (e) { setMsg(String(e)); } setDelAsk(false); }}>削除する</button>
            <button className="btn small" onClick={() => setDelAsk(false)}>やめる</button>
          </span>)}
    </section>
  );
}

/** 要約(追加機能)のモデルの取得・削除。有料版のみ。取得は利用者が押したときだけ通信する */
function SummarySection({ api, plan, offline }: { api: Api; plan?: Plan | null; offline: boolean }) {
  const [m, setM] = useState<SummaryStatus | null>(null);
  const [delAsk, setDelAsk] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  useEffect(() => { api.summaryStatus().then(setM).catch((e) => setMsg(String(e))); }, [api]);
  useEffect(() => {
    if (!m?.downloading) return;
    const t = setInterval(() => api.summaryStatus().then(setM).catch(() => undefined), 500);
    return () => clearInterval(t);
  }, [api, m?.downloading]);
  const [importing, setImporting] = useState(false);
  if (!m) return null;
  const pro = plan?.summary ?? false;
  const busy = m.downloading || importing;
  // ファイルから取り込む(通信しない)。終わるまで返らない。進み具合は summaryStatus を見に行く
  const importFile = async () => {
    setMsg(null);
    setImporting(true);
    try {
      const r = await api.pickAndImportSummaryModel();
      if (r) { setM(r); if (r.installed) setMsg("取り込みました。議事録の「要約」タブから使えます"); }
    } catch (e) { setMsg(String(e).replace(/^Error: /, "")); }
    finally { setImporting(false); setM(await api.summaryStatus()); }
  };
  const start = async () => {
    setMsg(null);
    setM({ ...m, downloading: true, error: null });
    try { const r = await api.downloadSummaryModel(); setM(r); if (r.installed) setMsg("取得しました。議事録の「要約」タブから使えます"); }
    catch (e) { setMsg(String(e).replace(/^Error: /, "")); setM(await api.summaryStatus()); }
  };
  return (
    <section className="card" data-testid="summary-model">
      <h2>要約(追加機能){!pro && <span className="pro">{PRO_LABEL}</span>}</h2>
      <p className="note">会話から要点・決定事項・やることの下書きを作ります。初めて使うときは、必要なデータを追加ダウンロードしてください。</p>
      <p className="note">必要な空き容量は約 {gb(m.size)} です。メモリ16GBのパソコンで動作確認しています。長い会議では数分かかることがあります。</p>
      {!pro && <p className="note" data-testid="summary-model-locked">有料版の機能です。無料版では取得できません。</p>}
      {m.installed && m.source === "managed" && <p className="msg">入っています</p>}
      {!m.engine && <p className="note">この版では要約を利用できません。</p>}
      {!m.installed && !busy && (
        <>
          <p className="note">まだ入っていません。取得するには約 {gb(m.size)} の通信と空き容量が要ります。ダウンロード時だけ通信します。録音や会話の内容は送りません。{m.downloaded > 0 && ` 途中まで取得済み(${mb(m.downloaded)})なので、続きから再開します。`}</p>
          {offline && <p className="note" data-testid="summary-model-offline">ネットワークを使わない設定のため、取得できません。ファイルから取り込んでください。</p>}
          <div className="row">
            <button className="btn primary" data-testid="summary-model-get" disabled={!pro || offline} onClick={start}>{m.downloaded > 0 ? "続きから取得する" : "取得する"}</button>
            <button className="btn" data-testid="summary-model-import" disabled={!pro} onClick={importFile}>ファイルから取り込む</button>
          </div>
          <p className="note">インターネットにつながらないパソコンでは、必要なファイルをUSBなどで持ち込み、「ファイルから取り込む」で選べます。</p>
          <a href="#third-party-licenses">対応ファイルとライセンスを確認する</a>
        </>
      )}
      {busy && (
        <div className="progress" data-testid="summary-model-progress">
          <div className="bar"><span style={{ width: `${(m.downloaded / m.size) * 100}%` }} /></div>
          <p className="note">{importing ? "取り込み中" : "取得中"} {mb(m.downloaded)} / {mb(m.size)}</p>
          {!importing && <button className="btn small" onClick={() => api.cancelSummaryDownload()}>中断</button>}
        </div>
      )}
      {m.error && <p className="msg err">{m.error}</p>}
      {msg && <p className="msg">{msg}</p>}
      {m.installed && m.source === "managed" && (!delAsk
        ? <button className="btn small ghost" onClick={() => setDelAsk(true)}>追加データを削除…</button>
        : <span className="ask">削除すると、要約には再取得が必要です。
            <button className="btn small danger" onClick={async () => { try { setM(await api.deleteSummaryModel()); } catch (e) { setMsg(String(e)); } setDelAsk(false); }}>削除する</button>
            <button className="btn small" onClick={() => setDelAsk(false)}>やめる</button>
          </span>)}
    </section>
  );
}

/** 無料版と有料版の比較 */
function PlanCard({ plan }: { plan: Plan }) {
  const min = (ms: number | null) => (ms == null ? "" : `${Math.floor(ms / 60000)} 分`);
  const rows: [string, string, string][] = [
    ["文字起こしの精度", "標準", "高精度"],
    ["文字起こしできる時間", `累計 ${min(plan.totalLimitMs ?? 3_600_000)}・1件 ${min(plan.meetingLimitMs ?? 900_000)}まで`, "制限なし"],
    ["マイク録音・取り込み・確認と修正・検索", "○", "○"],
    ["書き出し", "テキストのみ(末尾に無料版の表示)", "Word・PDF・Markdown・テキスト・字幕・音声"],
    ["話者の判別・ノイズ除去・用語辞書", "—", "○"],
    ["要約(後から追加ダウンロード。下書きを確認して使う)", "—", "○"],
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
      {plan.tier === "free" && <p className="note">ライセンスキーをお持ちの方は、すぐ下の「ライセンス」に入力すると有料版になります。購入の方法は準備中です。</p>}
    </section>
  );
}

/** ライセンス(オフラインで検証する。通信しない)。キーを貼る、またはファイルから登録する。端末コードは、端末に固定したライセンスを頼むときに伝える */
function LicenseSection({ api, plan, onChanged }: { api: Api; plan?: Plan | null; onChanged: () => void }) {
  const [st, setSt] = useState<LicenseStatus | null>(null);
  const [text, setText] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [ok, setOk] = useState<string | null>(null);
  const [ask, setAsk] = useState(false);
  const [copied, setCopied] = useState(false);
  const [busy, setBusy] = useState(false);
  useEffect(() => { api.licenseStatus().then(setSt).catch((e) => setErr(String(e).replace(/^Error: /, ""))); }, [api]);
  if (!st) return null;
  const apply = (r: LicenseStatus) => { setSt(r); setText(""); setAsk(false); setOk(`${r.info?.licensee ?? ""} 様のライセンスを登録しました。有料版になりました`); onChanged(); };
  const run = async (f: () => Promise<LicenseStatus | null>) => {
    setErr(null); setOk(null); setBusy(true);
    try { const r = await f(); if (r) apply(r); }
    catch (e) { setErr(String(e).replace(/^Error: /, "")); }
    finally { setBusy(false); }
  };
  const remove = async () => {
    setErr(null); setOk(null); setAsk(false);
    try { const r = await api.removeLicense(); setSt(r); setOk("ライセンスを外しました。無料版に戻りました"); onChanged(); }
    catch (e) { setErr(String(e).replace(/^Error: /, "")); }
  };
  const copy = async () => {
    if (!st.machineCode) return;
    try { await navigator.clipboard.writeText(st.machineCode); setCopied(true); setTimeout(() => setCopied(false), 2000); }
    catch { setErr("コピーできませんでした。表示されている端末コードを、手で選んでコピーしてください"); }
  };
  const i = st.info;
  const pro = (plan?.tier ?? st.tier) === "pro";
  return (
    <section className="card" data-testid="license">
      <h2>ライセンス</h2>
      <div className="lic-status" data-testid="license-status">
        <span className="pro lic-tier">{!pro ? "無料版" : i?.offline ? "有料版・オフライン版" : "有料版"}</span>
        {i ? (
          <dl className="lic-info">
            <dt>登録名</dt><dd data-testid="license-licensee">{i.licensee} 様</dd>
            <dt>版</dt><dd>{i.offline ? "オフライン版(ネットワークを使う機能を止めています)" : "有料版"}</dd>
            <dt>発行日</dt><dd>{i.issued}</dd>
            <dt>この端末への固定</dt><dd>{i.machineBound ? "あり(この端末でだけ使えます)" : "なし(他のパソコンにも入れられます)"}</dd>
            {i.dev && <><dt>種類</dt><dd>開発用のライセンス</dd></>}
          </dl>
        ) : pro ? (
          <p className="note">ライセンスは登録されていません(開発用の設定で有料版にしています)。</p>
        ) : (
          <p className="note">ライセンスは登録されていません。キーをお持ちの方は、下に貼り付けて「登録」を押してください。購入の方法は準備中です。</p>
        )}
      </div>
      {st.error && <p className="msg err" data-testid="license-saved-error">保存してあるライセンスが使えません: {st.error}</p>}

      <label className="lic-field">ライセンスキー
        <textarea rows={3} value={text} placeholder="MNT1- から始まる文字列を貼り付け" data-testid="license-input" spellCheck={false}
          onChange={(e) => { setText(e.target.value); setErr(null); }} />
      </label>
      <div className="row">
        <button className="btn primary" data-testid="license-install" disabled={busy || !text.trim()} onClick={() => run(() => api.installLicense(text))}>登録</button>
        <button className="btn" data-testid="license-file" disabled={busy} onClick={() => run(() => api.pickAndInstallLicense())}>ファイルから登録</button>
        {i && !ask && <button className="btn small ghost" data-testid="license-remove" onClick={() => setAsk(true)}>ライセンスを外す…</button>}
        {i && ask && (
          <span className="ask" data-testid="license-remove-confirm">
            外すと無料版に戻ります(議事録などのデータは消えません)。キーがあれば、またここに入れて登録できます。
            <button className="btn small danger" data-testid="license-remove-yes" onClick={remove}>外す</button>
            <button className="btn small" onClick={() => setAsk(false)}>やめる</button>
          </span>
        )}
      </div>
      {err && <p className="msg err" role="alert" data-testid="license-error">{err}</p>}
      {ok && <p className="msg" data-testid="license-ok">{ok}</p>}
      <p className="note">ライセンスの確認は、このパソコンの中だけで行います。通信はしません。「全データを削除」を押しても、ライセンスは消えません。</p>

      {st.machineCode && (
        <div className="lic-machine">
          <h3>端末コード</h3>
          <p className="note">ライセンスを、この端末だけで使えるように発行してもらうときに、購入先へ伝えるコードです。このパソコンの識別子そのものではありません。</p>
          <div className="row">
            <code className="mono lic-code" data-testid="machine-code">{st.machineCode}</code>
            <button className="btn small" data-testid="machine-copy" onClick={copy}>{copied ? "コピーしました" : "コピー"}</button>
          </div>
        </div>
      )}
    </section>
  );
}

export function SettingsView({ api, settings: s, plan, onSettings, onDeleted, onModelReady, onLicenseChanged }: {
  api: Api; settings: SettingsInfo | null; plan?: Plan | null; onSettings: (s: SettingsInfo) => void; onDeleted: () => void; onModelReady: () => void;
  /** ライセンスを登録・外したとき(プランと設定を読み直す) */
  onLicenseChanged: () => void;
}) {
  const [ask, setAsk] = useState(false);
  const [typed, setTyped] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [notices, setNotices] = useState<string | null>(null);
  if (!s) return <div className="pane">読み込み中…</div>;
  const set = async (k: Flag, on: boolean) => { try { onSettings(await api.setFlag(k, on)); } catch (e) { setMsg(String(e)); } };

  return (
    <div className="pane">
      <PageHead label="Settings" title="設定">プラン、文字起こしや要約の準備、データの扱いを確認できます。</PageHead>
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
      <LicenseSection api={api} plan={plan} onChanged={() => { api.settings().then(onSettings).catch(() => undefined); onLicenseChanged(); }} />
      <section className="card" data-testid="offline-mode">
        <h2>ネットワークを使わない</h2>
        <label className={"check" + (s.offlineForced ? " locked" : "")}>
          <input type="checkbox" data-testid="offline-toggle" checked={s.offlineMode} disabled={s.offlineForced} onChange={(e) => set("offline_mode", e.target.checked)} /> ネットワークを使う機能をすべて止める
        </label>
        {s.offlineForced
          ? <p className="note" data-testid="offline-forced">オフライン版のライセンスが登録されているため、常にオンです(切り替えられません)。</p>
          : <p className="note">オンにすると、追加ダウンロードなどの通信を止めます。要約に必要なファイルはUSBなどから取り込めます。通信できない場所や、機密の扱いが厳しい場所向けです。</p>}
      </section>
      <ModelSection key={`m-${plan?.tier}`} api={api} plan={plan} offline={s.offlineMode} onChanged={() => { api.settings().then(onSettings); onModelReady(); }} />
      <SummarySection key={`s-${plan?.tier}`} api={api} plan={plan} offline={s.offlineMode} />
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
                <td>{s.offlineMode ? "止めています(ネットワークを使わない設定)" : e.stoppable ? <label><input type="checkbox" checked={e.enabled} onChange={(ev) => set("update_check", ev.target.checked)} /> {e.enabled ? "オン" : "オフ"}</label> : "止められません"}</td></tr>
            ))}
          </tbody>
        </table>
        <p className="note">購入キーの確認にはインターネットを使いません。この版はアプリの更新を自動確認しません。</p>
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

      <section className="card" id="third-party-licenses">
        <h2>第三者ライセンス・技術情報</h2>
        <p className="note">通常の利用に、この一覧を覚える必要はありません。使用しているモデル、提供元、利用条件をまとめています。</p>
        <table className="table">
          <thead><tr><th>名前</th><th>用途</th><th>ライセンス</th></tr></thead>
          <tbody>
            {[
              ["Whisper small / large-v3-turbo（OpenAI）", "標準 / 高精度の文字起こし", "MIT"],
              ["WeSpeaker ResNet34-LM（WeSpeaker project）", "話者の判別・改変なし", "CC BY 4.0"],
              ["ONNX Runtime（Microsoft）", "話者判別の実行", "MIT"],
              ["whisper.cpp", "文字起こしの実行", "MIT"],
              ["Qwen3-4B-Instruct-2507(Alibaba Cloud)/ GGUF 量子化は Unsloth", "要約のモデル(追加機能・取得したときのみ)", "Apache-2.0"],
              ["llama.cpp / llama-cpp-2", "要約の実行(別の実行ファイル)", "MIT / MIT または Apache-2.0"],
              ["whisper-rs", "whisper.cpp の Rust 束ね", "Unlicense"],
              ["nnnoiseless(RNNoise の移植)", "ノイズ除去", "BSD-3-Clause"],
              ["Symphonia", "音声ファイルの読み込み", "MPL-2.0"],
              ["SQLite / rusqlite", "データベース", "パブリックドメイン / MIT"],
              ["Tauri", "アプリの枠組み", "Apache-2.0 または MIT"],
              ["React", "画面", "MIT"],
            ].map(([n, u, l]) => <tr key={n}><td>{n}</td><td>{u}</td><td>{l}</td></tr>)}
          </tbody>
        </table>
        <p className="note">オフラインで要約を準備する場合の対応ファイル: Qwen3-4B-Instruct-2507 の GGUF（Q4_K_M）。提供元・利用条件・著作権表示は下の全文に記載しています。</p>
        <p className="note">ここに挙げたもののほか、多数のオープンソースのライブラリを使っています。すべての著作権表示とライセンスの全文は、次のボタンから読めます(アプリにも同梱しています)。</p>
        <button className="btn small" onClick={async () => { try { setNotices(await api.thirdPartyNotices()); } catch (e) { setMsg(String(e)); } }}>ライセンスの全文を表示</button>
      </section>

      <section className="card danger-zone">
        <h2>全データを削除</h2>
        <p className="note">音声のコピー・議事録・用語辞書・設定と、ダウンロードした文字起こし用データをすべて消します。元に戻せません。取り込み元の録音ファイルは消えません。</p>
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
