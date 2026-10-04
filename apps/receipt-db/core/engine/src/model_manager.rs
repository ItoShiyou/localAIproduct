//! モデルの取得・確認・削除。
//!
//! - 取得は**利用者が画面で操作したときだけ**呼ぶこと(自動では取得しない)。送るのは HTTP の GET(モデルのURL)だけ。
//! - 途中で切れても続きから取得する(`<名前>.part` と HTTP の Range)。サーバーが Range に応じなければ最初から。
//! - 取得後に SHA-256 を照合し、合わなければ捨てる。照合済みの印(`<名前>.sha256`)を置き、起動のたびに読み直さない。
//! - 空き容量が足りないときは、それと分かるエラーを返す。

use crate::error::CoreError;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// アプリが使うモデル1つの定義(名前・取得元・ハッシュ・大きさ)。
#[derive(Debug, Clone, PartialEq)]
pub struct ModelSpec {
    /// 画面に出す名前
    pub name: &'static str,
    pub file_name: &'static str,
    pub url: &'static str,
    /// SHA-256(小文字の16進)
    pub sha256: &'static str,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub name: String,
    pub installed: bool,
    /// 途中まで取得したバイト数(取得済みなら size と同じ)
    pub downloaded: u64,
    pub size: u64,
}

pub struct ModelManager {
    dir: PathBuf,
}

fn io<E: std::fmt::Display>(e: E) -> CoreError {
    CoreError::Model(e.to_string())
}

fn write_err(e: std::io::Error) -> CoreError {
    if e.kind() == std::io::ErrorKind::StorageFull {
        CoreError::Model("ディスクの空き容量が足りません。空きを作ってから、もう一度取得してください(途中から再開します)".into())
    } else {
        CoreError::Model(format!("モデルを保存できません: {e}"))
    }
}

pub fn sha256_file(path: &Path) -> Result<String, CoreError> {
    let mut r = BufReader::with_capacity(1 << 20, File::open(path).map_err(io)?);
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = r.read(&mut buf).map_err(io)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

impl ModelManager {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn path(&self, spec: &ModelSpec) -> PathBuf {
        self.dir.join(spec.file_name)
    }
    fn part(&self, spec: &ModelSpec) -> PathBuf {
        self.dir.join(format!("{}.part", spec.file_name))
    }
    fn mark(&self, spec: &ModelSpec) -> PathBuf {
        self.dir.join(format!("{}.sha256", spec.file_name))
    }

    /// 取得済みで、照合も済んでいるか(照合の印と大きさで判定。中身は読み直さない)。
    pub fn is_installed(&self, spec: &ModelSpec) -> bool {
        let ok_size = fs::metadata(self.path(spec)).map(|m| m.len() == spec.size).unwrap_or(false);
        let ok_mark = fs::read_to_string(self.mark(spec)).map(|s| s.trim() == spec.sha256).unwrap_or(false);
        ok_size && ok_mark
    }

    /// 取得済みならその場所。
    pub fn installed_path(&self, spec: &ModelSpec) -> Option<PathBuf> {
        self.is_installed(spec).then(|| self.path(spec))
    }

    pub fn status(&self, spec: &ModelSpec) -> ModelStatus {
        let installed = self.is_installed(spec);
        let downloaded = if installed { spec.size } else { fs::metadata(self.part(spec)).map(|m| m.len()).unwrap_or(0) };
        ModelStatus { name: spec.name.into(), installed, downloaded, size: spec.size }
    }

    /// 取得する(続きから)。`on_progress(取得済み, 全体)` を少しずつ呼ぶ。`cancel` が立ったら止める(途中までは残る)。
    pub fn download(&self, spec: &ModelSpec, cancel: &AtomicBool, mut on_progress: impl FnMut(u64, u64)) -> Result<PathBuf, CoreError> {
        if self.is_installed(spec) {
            return Ok(self.path(spec));
        }
        fs::create_dir_all(&self.dir).map_err(write_err)?;
        let part = self.part(spec);
        let mut have = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        if have > spec.size {
            fs::remove_file(&part).map_err(io)?;
            have = 0;
        }
        if have < spec.size {
            let agent = ureq::AgentBuilder::new()
                .timeout_connect(std::time::Duration::from_secs(30))
                .timeout_read(std::time::Duration::from_secs(60))
                .build();
            let mut req = agent.get(spec.url);
            if have > 0 {
                req = req.set("Range", &format!("bytes={have}-"));
            }
            let resp = req.call().map_err(|e| CoreError::Network(e.to_string()))?;
            let resumed = resp.status() == 206;
            if !resumed {
                have = 0; // Range に応じなかった。最初から
            }
            let mut out = OpenOptions::new().create(true).write(true).append(resumed).truncate(!resumed).open(&part).map_err(write_err)?;
            let mut body = resp.into_reader();
            let mut buf = vec![0u8; 1 << 20];
            on_progress(have, spec.size);
            loop {
                if cancel.load(Ordering::SeqCst) {
                    out.flush().map_err(write_err)?;
                    return Err(CoreError::Cancelled);
                }
                let n = body.read(&mut buf).map_err(|e| CoreError::Network(format!("取得が途中で切れました(続きから再開できます): {e}")))?;
                if n == 0 {
                    break;
                }
                if have + n as u64 > spec.size {
                    drop(out);
                    let _ = fs::remove_file(&part);
                    return Err(CoreError::Model("取得したデータが想定より大きいため、破棄しました".into()));
                }
                out.write_all(&buf[..n]).map_err(write_err)?;
                have += n as u64;
                on_progress(have, spec.size);
            }
            out.flush().map_err(write_err)?;
        }
        if have < spec.size {
            return Err(CoreError::Network("取得が途中で切れました(続きから再開できます)".into()));
        }
        let got = sha256_file(&part)?;
        if got != spec.sha256 {
            let _ = fs::remove_file(&part);
            return Err(CoreError::Model("取得したモデルのハッシュが一致しないため、破棄しました。もう一度取得してください".into()));
        }
        fs::rename(&part, self.path(spec)).map_err(io)?;
        fs::write(self.mark(spec), spec.sha256).map_err(write_err)?;
        Ok(self.path(spec))
    }

    /// 取得済み・途中のファイルを消す。
    pub fn delete(&self, spec: &ModelSpec) -> Result<(), CoreError> {
        for p in [self.path(spec), self.part(spec), self.mark(spec)] {
            if p.exists() {
                fs::remove_file(&p).map_err(io)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader as BR};
    use std::net::TcpListener;
    use std::sync::Arc;

    /// Range に応じる小さな HTTP サーバー。`cut_first` なら1回目は途中で接続を切る。
    fn serve(data: Arc<Vec<u8>>, cut_first: bool, honor_range: bool) -> (String, std::thread::JoinHandle<()>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/model.bin", l.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            for (i, s) in l.incoming().take(2).enumerate() {
                let mut s = s.unwrap();
                let mut r = BR::new(s.try_clone().unwrap());
                let mut start = 0usize;
                loop {
                    let mut line = String::new();
                    r.read_line(&mut line).unwrap();
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
                        if honor_range {
                            start = v.trim().trim_end_matches('-').parse().unwrap();
                        }
                    }
                }
                let body = &data[start..];
                let head = if start > 0 {
                    format!("HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\n\r\n", body.len(), start, data.len() - 1, data.len())
                } else {
                    format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len())
                };
                s.write_all(head.as_bytes()).unwrap();
                let n = if cut_first && i == 0 { body.len() / 3 } else { body.len() };
                let _ = s.write_all(&body[..n]);
                let _ = s.flush();
                if cut_first && i == 0 {
                    let _ = s.shutdown(std::net::Shutdown::Both);
                }
            }
        });
        (url, h)
    }

    fn data() -> Arc<Vec<u8>> {
        Arc::new((0..3_000_000u32).map(|i| (i * 7 % 251) as u8).collect())
    }

    fn spec(url: &str, d: &[u8]) -> ModelSpec {
        let sha = format!("{:x}", Sha256::digest(d));
        ModelSpec { name: "テスト", file_name: "model.bin", url: Box::leak(url.to_string().into_boxed_str()), sha256: Box::leak(sha.into_boxed_str()), size: d.len() as u64 }
    }

    fn tmp(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("core-mm-{n}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn 途中で切れても続きから取得し_ハッシュを照合する() {
        let d = data();
        let (url, h) = serve(d.clone(), true, true);
        let s = spec(&url, &d);
        let m = ModelManager::new(tmp("resume"));
        assert!(!m.status(&s).installed);
        let first = m.download(&s, &AtomicBool::new(false), |_, _| {});
        assert!(matches!(first, Err(CoreError::Network(_))), "{first:?}");
        let partial = m.status(&s).downloaded;
        assert!(partial > 0 && partial < s.size);
        let mut last = (0, 0);
        let p = m.download(&s, &AtomicBool::new(false), |a, b| last = (a, b)).unwrap();
        h.join().unwrap();
        assert_eq!(last, (s.size, s.size));
        assert_eq!(fs::read(&p).unwrap(), *d);
        assert!(m.is_installed(&s));
        assert_eq!(m.installed_path(&s), Some(p));
        m.delete(&s).unwrap();
        assert!(!m.is_installed(&s) && m.status(&s).downloaded == 0);
    }

    #[test]
    fn rangeに応じないサーバーなら最初から取り直す() {
        let d = data();
        let (url, h) = serve(d.clone(), true, false);
        let s = spec(&url, &d);
        let m = ModelManager::new(tmp("norange"));
        assert!(m.download(&s, &AtomicBool::new(false), |_, _| {}).is_err());
        m.download(&s, &AtomicBool::new(false), |_, _| {}).unwrap();
        h.join().unwrap();
        assert_eq!(fs::read(m.path(&s)).unwrap(), *d);
    }

    #[test]
    fn ハッシュが合わなければ捨てる_中断できる() {
        let d = data();
        let (url, h) = serve(d.clone(), false, true);
        let mut s = spec(&url, &d);
        s.sha256 = "00";
        let m = ModelManager::new(tmp("badhash"));
        let r = m.download(&s, &AtomicBool::new(false), |_, _| {});
        assert!(matches!(r, Err(CoreError::Model(_))), "{r:?}");
        assert!(!m.is_installed(&s) && m.status(&s).downloaded == 0);
        // 中断
        let cancel = AtomicBool::new(false);
        let r = m.download(&s, &cancel, |a, _| if a > 0 { cancel.store(true, Ordering::SeqCst) });
        assert_eq!(r, Err(CoreError::Cancelled));
        h.join().unwrap();
    }
}
