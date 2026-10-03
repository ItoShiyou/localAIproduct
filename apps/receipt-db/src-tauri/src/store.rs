//! データベース(SQLite)。取り込んだ書類、読み取り結果(下書き/確定済み)、検索、編集(元に戻す付き)、削除、書き出し。
//!
//! 「確定済み」になるのは `confirm` を通したものだけ。下書きは一覧に出るが、CSV・SQLite の書き出しには入らない。

use crate::journal::{default_rules, suggest_journal};
use crate::receipt::Receipt;
use factory_core::db::Db;
use factory_core::export::{write_csv, Cell, CsvEncoding};
use factory_core::jobs::JOBS_SCHEMA;
use factory_core::CoreError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const MIGRATIONS: [&str; 2] = [
    "CREATE TABLE documents(
        id INTEGER PRIMARY KEY, sha256 TEXT NOT NULL UNIQUE, original_name TEXT NOT NULL,
        stored_path TEXT NOT NULL, kind TEXT NOT NULL, imported_at INTEGER NOT NULL);
     CREATE TABLE receipts(
        id INTEGER PRIMARY KEY, document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
        page INTEGER NOT NULL DEFAULT 1, date TEXT, vendor TEXT, total INTEGER, tax_rate INTEGER,
        invoice_no TEXT, summary TEXT, account_candidate TEXT,
        status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','confirmed')),
        field_confidence_json TEXT NOT NULL DEFAULT '{}', created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
     CREATE INDEX receipts_status_date ON receipts(status, date);
     CREATE VIRTUAL TABLE receipts_fts USING fts5(vendor, summary, invoice_no, tokenize='trigram');
     CREATE TABLE receipt_edits(
        id INTEGER PRIMARY KEY, receipt_id INTEGER NOT NULL REFERENCES receipts(id) ON DELETE CASCADE,
        field TEXT NOT NULL, old_value TEXT, new_value TEXT, at INTEGER NOT NULL);",
    JOBS_SCHEMA,
];

fn e<T: std::fmt::Display>(x: T) -> CoreError {
    CoreError::Db(x.to_string())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub id: i64,
    pub document_id: i64,
    pub receipt: Receipt,
    pub account_candidate: Option<String>,
    pub status: String,
    pub confidence: BTreeMap<String, f32>,
}

#[derive(Debug, Clone, Default)]
pub struct Search {
    pub text: String,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub min_total: Option<i64>,
    pub max_total: Option<i64>,
    pub status: Option<String>,
}

pub struct Store {
    pub db: Db,
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

const FIELDS: [&str; 6] = ["date", "vendor", "total", "tax_rate", "invoice_no", "summary"];

impl Store {
    pub fn open(path: &Path) -> Result<Self, CoreError> {
        Ok(Self { db: Db::open(path, &MIGRATIONS)? })
    }
    pub fn open_in_memory() -> Result<Self, CoreError> {
        Ok(Self { db: Db::open_in_memory(&MIGRATIONS)? })
    }

    /// 書類を登録する。同じ SHA-256 があれば `Ok(None)`(重複)。
    pub fn add_document(&self, sha256: &str, original_name: &str, stored_path: &str, kind: &str) -> Result<Option<i64>, CoreError> {
        let n = self
            .db
            .conn
            .execute(
                "INSERT OR IGNORE INTO documents(sha256, original_name, stored_path, kind, imported_at) VALUES (?1,?2,?3,?4,?5)",
                (sha256, original_name, stored_path, kind, now()),
            )
            .map_err(e)?;
        Ok((n == 1).then(|| self.db.conn.last_insert_rowid()))
    }

    pub fn has_document(&self, sha256: &str) -> Result<bool, CoreError> {
        self.db.conn.query_row("SELECT count(*) FROM documents WHERE sha256=?1", [sha256], |r| r.get::<_, i64>(0)).map(|n| n > 0).map_err(e)
    }

    fn fts_put(&self, id: i64, r: &Receipt) -> Result<(), CoreError> {
        use factory_core::db::fold_for_search as f;
        self.db.conn.execute("DELETE FROM receipts_fts WHERE rowid=?1", [id]).map_err(e)?;
        self.db
            .conn
            .execute(
                "INSERT INTO receipts_fts(rowid, vendor, summary, invoice_no) VALUES (?1,?2,?3,?4)",
                (id, f(r.vendor.as_deref().unwrap_or("")), f(r.summary.as_deref().unwrap_or("")), f(r.invoice_no.as_deref().unwrap_or(""))),
            )
            .map_err(e)?;
        Ok(())
    }

    /// 読み取り結果を「下書き」で保存する。勘定科目の候補は、支払先・摘要のキーワードから付ける(確定はしない)。
    pub fn add_draft(&self, document_id: i64, page: i64, r: &Receipt, confidence: &BTreeMap<String, f32>) -> Result<i64, CoreError> {
        let account = suggest_journal(r, &default_rules(), "現金").debit_account;
        let account = (account != crate::journal::UNDECIDED).then_some(account);
        self.db
            .conn
            .execute(
                "INSERT INTO receipts(document_id,page,date,vendor,total,tax_rate,invoice_no,summary,account_candidate,field_confidence_json,created_at,updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?11)",
                (
                    document_id, page, &r.date, &r.vendor, r.total, r.tax_rate.map(|x| x as i64), &r.invoice_no, &r.summary, account,
                    serde_json::to_string(confidence).map_err(|x| CoreError::Parse(x.to_string()))?, now(),
                ),
            )
            .map_err(e)?;
        let id = self.db.conn.last_insert_rowid();
        self.fts_put(id, r)?;
        Ok(id)
    }

    fn row_of(r: &rusqlite::Row) -> rusqlite::Result<Row> {
        let conf: String = r.get(10)?;
        Ok(Row {
            id: r.get(0)?,
            document_id: r.get(1)?,
            receipt: Receipt {
                date: r.get(2)?, vendor: r.get(3)?, total: r.get(4)?, tax_rate: r.get::<_, Option<i64>>(5)?.map(|x| x as u8),
                invoice_no: r.get(6)?, summary: r.get(7)?,
            },
            account_candidate: r.get(8)?,
            status: r.get(9)?,
            confidence: serde_json::from_str(&conf).unwrap_or_default(),
        })
    }

    const COLS: &'static str = "id, document_id, date, vendor, total, tax_rate, invoice_no, summary, account_candidate, status, field_confidence_json";

    pub fn get(&self, id: i64) -> Result<Option<Row>, CoreError> {
        use rusqlite::OptionalExtension;
        self.db.conn.query_row(&format!("SELECT {} FROM receipts WHERE id=?1", Self::COLS), [id], Self::row_of).optional().map_err(e)
    }

    /// 一覧と検索。`text` は空白区切りで AND(3文字以上は全文検索、未満は部分一致)。新しい日付順。
    pub fn search(&self, q: &Search) -> Result<Vec<Row>, CoreError> {
        let ids: Option<Vec<i64>> = if q.text.trim().is_empty() {
            None
        } else {
            Some(self.db.search_rowids("receipts_fts", &["vendor", "summary", "invoice_no"], &q.text, 100_000)?)
        };
        let mut stmt = self
            .db
            .conn
            .prepare(&format!(
                "SELECT {} FROM receipts WHERE (?1 IS NULL OR date >= ?1) AND (?2 IS NULL OR date <= ?2)
                 AND (?3 IS NULL OR total >= ?3) AND (?4 IS NULL OR total <= ?4) AND (?5 IS NULL OR status = ?5)
                 ORDER BY date DESC, id DESC",
                Self::COLS
            ))
            .map_err(e)?;
        let rows = stmt
            .query_map((&q.date_from, &q.date_to, q.min_total, q.max_total, &q.status), Self::row_of)
            .map_err(e)?;
        let mut out = Vec::new();
        for r in rows {
            let r = r.map_err(e)?;
            if ids.as_ref().map(|v| v.contains(&r.id)).unwrap_or(true) {
                out.push(r);
            }
        }
        Ok(out)
    }

    /// 1項目を直す。直す前の値を記録し、`undo` で戻せる。確定済みを直すと下書きに戻る(再確認が必要)。
    pub fn edit(&self, id: i64, field: &str, value: Option<&str>) -> Result<(), CoreError> {
        if !FIELDS.contains(&field) {
            return Err(CoreError::Db("直せない項目です".into()));
        }
        let old: Option<String> = self
            .db
            .conn
            .query_row(&format!("SELECT CAST({field} AS TEXT) FROM receipts WHERE id=?1"), [id], |r| r.get(0))
            .map_err(e)?;
        self.set_field(id, field, value)?;
        self.db
            .conn
            .execute("INSERT INTO receipt_edits(receipt_id, field, old_value, new_value, at) VALUES (?1,?2,?3,?4,?5)", (id, field, old, value, now()))
            .map_err(e)?;
        Ok(())
    }

    fn set_field(&self, id: i64, field: &str, value: Option<&str>) -> Result<(), CoreError> {
        let v: rusqlite::types::Value = match (field, value) {
            (_, None) => rusqlite::types::Value::Null,
            ("total" | "tax_rate", Some(s)) => match s.trim().replace(',', "").parse::<i64>() {
                Ok(n) => rusqlite::types::Value::Integer(n),
                Err(_) => return Err(CoreError::Db("数値で入力してください".into())),
            },
            (_, Some(s)) => rusqlite::types::Value::Text(s.trim().to_string()),
        };
        self.db
            .conn
            .execute(&format!("UPDATE receipts SET {field}=?2, status='draft', updated_at=?3 WHERE id=?1"), (id, v, now()))
            .map_err(e)?;
        let row = self.get(id)?.ok_or_else(|| CoreError::Db("見つかりません".into()))?;
        self.fts_put(id, &row.receipt)
    }

    /// 直近の修正を1つ元に戻す。戻したものがあれば true。
    pub fn undo(&self, id: i64) -> Result<bool, CoreError> {
        use rusqlite::OptionalExtension;
        let last: Option<(i64, String, Option<String>)> = self
            .db
            .conn
            .query_row("SELECT id, field, old_value FROM receipt_edits WHERE receipt_id=?1 ORDER BY id DESC LIMIT 1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()
            .map_err(e)?;
        let Some((eid, field, old)) = last else { return Ok(false) };
        self.set_field(id, &field, old.as_deref())?;
        self.db.conn.execute("DELETE FROM receipt_edits WHERE id=?1", [eid]).map_err(e)?;
        Ok(true)
    }

    /// 確定する(一括可)。検証に問題がある行は確定せず、(id, 指摘の項目名)で返す。
    /// 確定は利用者が確認画面で操作したときだけ呼ぶこと。
    pub fn confirm(&self, ids: &[i64]) -> Result<Vec<(i64, Vec<&'static str>)>, CoreError> {
        let mut rejected = Vec::new();
        for &id in ids {
            let Some(row) = self.get(id)? else { continue };
            let issues: Vec<&'static str> = row.receipt.validate().into_iter().map(|i| i.field).collect();
            if issues.is_empty() {
                self.db.conn.execute("UPDATE receipts SET status='confirmed', updated_at=?2 WHERE id=?1", (id, now())).map_err(e)?;
                self.db.conn.execute("DELETE FROM receipt_edits WHERE receipt_id=?1", [id]).map_err(e)?;
            } else {
                rejected.push((id, issues));
            }
        }
        Ok(rejected)
    }

    pub fn unconfirm(&self, id: i64) -> Result<(), CoreError> {
        self.db.conn.execute("UPDATE receipts SET status='draft', updated_at=?2 WHERE id=?1", (id, now())).map_err(e)?;
        Ok(())
    }

    /// 読み取り結果を消す。その書類に読み取り結果が残らなければ、書類の記録と原本のコピーも消す。消したコピーの数を返す。
    pub fn delete_receipt(&self, id: i64) -> Result<usize, CoreError> {
        let Some(row) = self.get(id)? else { return Ok(0) };
        self.db.conn.execute("DELETE FROM receipts_fts WHERE rowid=?1", [id]).map_err(e)?;
        self.db.conn.execute("DELETE FROM receipts WHERE id=?1", [id]).map_err(e)?;
        let left: i64 = self.db.conn.query_row("SELECT count(*) FROM receipts WHERE document_id=?1", [row.document_id], |r| r.get(0)).map_err(e)?;
        if left > 0 {
            return Ok(0);
        }
        let path: String = self.db.conn.query_row("SELECT stored_path FROM documents WHERE id=?1", [row.document_id], |r| r.get(0)).map_err(e)?;
        self.db.conn.execute("DELETE FROM documents WHERE id=?1", [row.document_id]).map_err(e)?;
        Ok(std::fs::remove_file(&path).is_ok() as usize)
    }

    /// CSV(確定済みだけ)。
    pub fn export_csv(&self, enc: CsvEncoding) -> Result<Vec<u8>, CoreError> {
        let rows = self.search(&Search { status: Some("confirmed".into()), ..Default::default() })?;
        let body: Vec<Vec<Cell>> = rows
            .iter()
            .map(|r| {
                let t = &r.receipt;
                vec![
                    t.date.clone().unwrap_or_default().into(),
                    t.vendor.clone().unwrap_or_default().into(),
                    t.total.map(Cell::Int).unwrap_or_else(|| "".into()),
                    t.tax_rate.map(|x| x.to_string()).unwrap_or_default().into(),
                    t.invoice_no.clone().unwrap_or_default().into(),
                    t.summary.clone().unwrap_or_default().into(),
                    r.account_candidate.clone().unwrap_or_default().into(),
                ]
            })
            .collect();
        write_csv(&["日付", "支払先", "税込合計", "税率", "登録番号", "摘要", "勘定科目(候補)"], &body, enc)
    }

    /// SQLite ファイルとして書き出す(確定済みの行だけ。下書きと原本のパスは含めない)。
    pub fn export_sqlite(&self, dest: &Path) -> Result<(), CoreError> {
        if dest.exists() {
            return Err(CoreError::Db("書き出し先に同名のファイルがあります".into()));
        }
        let conn = rusqlite::Connection::open(dest).map_err(e)?;
        conn.execute_batch("CREATE TABLE receipts(id INTEGER PRIMARY KEY, date TEXT, vendor TEXT, total INTEGER, tax_rate INTEGER, invoice_no TEXT, summary TEXT, account_candidate TEXT)").map_err(e)?;
        for r in self.search(&Search { status: Some("confirmed".into()), ..Default::default() })? {
            let t = r.receipt;
            conn.execute(
                "INSERT INTO receipts VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                (r.id, t.date, t.vendor, t.total, t.tax_rate.map(|x| x as i64), t.invoice_no, t.summary, r.account_candidate),
            )
            .map_err(e)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rc(date: &str, vendor: &str, total: i64) -> Receipt {
        Receipt { date: Some(date.into()), vendor: Some(vendor.into()), total: Some(total), tax_rate: Some(10), invoice_no: Some("T1234567890123".into()), summary: Some("コピー用紙".into()) }
    }

    fn seeded() -> (Store, Vec<i64>) {
        let s = Store::open_in_memory().unwrap();
        let mut ids = vec![];
        for (i, (d, v, t)) in [("2026-01-05", "株式会社ひまわり文具", 1100), ("2026-02-10", "有限会社さくら書店", 5500), ("2026-03-15", "=HYPERLINK(\"x\")商店", 330)].iter().enumerate() {
            let doc = s.add_document(&format!("sha{i}"), "a.pdf", &format!("/nonexistent/{i}"), "pdf").unwrap().unwrap();
            ids.push(s.add_draft(doc, 1, &rc(d, v, *t), &BTreeMap::new()).unwrap());
        }
        (s, ids)
    }

    #[test]
    fn 同じsha256は重複として取り込まない() {
        let s = Store::open_in_memory().unwrap();
        assert!(s.add_document("x", "a", "p", "pdf").unwrap().is_some());
        assert!(s.add_document("x", "b", "q", "pdf").unwrap().is_none());
        assert!(s.has_document("x").unwrap());
    }

    #[test]
    fn 検索_日付_金額_支払先_二文字の語() {
        let (s, _) = seeded();
        let q = |t: &str| Search { text: t.into(), ..Default::default() };
        assert_eq!(s.search(&q("")).unwrap().len(), 3);
        assert_eq!(s.search(&q("書店")).unwrap().len(), 1); // 2文字
        assert_eq!(s.search(&q("ひまわり")).unwrap().len(), 1);
        assert_eq!(s.search(&q("T1234567890123")).unwrap().len(), 3);
        let r = s.search(&Search { date_from: Some("2026-02-01".into()), min_total: Some(1000), ..Default::default() }).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].receipt.total, Some(5500));
        assert_eq!(s.search(&Search::default()).unwrap()[0].receipt.date.as_deref(), Some("2026-03-15")); // 新しい順
    }

    #[test]
    fn 確定するまで確定済みにならず_書き出しにも出ない() {
        let (s, ids) = seeded();
        assert!(s.search(&Search { status: Some("confirmed".into()), ..Default::default() }).unwrap().is_empty());
        let csv = String::from_utf8(s.export_csv(CsvEncoding::Utf8Bom).unwrap()[3..].to_vec()).unwrap();
        assert_eq!(csv.lines().count(), 1); // ヘッダだけ
        assert!(s.confirm(&ids[..2]).unwrap().is_empty());
        let csv = String::from_utf8(s.export_csv(CsvEncoding::Utf8Bom).unwrap()[3..].to_vec()).unwrap();
        assert_eq!(csv.lines().count(), 3);
        assert!(csv.contains("2026-01-05,株式会社ひまわり文具,1100,10,T1234567890123,コピー用紙"));
    }

    #[test]
    fn 問題のある行は一括確定でも確定されない() {
        let (s, ids) = seeded();
        s.edit(ids[0], "date", None).unwrap();
        let rejected = s.confirm(&ids).unwrap();
        assert_eq!(rejected, vec![(ids[0], vec!["date"])]);
        assert_eq!(s.get(ids[0]).unwrap().unwrap().status, "draft");
        assert_eq!(s.get(ids[1]).unwrap().unwrap().status, "confirmed");
    }

    #[test]
    fn 修正して元に戻せる_検索にも反映_確定済みを直すと下書きに戻る() {
        let (s, ids) = seeded();
        s.confirm(&[ids[0]]).unwrap();
        s.edit(ids[0], "vendor", Some("株式会社あおぞら食品")).unwrap();
        s.edit(ids[0], "total", Some("2,200")).unwrap();
        let r = s.get(ids[0]).unwrap().unwrap();
        assert_eq!((r.status.as_str(), r.receipt.total), ("draft", Some(2200)));
        assert_eq!(s.search(&Search { text: "あおぞら".into(), ..Default::default() }).unwrap().len(), 1);
        assert!(s.undo(ids[0]).unwrap() && s.undo(ids[0]).unwrap());
        assert!(!s.undo(ids[0]).unwrap());
        let r = s.get(ids[0]).unwrap().unwrap();
        assert_eq!((r.receipt.vendor.as_deref(), r.receipt.total), (Some("株式会社ひまわり文具"), Some(1100)));
        assert_eq!(s.search(&Search { text: "あおぞら".into(), ..Default::default() }).unwrap().len(), 0);
        assert!(s.edit(ids[0], "total", Some("abc")).is_err());
        assert!(s.edit(ids[0], "id; DROP", Some("1")).is_err());
    }

    #[test]
    fn 削除すると原本のコピーも消える() {
        let dir = std::env::temp_dir().join(format!("rdb-del-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("o.pdf");
        std::fs::write(&f, "x").unwrap();
        let s = Store::open_in_memory().unwrap();
        let doc = s.add_document("h", "o.pdf", f.to_str().unwrap(), "pdf").unwrap().unwrap();
        let id = s.add_draft(doc, 1, &rc("2026-01-01", "店", 100), &BTreeMap::new()).unwrap();
        assert_eq!(s.delete_receipt(id).unwrap(), 1);
        assert!(!f.exists() && !s.has_document("h").unwrap());
        assert_eq!(s.search(&Search { text: "コピー用紙".into(), ..Default::default() }).unwrap().len(), 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn csvの数式は無害化_勘定科目の候補が付く_sqlite書き出しは確定のみ() {
        let (s, ids) = seeded();
        s.confirm(&ids).unwrap();
        let csv = String::from_utf8(s.export_csv(CsvEncoding::Utf8Bom).unwrap()[3..].to_vec()).unwrap();
        assert!(csv.contains("'=HYPERLINK"));
        assert!(!csv.contains(",=HYPERLINK"));
        let dest = std::env::temp_dir().join(format!("rdb-exp-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&dest);
        s.export_sqlite(&dest).unwrap();
        let c = rusqlite::Connection::open(&dest).unwrap();
        assert_eq!(c.query_row("SELECT count(*) FROM receipts", [], |r| r.get::<_, i64>(0)).unwrap(), 3);
        assert!(s.export_sqlite(&dest).is_err());
        std::fs::remove_file(&dest).ok();
    }
}
