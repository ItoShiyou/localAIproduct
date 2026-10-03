//! 長い処理の管理。状態は SQLite(`jobs` テーブル)に保存するので、アプリを閉じても再開できる。
//!
//! - 1件のジョブ = 1つの作業単位(例: 領収書1枚の読み取り)。100件の取り込みなら100ジョブ。
//! - `run_pending` は、待ちのジョブを順に実行する。**1件の失敗では止めず**、そのジョブを `failed` にして続ける。
//!   `CancelToken` が立てば、いま実行中のジョブの終了後に止める(残りは `pending` のまま)。
//! - 起動時に `recover` を呼ぶと、前回の中断で `running` のまま残ったジョブを `pending` に戻す。
//! - 画面は `summary` で件数と進捗を得る。実行は呼び出し側のスレッドで行う(このモジュールはスレッドを作らない)。

use crate::db::Db;
use crate::error::CoreError;
use rusqlite::OptionalExtension;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// アプリのマイグレーションに含める SQL(`Db::open` の配列に追加する)。
pub const JOBS_SCHEMA: &str = "CREATE TABLE jobs(
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending',   -- pending | running | done | failed | cancelled
    progress REAL NOT NULL DEFAULT 0,        -- 0.0〜1.0
    error TEXT,
    payload_json TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX jobs_state ON jobs(state, id);";

fn e<T: std::fmt::Display>(x: T) -> CoreError {
    CoreError::Db(x.to_string())
}

#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub id: i64,
    pub kind: String,
    pub state: String,
    pub progress: f64,
    pub error: Option<String>,
    pub payload_json: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    pub pending: usize,
    pub running: usize,
    pub done: usize,
    pub failed: usize,
    pub cancelled: usize,
}

impl Summary {
    pub fn total(&self) -> usize {
        self.pending + self.running + self.done + self.failed + self.cancelled
    }
    /// 全体の進み具合(0.0〜1.0)。終わった(成功・失敗・取り消し)件数 / 全件数
    pub fn fraction(&self) -> f64 {
        let t = self.total();
        if t == 0 {
            1.0
        } else {
            (self.done + self.failed + self.cancelled) as f64 / t as f64
        }
    }
}

#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

pub struct Jobs<'a> {
    db: &'a Db,
}

impl<'a> Jobs<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn enqueue(&self, kind: &str, payload_json: &str) -> Result<i64, CoreError> {
        self.db
            .conn
            .execute("INSERT INTO jobs(kind, payload_json) VALUES (?1, ?2)", (kind, payload_json))
            .map_err(e)?;
        Ok(self.db.conn.last_insert_rowid())
    }

    pub fn get(&self, id: i64) -> Result<Option<Job>, CoreError> {
        self.db
            .conn
            .query_row("SELECT id, kind, state, progress, error, payload_json FROM jobs WHERE id = ?1", [id], |r| {
                Ok(Job { id: r.get(0)?, kind: r.get(1)?, state: r.get(2)?, progress: r.get(3)?, error: r.get(4)?, payload_json: r.get(5)? })
            })
            .optional()
            .map_err(e)
    }

    /// 前回の中断で running のまま残ったジョブを pending に戻す。戻した件数を返す。
    pub fn recover(&self) -> Result<usize, CoreError> {
        self.db.conn.execute("UPDATE jobs SET state='pending', progress=0 WHERE state='running'", []).map_err(e)
    }

    /// 失敗したジョブをもう一度 pending にする(「失敗だけ再実行」)。
    pub fn retry_failed(&self) -> Result<usize, CoreError> {
        self.db.conn.execute("UPDATE jobs SET state='pending', progress=0, error=NULL WHERE state='failed'", []).map_err(e)
    }

    /// 待ちのジョブをすべて取り消す(実行中のものは含まない)。
    pub fn cancel_pending(&self) -> Result<usize, CoreError> {
        self.db.conn.execute("UPDATE jobs SET state='cancelled' WHERE state='pending'", []).map_err(e)
    }

    pub fn set_progress(&self, id: i64, p: f64) -> Result<(), CoreError> {
        self.db.conn.execute("UPDATE jobs SET progress=?2 WHERE id=?1", (id, p.clamp(0.0, 1.0))).map_err(e)?;
        Ok(())
    }

    pub fn summary(&self, kind: Option<&str>) -> Result<Summary, CoreError> {
        let mut s = Summary::default();
        let mut stmt = self
            .db
            .conn
            .prepare("SELECT state, count(*) FROM jobs WHERE (?1 IS NULL OR kind = ?1) GROUP BY state")
            .map_err(e)?;
        let rows = stmt.query_map([kind], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as usize))).map_err(e)?;
        for row in rows {
            let (st, n) = row.map_err(e)?;
            match st.as_str() {
                "pending" => s.pending = n,
                "running" => s.running = n,
                "done" => s.done = n,
                "failed" => s.failed = n,
                "cancelled" => s.cancelled = n,
                _ => {}
            }
        }
        Ok(s)
    }

    fn next_pending(&self) -> Result<Option<Job>, CoreError> {
        let id: Option<i64> = self
            .db
            .conn
            .query_row("SELECT id FROM jobs WHERE state='pending' ORDER BY id LIMIT 1", [], |r| r.get(0))
            .optional()
            .map_err(e)?;
        match id {
            Some(id) => self.get(id),
            None => Ok(None),
        }
    }

    /// 待ちのジョブを順に実行する。`handler` が `Err(理由)` を返したジョブは failed にして続ける。
    /// `on_progress` は1件終わるごとに呼ばれる。実行した件数を返す。
    pub fn run_pending(
        &self,
        cancel: &CancelToken,
        mut handler: impl FnMut(&Job) -> Result<(), String>,
        mut on_progress: impl FnMut(&Summary),
    ) -> Result<usize, CoreError> {
        let mut ran = 0;
        while !cancel.is_cancelled() {
            let Some(job) = self.next_pending()? else { break };
            self.db.conn.execute("UPDATE jobs SET state='running' WHERE id=?1", [job.id]).map_err(e)?;
            // handler の panic でもほかのジョブを止めない
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler(&job)))
                .unwrap_or_else(|_| Err("処理中に予期しない停止が起きました".to_string()));
            match res {
                Ok(()) => self.db.conn.execute("UPDATE jobs SET state='done', progress=1.0, error=NULL WHERE id=?1", [job.id]),
                Err(msg) => self.db.conn.execute("UPDATE jobs SET state='failed', error=?2 WHERE id=?1", (job.id, msg)),
            }
            .map_err(e)?;
            ran += 1;
            on_progress(&self.summary(None)?);
        }
        Ok(ran)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Db {
        Db::open_in_memory(&[JOBS_SCHEMA]).unwrap()
    }

    #[test]
    fn 順に実行し_失敗しても続ける_進捗が出る() {
        let d = db();
        let j = Jobs::new(&d);
        for n in 0..5 {
            j.enqueue("read", &format!("{{\"n\":{n}}}")).unwrap();
        }
        let mut seen = vec![];
        let ran = j
            .run_pending(&CancelToken::new(), |job| if job.payload_json.contains("2") { Err("読めません".into()) } else { Ok(()) }, |s| seen.push(s.fraction()))
            .unwrap();
        assert_eq!(ran, 5);
        let s = j.summary(None).unwrap();
        assert_eq!((s.done, s.failed, s.pending), (4, 1, 0));
        assert_eq!(seen.last(), Some(&1.0));
        assert_eq!(j.get(3).unwrap().unwrap().error.as_deref(), Some("読めません"));
    }

    #[test]
    fn 中断すると残りは待ちのまま_再開で続きから() {
        let d = db();
        let j = Jobs::new(&d);
        for _ in 0..4 {
            j.enqueue("read", "{}").unwrap();
        }
        let c = CancelToken::new();
        let mut n = 0;
        j.run_pending(&c, |_| { n += 1; if n == 2 { c.cancel(); } Ok(()) }, |_| {}).unwrap();
        let s = j.summary(None).unwrap();
        assert_eq!((s.done, s.pending), (2, 2));
        let ran = j.run_pending(&CancelToken::new(), |_| Ok(()), |_| {}).unwrap();
        assert_eq!(ran, 2);
        assert_eq!(j.summary(None).unwrap().done, 4);
    }

    #[test]
    fn 異常終了で残ったrunningは起動時に待ちへ戻る() {
        let d = db();
        let j = Jobs::new(&d);
        let id = j.enqueue("read", "{}").unwrap();
        d.conn.execute("UPDATE jobs SET state='running' WHERE id=?1", [id]).unwrap();
        assert_eq!(j.recover().unwrap(), 1);
        assert_eq!(j.get(id).unwrap().unwrap().state, "pending");
    }

    #[test]
    fn panicしてもほかのジョブは動く_失敗の再実行と取り消し() {
        let d = db();
        let j = Jobs::new(&d);
        for _ in 0..3 {
            j.enqueue("k", "{}").unwrap();
        }
        let mut n = 0;
        j.run_pending(&CancelToken::new(), |_| { n += 1; if n == 1 { panic!("x") } else { Ok(()) } }, |_| {}).unwrap();
        let s = j.summary(None).unwrap();
        assert_eq!((s.done, s.failed), (2, 1));
        assert_eq!(j.retry_failed().unwrap(), 1);
        assert_eq!(j.cancel_pending().unwrap(), 1);
        assert_eq!(j.summary(Some("k")).unwrap().cancelled, 1);
        assert_eq!(j.summary(Some("other")).unwrap().total(), 0);
    }
}
