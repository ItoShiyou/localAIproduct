import { useEffect, useState } from "react";
import type { Api } from "./api";
import type { SettingsInfo } from "./types";

export function SettingsView({ api, onDeleted }: { api: Api; onDeleted: () => void }) {
  const [s, setS] = useState<SettingsInfo | null>(null);
  const [ask, setAsk] = useState(false);
  const [typed, setTyped] = useState("");
  const [msg, setMsg] = useState<string | null>(null);

  useEffect(() => { api.settings().then(setS).catch((e) => setMsg(String(e))); }, [api]);
  if (!s) return <div className="pane">{msg ?? "読み込み中…"}</div>;

  return (
    <div className="pane">
      <section className="card">
        <h2>通信一覧</h2>
        <p className="note">領収書の画像・PDF・読み取り結果を送る通信はありません。通信するのは次の場面だけです。</p>
        <table className="table">
          <thead><tr><th>目的</th><th>送信先</th><th>送る内容</th><th>状態</th></tr></thead>
          <tbody>
            {s.network.map((e) => (
              <tr key={e.purpose}><td>{e.purpose}</td><td>{e.destination}</td><td>{e.content}</td>
                <td>{e.stoppable ? (
                  <label><input type="checkbox" checked={e.enabled} onChange={async (ev) => setS(await api.setUpdateCheck(ev.target.checked))} /> {e.enabled ? "オン" : "オフ"}</label>
                ) : "止められません"}</td></tr>
            ))}
          </tbody>
        </table>
        <p className="note">ライセンスの有効化と更新の確認は、まだ接続していません(開発中)。</p>
      </section>

      <section className="card">
        <h2>データの保存場所</h2>
        <p className="mono">{s.dataDir}</p>
        <p className="note">原本のコピー・データベース・設定はこのフォルダに保存します。アプリ内では暗号化していません。パソコンのディスク暗号化(macOS は FileVault、Windows は BitLocker)を有効にしておくことをおすすめします。</p>
        <p className="note">文字の読み取り: {s.ocr === "real" ? "有効(この端末の中で処理します)" : "モデルが未設定のため、画像の読み取りは行いません(テキスト層のある PDF は読めます。画像は確認画面で手入力してください)"}</p>
      </section>

      <section className="card danger-zone">
        <h2>全データを削除</h2>
        <p className="note">原本のコピー・読み取り結果・設定をすべて消します。元に戻せません。取り込み元のファイルは消えません。</p>
        {!ask ? (
          <button className="btn danger" onClick={() => setAsk(true)}>全データを削除…</button>
        ) : (
          <div className="row">
            <input placeholder="「削除」と入力" value={typed} onChange={(e) => setTyped(e.target.value)} />
            <button className="btn danger" disabled={typed !== "削除"} onClick={async () => {
              try { await api.deleteAll(); setMsg("すべて削除しました"); setAsk(false); setTyped(""); setS(await api.settings()); onDeleted(); }
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
