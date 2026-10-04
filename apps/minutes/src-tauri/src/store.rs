//! データベース(SQLite)。議事録、区間(chunks: 処理の単位、再開用)、文(segments)、用語辞書、編集の履歴(元に戻す)。
//!
//! - 「確定」になるのは利用者が確認画面で `confirm` したものだけ。書き出しは確定した議事録だけ。
//! - 文は `raw_text`(文字起こしの結果そのまま)と `text`(用語辞書の置換・手での修正の後)を持つ。元の結果は消えない。
//! - 元に戻すは、操作の直前の、その議事録の文をまるごと控えておき、戻す(結合・分割・辞書の適用も同じ仕組み)。

use factory_core::db::{fold_for_search, Db};
use factory_core::jobs::JOBS_SCHEMA;
use factory_core::CoreError;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const MIGRATIONS: [&str; 2] = [
    "CREATE TABLE meetings(
        id INTEGER PRIMARY KEY, title TEXT NOT NULL, held_on TEXT, participants_text TEXT NOT NULL DEFAULT '',
        source_name TEXT NOT NULL, audio_path TEXT, pcm_path TEXT, duration_ms INTEGER,
        denoise INTEGER NOT NULL DEFAULT 1,
        state TEXT NOT NULL DEFAULT 'queued' CHECK (state IN ('queued','processing','done','failed')),
        error TEXT,
        status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','confirmed')),
        created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
     CREATE TABLE chunks(
        meeting_id INTEGER NOT NULL REFERENCES meetings(id) ON DELETE CASCADE, idx INTEGER NOT NULL,
        start_ms INTEGER NOT NULL, end_ms INTEGER NOT NULL, silent INTEGER NOT NULL, done INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (meeting_id, idx));
     CREATE TABLE segments(
        id INTEGER PRIMARY KEY, meeting_id INTEGER NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
        chunk_idx INTEGER NOT NULL, start_ms INTEGER NOT NULL, end_ms INTEGER NOT NULL,
        speaker TEXT NOT NULL DEFAULT '', text TEXT NOT NULL, raw_text TEXT NOT NULL,
        confidence REAL NOT NULL DEFAULT 1.0, edited INTEGER NOT NULL DEFAULT 0);
     CREATE INDEX segments_meeting ON segments(meeting_id, start_ms);
     CREATE VIRTUAL TABLE segments_fts USING fts5(text, tokenize='trigram');
     CREATE TABLE segment_history(
        id INTEGER PRIMARY KEY, meeting_id INTEGER NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
        label TEXT NOT NULL, snapshot_json TEXT NOT NULL, at INTEGER NOT NULL);
     CREATE TABLE glossary(id INTEGER PRIMARY KEY, wrong TEXT NOT NULL, right TEXT NOT NULL, UNIQUE(wrong));",
    JOBS_SCHEMA,
];

pub const LOW_CONFIDENCE: f64 = 0.6;
const HISTORY_KEEP: i64 = 50;

fn e<T: std::fmt::Display>(x: T) -> CoreError {
    CoreError::Db(x.to_string())
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meeting {
    pub id: i64,
    pub title: String,
    pub held_on: Option<String>,
    pub participants_text: String,
    pub source_name: String,
    pub has_audio: bool,
    pub duration_ms: Option<i64>,
    pub denoise: bool,
    /// 処理の状態: queued | processing | done | failed
    pub state: String,
    pub error: Option<String>,
    /// draft | confirmed
    pub status: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub id: i64,
    pub chunk_idx: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub speaker: String,
    pub text: String,
    pub raw_text: String,
    pub confidence: f64,
    pub edited: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub meeting_id: i64,
    pub title: String,
    pub held_on: Option<String>,
    pub segment_id: i64,
    pub start_ms: i64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlossaryEntry {
    pub id: i64,
    pub wrong: String,
    pub right: String,
}

pub struct Store {
    pub db: Db,
}

/// 用語辞書を適用する(長い語から順に置換。ほかの語の置換結果をさらに置換しないよう、一度に走査する)。
pub fn apply_glossary(text: &str, entries: &[GlossaryEntry]) -> String {
    let mut es: Vec<&GlossaryEntry> = entries.iter().filter(|g| !g.wrong.is_empty()).collect();
    es.sort_by(|a, b| b.wrong.chars().count().cmp(&a.wrong.chars().count()));
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    'outer: while !rest.is_empty() {
        for g in &es {
            if let Some(r) = rest.strip_prefix(g.wrong.as_str()) {
                out.push_str(&g.right);
                rest = r;
                continue 'outer;
            }
        }
        let c = rest.chars().next().unwrap();
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, CoreError> {
        let db = Db::open(path, &MIGRATIONS)?;
        db.conn.busy_timeout(std::time::Duration::from_secs(10)).map_err(e)?;
        Ok(Self { db })
    }
    pub fn open_in_memory() -> Result<Self, CoreError> {
        Ok(Self { db: Db::open_in_memory(&MIGRATIONS)? })
    }

    // ---------------- 議事録 ----------------

    pub fn add_meeting(&self, title: &str, source_name: &str, audio_path: &str, denoise: bool) -> Result<i64, CoreError> {
        self.db
            .conn
            .execute(
                "INSERT INTO meetings(title, source_name, audio_path, denoise, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?5)",
                (title, source_name, audio_path, denoise as i64, now()),
            )
            .map_err(e)?;
        Ok(self.db.conn.last_insert_rowid())
    }

    const MCOLS: &'static str =
        "id, title, held_on, participants_text, source_name, audio_path IS NOT NULL, duration_ms, denoise, state, error, status, created_at";

    fn meeting_of(r: &rusqlite::Row) -> rusqlite::Result<Meeting> {
        Ok(Meeting {
            id: r.get(0)?, title: r.get(1)?, held_on: r.get(2)?, participants_text: r.get(3)?, source_name: r.get(4)?,
            has_audio: r.get(5)?, duration_ms: r.get(6)?, denoise: r.get::<_, i64>(7)? != 0, state: r.get(8)?,
            error: r.get(9)?, status: r.get(10)?, created_at: r.get(11)?,
        })
    }

    pub fn meeting(&self, id: i64) -> Result<Option<Meeting>, CoreError> {
        self.db.conn.query_row(&format!("SELECT {} FROM meetings WHERE id=?1", Self::MCOLS), [id], Self::meeting_of).optional().map_err(e)
    }

    pub fn meetings(&self) -> Result<Vec<Meeting>, CoreError> {
        let mut st = self.db.conn.prepare(&format!("SELECT {} FROM meetings ORDER BY COALESCE(held_on, '') DESC, id DESC", Self::MCOLS)).map_err(e)?;
        let rows = st.query_map([], Self::meeting_of).map_err(e)?.collect::<Result<Vec<_>, _>>().map_err(e)?;
        Ok(rows)
    }

    pub fn paths(&self, id: i64) -> Result<(Option<String>, Option<String>), CoreError> {
        self.db.conn.query_row("SELECT audio_path, pcm_path FROM meetings WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).map_err(e)
    }

    /// タイトル・日付・参加者を直す。確定済みなら下書きに戻る。
    pub fn update_meta(&self, id: i64, title: &str, held_on: Option<&str>, participants: &str) -> Result<(), CoreError> {
        if title.trim().is_empty() {
            return Err(CoreError::Db("タイトルを入力してください".into()));
        }
        if let Some(d) = held_on {
            if !is_valid_date(d) {
                return Err(CoreError::Db("日付は YYYY-MM-DD の形で入力してください".into()));
            }
        }
        self.db
            .conn
            .execute(
                "UPDATE meetings SET title=?2, held_on=?3, participants_text=?4, status='draft', updated_at=?5 WHERE id=?1",
                (id, title.trim(), held_on, participants, now()),
            )
            .map_err(e)?;
        Ok(())
    }

    pub fn set_state(&self, id: i64, state: &str, error: Option<&str>) -> Result<(), CoreError> {
        self.db.conn.execute("UPDATE meetings SET state=?2, error=?3, updated_at=?4 WHERE id=?1", (id, state, error, now())).map_err(e)?;
        Ok(())
    }

    pub fn set_pcm(&self, id: i64, pcm_path: Option<&str>, duration_ms: Option<i64>) -> Result<(), CoreError> {
        self.db
            .conn
            .execute("UPDATE meetings SET pcm_path=?2, duration_ms=COALESCE(?3, duration_ms) WHERE id=?1", (id, pcm_path, duration_ms))
            .map_err(e)?;
        Ok(())
    }

    pub fn clear_audio_path(&self, id: i64) -> Result<(), CoreError> {
        self.db.conn.execute("UPDATE meetings SET audio_path=NULL WHERE id=?1", [id]).map_err(e)?;
        Ok(())
    }

    /// 確定する(利用者が確認画面で押したときだけ呼ぶ)。文字起こしが終わっていなければ確定しない。
    pub fn confirm(&self, id: i64) -> Result<(), CoreError> {
        let m = self.meeting(id)?.ok_or_else(|| CoreError::Db("見つかりません".into()))?;
        if m.state != "done" {
            return Err(CoreError::Db("文字起こしが終わってから確定してください".into()));
        }
        self.db.conn.execute("UPDATE meetings SET status='confirmed', updated_at=?2 WHERE id=?1", (id, now())).map_err(e)?;
        Ok(())
    }

    pub fn unconfirm(&self, id: i64) -> Result<(), CoreError> {
        self.db.conn.execute("UPDATE meetings SET status='draft', updated_at=?2 WHERE id=?1", (id, now())).map_err(e)?;
        Ok(())
    }

    /// 議事録を消す(文・区間・履歴も)。音声ファイルの場所を返す(呼び出し側で消す)。
    pub fn delete_meeting(&self, id: i64) -> Result<Vec<String>, CoreError> {
        let (a, p) = self.paths(id)?;
        let tx = self.db.conn.unchecked_transaction().map_err(e)?;
        tx.execute("DELETE FROM segments_fts WHERE rowid IN (SELECT id FROM segments WHERE meeting_id=?1)", [id]).map_err(e)?;
        tx.execute("DELETE FROM jobs WHERE json_extract(payload_json, '$.meeting_id')=?1", [id]).map_err(e)?;
        tx.execute("DELETE FROM segments WHERE meeting_id=?1", [id]).map_err(e)?;
        tx.execute("DELETE FROM chunks WHERE meeting_id=?1", [id]).map_err(e)?;
        tx.execute("DELETE FROM segment_history WHERE meeting_id=?1", [id]).map_err(e)?;
        tx.execute("DELETE FROM meetings WHERE id=?1", [id]).map_err(e)?;
        tx.commit().map_err(e)?;
        Ok([a, p].into_iter().flatten().collect())
    }

    // ---------------- 区間(処理の単位) ----------------

    pub fn has_chunks(&self, id: i64) -> Result<bool, CoreError> {
        self.db.conn.query_row("SELECT count(*) FROM chunks WHERE meeting_id=?1", [id], |r| r.get::<_, i64>(0)).map(|n| n > 0).map_err(e)
    }

    pub fn set_chunks(&self, id: i64, chunks: &[crate::audio::Chunk]) -> Result<(), CoreError> {
        let tx = self.db.conn.unchecked_transaction().map_err(e)?;
        tx.execute("DELETE FROM chunks WHERE meeting_id=?1", [id]).map_err(e)?;
        for (i, c) in chunks.iter().enumerate() {
            tx.execute(
                "INSERT INTO chunks(meeting_id, idx, start_ms, end_ms, silent) VALUES (?1,?2,?3,?4,?5)",
                (id, i as i64, c.start_ms as i64, c.end_ms as i64, c.silent as i64),
            )
            .map_err(e)?;
        }
        tx.commit().map_err(e)
    }

    /// 未処理の区間: (idx, start_ms, end_ms, silent)
    pub fn pending_chunks(&self, id: i64) -> Result<Vec<(i64, i64, i64, bool)>, CoreError> {
        let mut st = self.db.conn.prepare("SELECT idx, start_ms, end_ms, silent FROM chunks WHERE meeting_id=?1 AND done=0 ORDER BY idx").map_err(e)?;
        let v = st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, i64>(3)? != 0))).map_err(e)?.collect::<Result<Vec<_>, _>>().map_err(e)?;
        Ok(v)
    }

    /// (処理済み, 全体)の区間数
    pub fn chunk_progress(&self, id: i64) -> Result<(i64, i64), CoreError> {
        self.db.conn.query_row("SELECT COALESCE(SUM(done),0), count(*) FROM chunks WHERE meeting_id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).map_err(e)
    }

    /// 1区間の結果を保存し、処理済みにする(途中で止まっても、区間の単位で続きから)。
    pub fn save_chunk(&self, id: i64, idx: i64, chunk_start_ms: i64, chunk_end_ms: i64, segs: &[crate::asr::AsrSegment]) -> Result<(), CoreError> {
        let glossary = self.glossary()?;
        let tx = self.db.conn.unchecked_transaction().map_err(e)?;
        tx.execute("DELETE FROM segments_fts WHERE rowid IN (SELECT id FROM segments WHERE meeting_id=?1 AND chunk_idx=?2)", (id, idx)).map_err(e)?;
        tx.execute("DELETE FROM segments WHERE meeting_id=?1 AND chunk_idx=?2", (id, idx)).map_err(e)?;
        for s in segs {
            let start = (chunk_start_ms + s.start_ms as i64).min(chunk_end_ms);
            let end = (chunk_start_ms + s.end_ms as i64).clamp(start, chunk_end_ms);
            let text = apply_glossary(&s.text, &glossary);
            tx.execute(
                "INSERT INTO segments(meeting_id, chunk_idx, start_ms, end_ms, text, raw_text, confidence) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                (id, idx, start, end, &text, &s.text, s.confidence as f64),
            )
            .map_err(e)?;
            let sid = tx.last_insert_rowid();
            tx.execute("INSERT INTO segments_fts(rowid, text) VALUES (?1, ?2)", (sid, fold_for_search(&text))).map_err(e)?;
        }
        tx.execute("UPDATE chunks SET done=1 WHERE meeting_id=?1 AND idx=?2", (id, idx)).map_err(e)?;
        tx.commit().map_err(e)
    }

    // ---------------- 文 ----------------

    pub fn segments(&self, id: i64) -> Result<Vec<Segment>, CoreError> {
        let mut st = self
            .db
            .conn
            .prepare("SELECT id, chunk_idx, start_ms, end_ms, speaker, text, raw_text, confidence, edited FROM segments WHERE meeting_id=?1 ORDER BY start_ms, id")
            .map_err(e)?;
        let v = st
            .query_map([id], |r| {
                Ok(Segment {
                    id: r.get(0)?, chunk_idx: r.get(1)?, start_ms: r.get(2)?, end_ms: r.get(3)?, speaker: r.get(4)?,
                    text: r.get(5)?, raw_text: r.get(6)?, confidence: r.get(7)?, edited: r.get::<_, i64>(8)? != 0,
                })
            })
            .map_err(e)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(e)?;
        Ok(v)
    }

    fn meeting_of_segment(&self, sid: i64) -> Result<i64, CoreError> {
        self.db.conn.query_row("SELECT meeting_id FROM segments WHERE id=?1", [sid], |r| r.get(0)).map_err(|_| CoreError::Db("文が見つかりません".into()))
    }

    /// 操作の前に、その議事録の文を控える(元に戻す用)。確定済みなら下書きに戻す。
    fn checkpoint(&self, mid: i64, label: &str) -> Result<(), CoreError> {
        let snap = serde_json::to_string(&self.segments(mid)?).map_err(|x| CoreError::Parse(x.to_string()))?;
        self.db
            .conn
            .execute("INSERT INTO segment_history(meeting_id, label, snapshot_json, at) VALUES (?1,?2,?3,?4)", (mid, label, snap, now()))
            .map_err(e)?;
        self.db
            .conn
            .execute(
                "DELETE FROM segment_history WHERE meeting_id=?1 AND id NOT IN (SELECT id FROM segment_history WHERE meeting_id=?1 ORDER BY id DESC LIMIT ?2)",
                (mid, HISTORY_KEEP),
            )
            .map_err(e)?;
        self.db.conn.execute("UPDATE meetings SET status='draft', updated_at=?2 WHERE id=?1", (mid, now())).map_err(e)?;
        Ok(())
    }

    fn replace_all(&self, mid: i64, segs: &[Segment]) -> Result<(), CoreError> {
        let tx = self.db.conn.unchecked_transaction().map_err(e)?;
        tx.execute("DELETE FROM segments_fts WHERE rowid IN (SELECT id FROM segments WHERE meeting_id=?1)", [mid]).map_err(e)?;
        tx.execute("DELETE FROM segments WHERE meeting_id=?1", [mid]).map_err(e)?;
        for s in segs {
            tx.execute(
                "INSERT INTO segments(id, meeting_id, chunk_idx, start_ms, end_ms, speaker, text, raw_text, confidence, edited) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                (s.id, mid, s.chunk_idx, s.start_ms, s.end_ms, &s.speaker, &s.text, &s.raw_text, s.confidence, s.edited as i64),
            )
            .map_err(e)?;
            tx.execute("INSERT INTO segments_fts(rowid, text) VALUES (?1, ?2)", (s.id, fold_for_search(&s.text))).map_err(e)?;
        }
        tx.commit().map_err(e)
    }

    /// 直前の操作を1つ元に戻す。戻したものがあれば true。
    pub fn undo(&self, mid: i64) -> Result<bool, CoreError> {
        let last: Option<(i64, String)> = self
            .db
            .conn
            .query_row("SELECT id, snapshot_json FROM segment_history WHERE meeting_id=?1 ORDER BY id DESC LIMIT 1", [mid], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()
            .map_err(e)?;
        let Some((hid, snap)) = last else { return Ok(false) };
        let segs: Vec<Segment> = serde_json::from_str(&snap).map_err(|x| CoreError::Parse(x.to_string()))?;
        self.replace_all(mid, &segs)?;
        self.db.conn.execute("DELETE FROM segment_history WHERE id=?1", [hid]).map_err(e)?;
        self.db.conn.execute("UPDATE meetings SET status='draft', updated_at=?2 WHERE id=?1", (mid, now())).map_err(e)?;
        Ok(true)
    }

    pub fn can_undo(&self, mid: i64) -> Result<bool, CoreError> {
        self.db.conn.query_row("SELECT count(*) FROM segment_history WHERE meeting_id=?1", [mid], |r| r.get::<_, i64>(0)).map(|n| n > 0).map_err(e)
    }

    pub fn edit_text(&self, sid: i64, text: &str) -> Result<i64, CoreError> {
        let mid = self.meeting_of_segment(sid)?;
        self.checkpoint(mid, "文の修正")?;
        self.db.conn.execute("UPDATE segments SET text=?2, edited=1 WHERE id=?1", (sid, text)).map_err(e)?;
        self.db.conn.execute("DELETE FROM segments_fts WHERE rowid=?1", [sid]).map_err(e)?;
        self.db.conn.execute("INSERT INTO segments_fts(rowid, text) VALUES (?1, ?2)", (sid, fold_for_search(text))).map_err(e)?;
        Ok(mid)
    }

    /// 話者のラベルを付ける。`following` が true なら、この文から次に別の話者が付いている文の手前まで、まとめて付ける。
    pub fn set_speaker(&self, sid: i64, speaker: &str, following: bool) -> Result<i64, CoreError> {
        let mid = self.meeting_of_segment(sid)?;
        self.checkpoint(mid, "話者")?;
        let segs = self.segments(mid)?;
        let pos = segs.iter().position(|s| s.id == sid).unwrap_or(0);
        let old = segs[pos].speaker.clone();
        for s in &segs[pos..] {
            if s.id != sid && (!following || s.speaker != old) {
                break;
            }
            self.db.conn.execute("UPDATE segments SET speaker=?2 WHERE id=?1", (s.id, speaker.trim())).map_err(e)?;
        }
        Ok(mid)
    }

    /// 次の文と結合する。
    pub fn merge_next(&self, sid: i64) -> Result<i64, CoreError> {
        let mid = self.meeting_of_segment(sid)?;
        let segs = self.segments(mid)?;
        let pos = segs.iter().position(|s| s.id == sid).ok_or_else(|| CoreError::Db("文が見つかりません".into()))?;
        let Some(next) = segs.get(pos + 1) else { return Err(CoreError::Db("次の文がありません".into())) };
        let cur = &segs[pos];
        self.checkpoint(mid, "結合")?;
        let mut merged = cur.clone();
        merged.end_ms = next.end_ms;
        merged.text = format!("{}{}", cur.text, next.text);
        merged.raw_text = format!("{}{}", cur.raw_text, next.raw_text);
        merged.confidence = cur.confidence.min(next.confidence);
        merged.edited = true;
        let mut all: Vec<Segment> = segs.iter().filter(|s| s.id != next.id).cloned().collect();
        all[pos] = merged;
        self.replace_all(mid, &all)?;
        Ok(mid)
    }

    /// `at`(文字数)の位置で2つに分ける。時刻は文字数の比で割り振る(目安)。
    pub fn split(&self, sid: i64, at: usize) -> Result<i64, CoreError> {
        let mid = self.meeting_of_segment(sid)?;
        let segs = self.segments(mid)?;
        let pos = segs.iter().position(|s| s.id == sid).ok_or_else(|| CoreError::Db("文が見つかりません".into()))?;
        let cur = &segs[pos];
        let chars: Vec<char> = cur.text.chars().collect();
        if at == 0 || at >= chars.len() {
            return Err(CoreError::Db("分ける位置は文の途中にしてください".into()));
        }
        self.checkpoint(mid, "分割")?;
        let t = cur.start_ms + (cur.end_ms - cur.start_ms) * at as i64 / chars.len() as i64;
        let mut a = cur.clone();
        a.text = chars[..at].iter().collect();
        a.end_ms = t;
        a.edited = true;
        let mut b = cur.clone();
        b.id = self.db.conn.query_row("SELECT COALESCE(MAX(id),0)+1 FROM segments", [], |r| r.get(0)).map_err(e)?;
        b.text = chars[at..].iter().collect();
        b.start_ms = t;
        b.edited = true;
        // 元の文字起こし(raw_text)は前半の文に残し、後半は空にする(元に戻す用の控えは履歴にある)
        b.raw_text = String::new();
        let mut all = segs.clone();
        all[pos] = a;
        all.insert(pos + 1, b);
        self.replace_all(mid, &all)?;
        Ok(mid)
    }

    /// 文を、文字起こしの結果そのまま(+用語辞書)に戻す。
    pub fn revert_segment(&self, sid: i64) -> Result<i64, CoreError> {
        let mid = self.meeting_of_segment(sid)?;
        let raw: String = self.db.conn.query_row("SELECT raw_text FROM segments WHERE id=?1", [sid], |r| r.get(0)).map_err(e)?;
        let text = apply_glossary(&raw, &self.glossary()?);
        self.checkpoint(mid, "元に戻す(文)")?;
        self.db.conn.execute("UPDATE segments SET text=?2, edited=0 WHERE id=?1", (sid, &text)).map_err(e)?;
        self.db.conn.execute("DELETE FROM segments_fts WHERE rowid=?1", [sid]).map_err(e)?;
        self.db.conn.execute("INSERT INTO segments_fts(rowid, text) VALUES (?1, ?2)", (sid, fold_for_search(&text))).map_err(e)?;
        Ok(mid)
    }

    /// 用語辞書を、手で直していない文に適用し直す(元に戻せる)。置き換えた文の数を返す。
    pub fn reapply_glossary(&self, mid: i64) -> Result<usize, CoreError> {
        let g = self.glossary()?;
        let segs = self.segments(mid)?;
        let mut changed = 0;
        let new: Vec<Segment> = segs
            .iter()
            .map(|s| {
                let mut s = s.clone();
                if !s.edited {
                    let t = apply_glossary(&s.raw_text, &g);
                    if t != s.text {
                        s.text = t;
                        changed += 1;
                    }
                }
                s
            })
            .collect();
        if changed > 0 {
            self.checkpoint(mid, "用語辞書")?;
            self.replace_all(mid, &new)?;
        }
        Ok(changed)
    }

    // ---------------- 用語辞書 ----------------

    pub fn glossary(&self) -> Result<Vec<GlossaryEntry>, CoreError> {
        let mut st = self.db.conn.prepare("SELECT id, wrong, right FROM glossary ORDER BY id").map_err(e)?;
        let v = st.query_map([], |r| Ok(GlossaryEntry { id: r.get(0)?, wrong: r.get(1)?, right: r.get(2)? })).map_err(e)?.collect::<Result<Vec<_>, _>>().map_err(e)?;
        Ok(v)
    }

    pub fn add_glossary(&self, wrong: &str, right: &str) -> Result<(), CoreError> {
        let (w, r) = (wrong.trim(), right.trim());
        if w.is_empty() || r.is_empty() {
            return Err(CoreError::Db("誤りやすい表記と正しい表記の両方を入力してください".into()));
        }
        self.db
            .conn
            .execute("INSERT INTO glossary(wrong, right) VALUES (?1,?2) ON CONFLICT(wrong) DO UPDATE SET right=excluded.right", (w, r))
            .map_err(e)?;
        Ok(())
    }

    pub fn delete_glossary(&self, id: i64) -> Result<(), CoreError> {
        self.db.conn.execute("DELETE FROM glossary WHERE id=?1", [id]).map_err(e)?;
        Ok(())
    }

    /// 文字起こしのヒント(正しい表記を並べたもの。長すぎないよう 200 文字まで)
    pub fn glossary_hint(&self) -> Result<String, CoreError> {
        let mut s = String::new();
        for g in self.glossary()? {
            if s.chars().count() + g.right.chars().count() > 200 {
                break;
            }
            if !s.is_empty() {
                s.push('、');
            }
            s.push_str(&g.right);
        }
        Ok(s)
    }

    // ---------------- 検索 ----------------

    pub fn search(&self, query: &str) -> Result<Vec<SearchHit>, CoreError> {
        if query.trim().is_empty() {
            return Ok(vec![]);
        }
        let ids = self.db.search_rowids("segments_fts", &["text"], query, 500)?;
        let mut out = Vec::new();
        for sid in ids {
            if let Some(h) = self
                .db
                .conn
                .query_row(
                    "SELECT m.id, m.title, m.held_on, s.id, s.start_ms, s.text FROM segments s JOIN meetings m ON m.id = s.meeting_id WHERE s.id=?1",
                    [sid],
                    |r| Ok(SearchHit { meeting_id: r.get(0)?, title: r.get(1)?, held_on: r.get(2)?, segment_id: r.get(3)?, start_ms: r.get(4)?, text: r.get(5)? }),
                )
                .optional()
                .map_err(e)?
            {
                out.push(h);
            }
        }
        out.sort_by(|a, b| b.held_on.cmp(&a.held_on).then(a.meeting_id.cmp(&b.meeting_id)).then(a.start_ms.cmp(&b.start_ms)));
        Ok(out)
    }
}

pub fn is_valid_date(s: &str) -> bool {
    let p: Vec<&str> = s.split('-').collect();
    if p.len() != 3 || p[0].len() != 4 || p[1].len() != 2 || p[2].len() != 2 {
        return false;
    }
    let (Ok(y), Ok(m), Ok(d)) = (p[0].parse::<i32>(), p[1].parse::<u32>(), p[2].parse::<u32>()) else { return false };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    (1..=12).contains(&m) && d >= 1 && d <= days[(m - 1) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::AsrSegment;
    use crate::audio::Chunk;

    fn seg(s: u64, e: u64, t: &str) -> AsrSegment {
        AsrSegment { start_ms: s, end_ms: e, text: t.into(), confidence: 0.9 }
    }

    fn setup() -> (Store, i64) {
        let st = Store::open_in_memory().unwrap();
        let id = st.add_meeting("定例会議", "a.m4a", "/tmp/a.m4a", true).unwrap();
        st.set_chunks(id, &[Chunk { start_ms: 0, end_ms: 30_000, silent: false }, Chunk { start_ms: 30_000, end_ms: 50_000, silent: false }]).unwrap();
        (st, id)
    }

    #[test]
    fn 用語辞書は長い語を優先し_置換結果を再置換しない() {
        let g = vec![
            GlossaryEntry { id: 1, wrong: "でぃーぷらーにんぐ".into(), right: "ディープラーニング".into() },
            GlossaryEntry { id: 2, wrong: "らーにんぐ".into(), right: "学習".into() },
            GlossaryEntry { id: 3, wrong: "学習".into(), right: "がくしゅう".into() },
        ];
        assert_eq!(apply_glossary("でぃーぷらーにんぐとらーにんぐ", &g), "ディープラーニングと学習");
    }

    #[test]
    fn 区間を保存すると時刻が全体の時刻になり_再保存で重複しない() {
        let (st, id) = setup();
        st.add_glossary("やまだ商事", "山田商事").unwrap();
        st.save_chunk(id, 1, 30_000, 50_000, &[seg(1_000, 4_000, "やまだ商事の件です"), seg(4_000, 99_000, "以上")]).unwrap();
        st.save_chunk(id, 1, 30_000, 50_000, &[seg(1_000, 4_000, "やまだ商事の件です"), seg(4_000, 99_000, "以上")]).unwrap();
        let s = st.segments(id).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].start_ms, s[0].end_ms), (31_000, 34_000));
        assert_eq!(s[1].end_ms, 50_000); // 区間の終わりを超えない
        assert_eq!((s[0].text.as_str(), s[0].raw_text.as_str()), ("山田商事の件です", "やまだ商事の件です"));
        assert_eq!(st.pending_chunks(id).unwrap().len(), 1);
        assert_eq!(st.chunk_progress(id).unwrap(), (1, 2));
        assert_eq!(st.search("山田商事").unwrap().len(), 1);
    }

    #[test]
    fn 修正_結合_分割_話者は元に戻せる() {
        let (st, id) = setup();
        st.save_chunk(id, 0, 0, 30_000, &[seg(0, 2_000, "こんにちは"), seg(2_000, 6_000, "本日の議題です"), seg(6_000, 9_000, "はい")]).unwrap();
        let ids: Vec<i64> = st.segments(id).unwrap().iter().map(|s| s.id).collect();
        st.edit_text(ids[0], "こんにちは。").unwrap();
        st.merge_next(ids[1]).unwrap();
        let s = st.segments(id).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!((s[1].text.as_str(), s[1].end_ms), ("本日の議題ですはい", 9_000));
        st.split(s[1].id, 7).unwrap();
        let s = st.segments(id).unwrap();
        assert_eq!(s.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(), ["こんにちは。", "本日の議題です", "はい"]);
        st.set_speaker(s[0].id, "佐藤", true).unwrap();
        assert!(st.segments(id).unwrap().iter().all(|x| x.speaker == "佐藤")); // 空の話者が続く範囲にまとめて付く
        st.set_speaker(s[2].id, "鈴木", false).unwrap();
        assert_eq!(st.segments(id).unwrap()[2].speaker, "鈴木");
        assert_eq!(st.search("議題").unwrap().len(), 1);
        // 4回戻すと、修正前に戻る
        for _ in 0..4 {
            assert!(st.undo(id).unwrap());
        }
        let s = st.segments(id).unwrap();
        assert_eq!(s.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(), ["こんにちは。", "本日の議題です", "はい"]);
        assert!(st.undo(id).unwrap()); // 文の修正
        assert_eq!(st.segments(id).unwrap()[0].text, "こんにちは");
        assert!(!st.undo(id).unwrap());
    }

    #[test]
    fn 辞書の再適用は手で直した文を変えず_元に戻せる() {
        let (st, id) = setup();
        st.save_chunk(id, 0, 0, 30_000, &[seg(0, 2_000, "くらうど の話"), seg(2_000, 4_000, "くらうど です")]).unwrap();
        let ids: Vec<i64> = st.segments(id).unwrap().iter().map(|s| s.id).collect();
        st.edit_text(ids[1], "手で直した").unwrap();
        st.add_glossary("くらうど", "クラウド").unwrap();
        assert_eq!(st.reapply_glossary(id).unwrap(), 1);
        let s = st.segments(id).unwrap();
        assert_eq!((s[0].text.as_str(), s[1].text.as_str()), ("クラウド の話", "手で直した"));
        st.undo(id).unwrap();
        assert_eq!(st.segments(id).unwrap()[0].text, "くらうど の話");
        st.revert_segment(ids[1]).unwrap();
        assert_eq!(st.segments(id).unwrap()[1].text, "クラウド です");
    }

    #[test]
    fn 確定は処理が終わってから_編集すると下書きに戻る() {
        let (st, id) = setup();
        assert!(st.confirm(id).is_err());
        st.save_chunk(id, 0, 0, 30_000, &[seg(0, 1_000, "a")]).unwrap();
        st.set_state(id, "done", None).unwrap();
        st.confirm(id).unwrap();
        assert_eq!(st.meeting(id).unwrap().unwrap().status, "confirmed");
        let sid = st.segments(id).unwrap()[0].id;
        st.edit_text(sid, "b").unwrap();
        assert_eq!(st.meeting(id).unwrap().unwrap().status, "draft");
        assert!(st.update_meta(id, "x", Some("2026-02-30"), "").is_err());
        st.update_meta(id, "x", Some("2026-02-28"), "佐藤、鈴木").unwrap();
    }

    #[test]
    fn 削除で文と検索から消える() {
        let (st, id) = setup();
        st.save_chunk(id, 0, 0, 30_000, &[seg(0, 1_000, "削除される文です")]).unwrap();
        let paths = st.delete_meeting(id).unwrap();
        assert_eq!(paths, vec!["/tmp/a.m4a".to_string()]);
        assert!(st.search("削除される").unwrap().is_empty());
        assert!(st.meetings().unwrap().is_empty());
    }
}
