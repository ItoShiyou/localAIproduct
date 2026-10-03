//! SQLite を開く・マイグレーション・全文検索。
//!
//! - マイグレーションは `PRAGMA user_version` で管理する。アプリが `&[&str]`(上から順に適用する SQL)を渡す。
//!   途中で失敗したら、そのステップだけ巻き戻して `Err`(データは壊さない)。
//! - 全文検索は FTS5 の `trigram` トークナイザ(日本語の部分一致が使える)。**3文字未満の語は
//!   trigram に当たらない**(実測: 「書店」「文具」「会社」は 0 件)ので、その語だけ `LIKE` に切り替える。
//! - 検索語と保存する文字列は、`fold_for_search`(NFKC: 全角英数・半角カナをそろえる)を通す。

use crate::error::CoreError;
use rusqlite::{params_from_iter, Connection};
use std::path::Path;
use unicode_normalization::UnicodeNormalization;

fn db_err<E: std::fmt::Display>(e: E) -> CoreError {
    CoreError::Db(e.to_string())
}

pub struct Db {
    pub conn: Connection,
}

/// 検索用に文字をそろえる(NFKC)。インデックスに入れる文字列と検索語の両方に使う。
pub fn fold_for_search(s: &str) -> String {
    s.nfkc().collect()
}

impl Db {
    pub fn open(path: &Path, migrations: &[&str]) -> Result<Self, CoreError> {
        Self::init(Connection::open(path).map_err(db_err)?, migrations)
    }

    pub fn open_in_memory(migrations: &[&str]) -> Result<Self, CoreError> {
        Self::init(Connection::open_in_memory().map_err(db_err)?, migrations)
    }

    fn init(conn: Connection, migrations: &[&str]) -> Result<Self, CoreError> {
        conn.pragma_update(None, "foreign_keys", "ON").map_err(db_err)?;
        let mut db = Self { conn };
        db.migrate(migrations)?;
        Ok(db)
    }

    pub fn schema_version(&self) -> Result<usize, CoreError> {
        self.conn
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .map(|v| v as usize)
            .map_err(db_err)
    }

    fn migrate(&mut self, migrations: &[&str]) -> Result<(), CoreError> {
        let current = self.schema_version()?;
        if current > migrations.len() {
            return Err(CoreError::Db(format!(
                "データベースがこのアプリより新しい版です(版 {current} / このアプリは {})。アプリを更新してください",
                migrations.len()
            )));
        }
        for (i, sql) in migrations.iter().enumerate().skip(current) {
            let tx = self.conn.transaction().map_err(db_err)?;
            tx.execute_batch(sql).map_err(|e| CoreError::Db(format!("マイグレーション {} に失敗: {e}", i + 1)))?;
            tx.pragma_update(None, "user_version", (i + 1) as i64).map_err(db_err)?;
            tx.commit().map_err(db_err)?;
        }
        Ok(())
    }

    /// 全文検索テーブル(trigram)の rowid を返す。`query` は空白区切りで、全部の語を含むものに絞る(AND)。
    /// 3文字以上の語は FTS5、3文字未満の語は `columns` に対する LIKE。語が無ければ空。
    /// `fts_table` と `columns` は識別子(英数字と _ だけ)であること。
    pub fn search_rowids(&self, fts_table: &str, columns: &[&str], query: &str, limit: usize) -> Result<Vec<i64>, CoreError> {
        let ident = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !ident(fts_table) || columns.is_empty() || !columns.iter().all(|c| ident(c)) {
            return Err(CoreError::Db("テーブル名・列名が不正です".into()));
        }
        let folded = fold_for_search(query);
        let terms: Vec<&str> = folded.split_whitespace().collect();
        if terms.is_empty() {
            return Ok(vec![]);
        }
        let mut conds: Vec<String> = Vec::new();
        let mut args: Vec<String> = Vec::new();
        let long: Vec<String> = terms
            .iter()
            .filter(|t| t.chars().count() >= 3)
            .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
            .collect();
        if !long.is_empty() {
            conds.push(format!("{fts_table} MATCH ?"));
            args.push(long.join(" "));
        }
        for t in terms.iter().filter(|t| t.chars().count() < 3) {
            let like = format!("%{}%", t.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
            let any = columns.iter().map(|c| format!("{c} LIKE ? ESCAPE '\\'")).collect::<Vec<_>>().join(" OR ");
            conds.push(format!("({any})"));
            args.extend(std::iter::repeat(like).take(columns.len()));
        }
        let sql = format!("SELECT rowid FROM {fts_table} WHERE {} ORDER BY rowid LIMIT {}", conds.join(" AND "), limit.max(1));
        let mut stmt = self.conn.prepare(&sql).map_err(db_err)?;
        let rows = stmt
            .query_map(params_from_iter(args.iter()), |r| r.get::<_, i64>(0))
            .map_err(db_err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const M1: &str = "CREATE TABLE docs(id INTEGER PRIMARY KEY, vendor TEXT, summary TEXT);
        CREATE VIRTUAL TABLE docs_fts USING fts5(vendor, summary, tokenize='trigram');";
    const M2: &str = "ALTER TABLE docs ADD COLUMN note TEXT;";

    fn add(db: &Db, id: i64, vendor: &str, summary: &str) {
        db.conn.execute("INSERT INTO docs(id, vendor, summary) VALUES (?1, ?2, ?3)", (id, vendor, summary)).unwrap();
        db.conn
            .execute("INSERT INTO docs_fts(rowid, vendor, summary) VALUES (?1, ?2, ?3)", (id, fold_for_search(vendor), fold_for_search(summary)))
            .unwrap();
    }

    fn seeded() -> Db {
        let db = Db::open_in_memory(&[M1]).unwrap();
        add(&db, 1, "株式会社ひまわり文具", "コピー用紙 A4");
        add(&db, 2, "有限会社さくら書店", "書籍");
        add(&db, 3, "合同会社あおぞら食品", "弁当");
        db
    }

    #[test]
    fn 三文字以上は全文検索_二文字以下はlikeで当たる() {
        let db = seeded();
        let s = |q: &str| db.search_rowids("docs_fts", &["vendor", "summary"], q, 50).unwrap();
        assert_eq!(s("ひまわり"), vec![1]);
        assert_eq!(s("さくら書店"), vec![2]);
        assert_eq!(s("書店"), vec![2]); // 2文字: trigram では 0 件になる語
        assert_eq!(s("文具"), vec![1]);
        assert_eq!(s("会社"), vec![1, 2, 3]);
        assert_eq!(s("会社 書店"), vec![2]); // AND(LIKE どうし)
        assert_eq!(s("あおぞら 弁当"), vec![3]); // FTS と LIKE の混在
        assert_eq!(s("存在しない語"), Vec::<i64>::new());
        assert_eq!(s("   "), Vec::<i64>::new());
    }

    #[test]
    fn 全角半角と大文字小文字をそろえて検索できる() {
        let db = seeded();
        assert_eq!(db.search_rowids("docs_fts", &["vendor", "summary"], "ｺﾋﾟｰ", 10).unwrap(), vec![1]);
        assert_eq!(db.search_rowids("docs_fts", &["vendor", "summary"], "ａ４", 10).unwrap(), vec![1]); // 2文字→LIKE(全角も半角に)
        assert_eq!(db.search_rowids("docs_fts", &["vendor", "summary"], "コピー用紙 a4", 10).unwrap(), vec![1]);
    }

    #[test]
    fn 記号を含む語でも壊れず_識別子の不正は拒否する() {
        let db = seeded();
        assert!(db.search_rowids("docs_fts", &["vendor"], "\"; DROP TABLE docs; --", 10).is_ok());
        assert!(db.search_rowids("docs_fts", &["vendor"], "100%", 10).is_ok());
        assert!(db.search_rowids("docs; DROP", &["vendor"], "abc", 10).is_err());
        assert!(db.search_rowids("docs_fts", &["vendor) --"], "abc", 10).is_err());
        let n: i64 = db.conn.query_row("SELECT count(*) FROM docs", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 3);
    }

    #[test]
    fn マイグレーションは順に適用され_再実行しても進まない() {
        let dir = std::env::temp_dir().join(format!("factory-core-db-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.sqlite");
        {
            let db = Db::open(&path, &[M1]).unwrap();
            assert_eq!(db.schema_version().unwrap(), 1);
        }
        {
            let db = Db::open(&path, &[M1, M2]).unwrap();
            assert_eq!(db.schema_version().unwrap(), 2);
            db.conn.execute("UPDATE docs SET note = 'x'", []).unwrap();
        }
        assert_eq!(Db::open(&path, &[M1, M2]).unwrap().schema_version().unwrap(), 2);
        // 新しい版のデータを古いアプリで開かない
        assert!(matches!(Db::open(&path, &[M1]), Err(CoreError::Db(_))));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 失敗したマイグレーションは巻き戻す() {
        let db = Db::open_in_memory(&[M1]).unwrap();
        let _ = db;
        let r = Db::open_in_memory(&[M1, "CREATE TABLE ok(a); THIS IS NOT SQL;"]);
        assert!(r.is_err());
    }
}
