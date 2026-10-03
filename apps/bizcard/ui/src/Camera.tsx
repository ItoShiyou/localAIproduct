import { useEffect, useRef, useState } from "react";

interface Props {
  onCapture: (dataUrl: string) => void;
  onFiles: (files: File[]) => void;
}

/** カメラ(getUserMedia)のプレビューと撮影ボタン。使えなければ、写真の取り込みだけを出す。
 *  スマホの縦持ちでは背面カメラ(facingMode: environment)を優先する。映像は端末の外に出さない。 */
export function Camera({ onCapture, onFiles }: Props) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const [state, setState] = useState<"starting" | "ready" | "unavailable">("starting");
  const [why, setWhy] = useState("");

  useEffect(() => {
    let stream: MediaStream | null = null;
    let cancelled = false;
    (async () => {
      try {
        if (!navigator.mediaDevices?.getUserMedia) throw new Error("このブラウザではカメラを使えません");
        stream = await navigator.mediaDevices.getUserMedia({
          video: { facingMode: { ideal: "environment" }, width: { ideal: 1920 }, height: { ideal: 1080 } },
          audio: false,
        });
        if (cancelled) {
          stream.getTracks().forEach((t) => t.stop());
          return;
        }
        const v = videoRef.current!;
        v.srcObject = stream;
        await v.play().catch(() => undefined);
        setState("ready");
      } catch (e) {
        setWhy(e instanceof Error ? e.message : String(e));
        setState("unavailable");
      }
    })();
    return () => {
      cancelled = true;
      stream?.getTracks().forEach((t) => t.stop());
    };
  }, []);

  const shoot = () => {
    const v = videoRef.current;
    if (!v || !v.videoWidth) return;
    const c = document.createElement("canvas");
    c.width = v.videoWidth;
    c.height = v.videoHeight;
    c.getContext("2d")!.drawImage(v, 0, 0);
    onCapture(c.toDataURL("image/jpeg", 0.92));
  };

  return (
    <div className="camera" data-testid="camera">
      {state !== "unavailable" ? (
        <div className="viewport">
          <video ref={videoRef} playsInline muted data-testid="preview" />
          <div className="guide" aria-hidden="true" />
          {state === "starting" && <p className="overlay">カメラを起動しています…</p>}
        </div>
      ) : (
        <div className="viewport unavailable" data-testid="camera-unavailable">
          <p>カメラを使えません。写真ファイルを取り込んでください。</p>
          <small>{why}</small>
        </div>
      )}
      <div className="actions">
        <button className="shutter" data-testid="shutter" disabled={state !== "ready"} onClick={shoot} aria-label="撮影">
          <span />
        </button>
        <label className="btn secondary">
          写真を選ぶ
          <input type="file" accept="image/jpeg,image/png" multiple hidden data-testid="file-input"
            onChange={(e) => { onFiles(Array.from(e.target.files ?? [])); e.target.value = ""; }} />
        </label>
        <label className="btn secondary">
          フォルダ
          <input type="file" accept="image/jpeg,image/png" multiple hidden data-testid="folder-input"
            // @ts-expect-error 非標準属性(Chromium / WebView2 / WKWebView で動く)
            webkitdirectory=""
            onChange={(e) => { onFiles(Array.from(e.target.files ?? []).filter((f) => /^image\/(jpeg|png)$/.test(f.type))); e.target.value = ""; }} />
        </label>
      </div>
    </div>
  );
}
