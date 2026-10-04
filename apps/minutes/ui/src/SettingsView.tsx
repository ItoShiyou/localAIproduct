import { useState } from "react";
import type { Api } from "./api";
import type { Flag, SettingsInfo } from "./types";

export function SettingsView({ api, settings: s, onSettings, onDeleted }: {
  api: Api; settings: SettingsInfo | null; onSettings: (s: SettingsInfo) => void; onDeleted: () => void;
}) {
  const [ask, setAsk] = useState(false);
  const [typed, setTyped] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  if (!s) return <div className="pane">読み込み中…</div>;
  const set = async (k: Flag, on: boolean) => { try { onSettings(await api.setFlag(k, on)); } catch (e) { setMsg(String(e)); } };

  return (
    <div className="pane">
      <section className="card">
        <h2>処理の設定</h2>
        <label className="check"><input type="checkbox" checked={s.denoiseDefault} onChange={(e) => set("denoise_default", e.target.checked)} /> 取り込むとき、ノイズ除去を既定でオンにする</label>
        <p className="note">ノイズ除去で文字起こしの精度が下がる録音もあります。結果が良くないときは、オフにして取り込み直してください。</p>
        <label className="check"><input type="checkbox" checked={s.keepAudio} onChange={(e) => set("keep_audio", e.target.checked)} /> 文字起こしの後も音声のコピーを残す(確認画面で再生するために必要)</label>
        <p className="note">オフにすると、文字起こしが終わった時点で音声のコピーを消し、文字だけを残します。</p>
        <p className="note">文字起こしのモデル: {s.model === "none" ? "未設定(文字起こしはできません)" : s.model}</p>
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

      <section className="card danger-zone">
        <h2>全データを削除</h2>
        <p className="note">音声のコピー・議事録・用語辞書・設定をすべて消します。元に戻せません。取り込み元の録音ファイルは消えません。</p>
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
