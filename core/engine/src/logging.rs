//! 端末内だけのログ。個人データ(氏名・金額・文章・ファイル名)をログに書かないため、
//! 書けるのは「固定の文字列(`&'static str`)」と「数値」だけにしている。実行時に作った文字列は渡せない。

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
}

/// ログに書ける値。文字列は固定のもの(コード上のリテラル)だけ。
#[derive(Debug, Clone, Copy)]
pub enum Val {
    Int(i64),
    Static(&'static str),
}

impl From<i64> for Val {
    fn from(n: i64) -> Self {
        Val::Int(n)
    }
}
impl From<usize> for Val {
    fn from(n: usize) -> Self {
        Val::Int(n as i64)
    }
}
impl From<&'static str> for Val {
    fn from(s: &'static str) -> Self {
        Val::Static(s)
    }
}

pub struct Logger {
    path: PathBuf,
    max_bytes: u64,
    min: Level,
    file: Mutex<Option<File>>,
}

impl Logger {
    /// `dir/app.log` に書く。`max_bytes` を超えたら `app.log.1` に1世代だけ退避する。
    pub fn new(dir: &Path, max_bytes: u64, min: Level) -> std::io::Result<Self> {
        fs::create_dir_all(dir)?;
        Ok(Self { path: dir.join("app.log"), max_bytes, min, file: Mutex::new(None) })
    }

    pub fn log(&self, level: Level, msg: &'static str, fields: &[(&'static str, Val)]) {
        if level < self.min {
            return;
        }
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut line = format!("{ts} {} {msg}", level.as_str());
        for (k, v) in fields {
            match v {
                Val::Int(n) => line.push_str(&format!(" {k}={n}")),
                Val::Static(s) => line.push_str(&format!(" {k}={s}")),
            }
        }
        line.push('\n');
        let Ok(mut guard) = self.file.lock() else { return };
        if let Ok(meta) = fs::metadata(&self.path) {
            if meta.len() + line.len() as u64 > self.max_bytes {
                *guard = None;
                let _ = fs::rename(&self.path, self.path.with_extension("log.1"));
            }
        }
        if guard.is_none() {
            *guard = OpenOptions::new().create(true).append(true).open(&self.path).ok();
        }
        if let Some(f) = guard.as_mut() {
            let _ = f.write_all(line.as_bytes());
        }
    }

    pub fn info(&self, msg: &'static str, fields: &[(&'static str, Val)]) {
        self.log(Level::Info, msg, fields);
    }
    pub fn warn(&self, msg: &'static str, fields: &[(&'static str, Val)]) {
        self.log(Level::Warn, msg, fields);
    }
    pub fn error(&self, msg: &'static str, fields: &[(&'static str, Val)]) {
        self.log(Level::Error, msg, fields);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("factory-log-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn 固定文字列と数値だけが書かれ_レベルで絞る() {
        let d = tmp("a");
        let l = Logger::new(&d, 10_000, Level::Info).unwrap();
        l.log(Level::Debug, "出ない", &[]);
        l.info("取り込み完了", &[("件数", 3usize.into()), ("結果", "ok".into())]);
        l.error("失敗", &[("コード", 7i64.into())]);
        let t = fs::read_to_string(d.join("app.log")).unwrap();
        assert!(!t.contains("出ない"));
        assert!(t.contains("INFO 取り込み完了 件数=3 結果=ok"));
        assert!(t.contains("ERROR 失敗 コード=7"));
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 上限を超えたら1世代だけ退避する() {
        let d = tmp("b");
        let l = Logger::new(&d, 200, Level::Info).unwrap();
        for _ in 0..30 {
            l.info("行", &[("n", 1i64.into())]);
        }
        assert!(d.join("app.log.1").exists());
        assert!(fs::metadata(d.join("app.log")).unwrap().len() <= 200);
        fs::remove_dir_all(&d).ok();
    }
}
