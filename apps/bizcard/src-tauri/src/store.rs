//! 連絡先の保存・検索(「この人、誰だっけ」の中心)。
//!
//! 流れ(利用者の操作3つ): ① 取り込み(`import_card`)→ ② 確認して確定(`confirm`)→
//! ③ 検索して画像とメモを見る(`search` → `person_view`)。確定するまでは下書きで、検索にも一覧にも出ない。
//! 重複の候補は `import_card` が返すだけで、統合はしない。
//! 検索は core の `Db::search_rowids`(3文字以上は FTS5 trigram、3文字未満は LIKE)。
//! 個人情報をログに出さない(このモジュールはログを書かない)。

use crate::dedup::{find_duplicate_pairs, find_duplicates, DuplicateCandidate, PersonKey, Reason};
use crate::extract::Extraction;
use factory_core::db::{fold_for_search, Db};
use factory_core::error::CoreError;
use std::path::Path;

pub const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_init.sql")];

const FTS_COLUMNS: [&str; 8] = ["name", "name_kana", "company", "department", "title", "memo_all", "place_all", "tags_all"];

fn db_err<E: std::fmt::Display>(e: E) -> CoreError {
    CoreError::Db(e.to_string())
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PersonFields {
    pub name: String,
    pub name_kana: String,
    pub company: String,
    pub department: String,
    pub title: String,
    pub email: String,
    pub phone: String,
    pub mobile: String,
    pub postal_code: String,
    pub address: String,
    pub url: String,
}

impl PersonFields {
    /// 抽出結果から下書きの項目を作る。複数あるメール・電話は先頭を採り、残りは別に返す(利用者が確認画面で選ぶ)
    pub fn from_extraction(x: &Extraction) -> (Self, Vec<String>) {
        let v = |f: &Option<crate::extract::Field>| f.as_ref().map(|f| f.value.clone()).unwrap_or_default();
        let first = |l: &[crate::extract::Field]| l.first().map(|f| f.value.clone()).unwrap_or_default();
        let extra: Vec<String> = x.emails.iter().skip(1).map(|f| f.value.clone()).collect();
        (
            PersonFields {
                name: v(&x.name),
                name_kana: v(&x.name_kana),
                company: v(&x.company),
                department: v(&x.department),
                title: v(&x.title),
                email: first(&x.emails),
                phone: first(&x.phones),
                mobile: first(&x.mobiles),
                postal_code: v(&x.postal_code),
                address: v(&x.address),
                url: first(&x.urls),
            },
            extra,
        )
    }
}

fn confidence_json(x: &Extraction) -> String {
    let mut m = serde_json::Map::new();
    let mut put = |k: &str, f: &Option<crate::extract::Field>| {
        if let Some(f) = f {
            m.insert(k.into(), serde_json::json!((f.confidence * 100.0).round() / 100.0));
        }
    };
    put("name", &x.name);
    put("name_kana", &x.name_kana);
    put("company", &x.company);
    put("department", &x.department);
    put("title", &x.title);
    put("email", &x.emails.first().cloned());
    put("phone", &x.phones.first().cloned());
    put("mobile", &x.mobiles.first().cloned());
    put("postal_code", &x.postal_code);
    put("address", &x.address);
    put("url", &x.urls.first().cloned());
    serde_json::Value::Object(m).to_string()
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewEncounter {
    /// 'YYYY-MM-DD'。不明なら None
    pub met_on: Option<String>,
    pub place: String,
    pub how_met: String,
    pub memo: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Encounter {
    pub id: i64,
    pub met_on: Option<String>,
    pub place: String,
    pub how_met: String,
    pub memo: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardImage {
    pub id: i64,
    pub side: String,
    pub sha256: String,
    pub stored_path: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersonView {
    pub id: i64,
    pub status: String,
    pub fields: PersonFields,
    pub field_confidence_json: String,
    pub images: Vec<CardImage>,
    /// 新しい順(会った日、日付が無いものは後ろ)
    pub encounters: Vec<Encounter>,
    pub tags: Vec<String>,
}

/// 検索結果の1件(一覧に出す分)。画像とメモを大きく見せる画面は、`person_view` で詳細を取る
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub id: i64,
    pub name: String,
    pub company: String,
    pub title: String,
    pub thumbnail_path: Option<String>,
    pub latest_memo: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImportOutcome {
    /// 同じ画像(SHA-256)は取り込まない
    AlreadyImported { person_id: i64 },
    Imported { person_id: i64, duplicates: Vec<DuplicateCandidate>, extra_emails: Vec<String> },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Order {
    RecentlyAdded,
    MetOn,
    Company,
}

pub struct Store {
    db: Db,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, CoreError> {
        Ok(Self { db: Db::open(path, MIGRATIONS)? })
    }
    pub fn open_in_memory() -> Result<Self, CoreError> {
        Ok(Self { db: Db::open_in_memory(MIGRATIONS)? })
    }
    pub fn db(&self) -> &Db {
        &self.db
    }

    /// ① 取り込み。抽出結果から下書きを作り、画像を紐づける。重複の候補(確定済みの人のみ)を返す。
    pub fn import_card(&self, sha256: &str, stored_path: &str, side: &str, x: &Extraction) -> Result<ImportOutcome, CoreError> {
        if let Some(pid) = self.person_of_image(sha256)? {
            return Ok(ImportOutcome::AlreadyImported { person_id: pid });
        }
        let (f, extra_emails) = PersonFields::from_extraction(x);
        let c = &self.db.conn;
        c.execute(
            "INSERT INTO people(name,name_kana,company,department,title,email,phone,mobile,postal_code,address,url,status,field_confidence_json)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'draft',?12)",
            (&f.name, &f.name_kana, &f.company, &f.department, &f.title, &f.email, &f.phone, &f.mobile, &f.postal_code, &f.address, &f.url, confidence_json(x)),
        )
        .map_err(db_err)?;
        let pid = c.last_insert_rowid();
        self.attach_image(pid, side, sha256, stored_path)?;
        let duplicates = self.duplicates_for_fields(pid, &f, &extra_emails)?;
        Ok(ImportOutcome::Imported { person_id: pid, duplicates, extra_emails })
    }

    fn person_of_image(&self, sha256: &str) -> Result<Option<i64>, CoreError> {
        match self.db.conn.query_row("SELECT person_id FROM card_images WHERE sha256=?1", [sha256], |r| r.get(0)) {
            Ok(v) => Ok(Some(v)),
            Err(e) if e.to_string().contains("no rows") => Ok(None),
            Err(e) => Err(db_err(e)),
        }
    }

    /// 裏面など、同じ人にもう1枚画像を足す。同じ画像なら false
    pub fn attach_image(&self, person_id: i64, side: &str, sha256: &str, stored_path: &str) -> Result<bool, CoreError> {
        if self.person_of_image(sha256)?.is_some() {
            return Ok(false);
        }
        self.db
            .conn
            .execute("INSERT INTO card_images(person_id, side, sha256, stored_path) VALUES (?1,?2,?3,?4)", (person_id, side, sha256, stored_path))
            .map_err(db_err)?;
        Ok(true)
    }

    fn keys(&self, only_confirmed: bool) -> Result<Vec<PersonKey>, CoreError> {
        let sql = format!(
            "SELECT id, name, company, email FROM people {}",
            if only_confirmed { "WHERE status='confirmed'" } else { "" }
        );
        let mut st = self.db.conn.prepare(&sql).map_err(db_err)?;
        let rows = st
            .query_map([], |r| {
                let email: String = r.get(3)?;
                Ok(PersonKey { id: r.get(0)?, name: r.get(1)?, company: r.get(2)?, emails: if email.is_empty() { vec![] } else { vec![email] } })
            })
            .map_err(db_err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
    }

    fn duplicates_for_fields(&self, id: i64, f: &PersonFields, extra: &[String]) -> Result<Vec<DuplicateCandidate>, CoreError> {
        let mut emails = vec![f.email.clone()];
        emails.extend(extra.iter().cloned());
        let me = PersonKey { id, name: f.name.clone(), company: f.company.clone(), emails };
        Ok(find_duplicates(&me, &self.keys(true)?))
    }

    /// 同じ人かもしれない確定済みの人(統合はしない)
    pub fn duplicates_for(&self, id: i64) -> Result<Vec<DuplicateCandidate>, CoreError> {
        let v = self.person_view(id)?;
        self.duplicates_for_fields(id, &v.fields, &[])
    }

    /// 確定済みの全員の中の、同じ人かもしれない組
    pub fn duplicate_pairs(&self) -> Result<Vec<(i64, i64, Vec<Reason>)>, CoreError> {
        Ok(find_duplicate_pairs(&self.keys(true)?))
    }

    /// ② 確認画面で直した内容で確定する。ここで初めて連絡先として検索に出る。
    pub fn confirm(&self, id: i64, f: &PersonFields, encounter: Option<NewEncounter>, tags: &[String]) -> Result<(), CoreError> {
        self.write_fields(id, f)?;
        self.db.conn.execute("UPDATE people SET status='confirmed' WHERE id=?1", [id]).map_err(db_err)?;
        if let Some(e) = encounter {
            self.insert_encounter(id, &e)?;
        }
        for t in tags {
            self.add_tag(id, t)?;
        }
        self.reindex(id)
    }

    /// 確定後の修正
    pub fn update_person(&self, id: i64, f: &PersonFields) -> Result<(), CoreError> {
        self.write_fields(id, f)?;
        self.reindex(id)
    }

    fn write_fields(&self, id: i64, f: &PersonFields) -> Result<(), CoreError> {
        let n = self
            .db
            .conn
            .execute(
                "UPDATE people SET name=?2,name_kana=?3,company=?4,department=?5,title=?6,email=?7,phone=?8,mobile=?9,postal_code=?10,address=?11,url=?12 WHERE id=?1",
                (id, &f.name, &f.name_kana, &f.company, &f.department, &f.title, &f.email, &f.phone, &f.mobile, &f.postal_code, &f.address, &f.url),
            )
            .map_err(db_err)?;
        if n == 0 {
            return Err(CoreError::Db("その人が見つかりません".into()));
        }
        Ok(())
    }

    fn insert_encounter(&self, id: i64, e: &NewEncounter) -> Result<i64, CoreError> {
        self.db
            .conn
            .execute("INSERT INTO encounters(person_id, met_on, place, how_met, memo) VALUES (?1,?2,?3,?4,?5)", (id, &e.met_on, &e.place, &e.how_met, &e.memo))
            .map_err(db_err)?;
        Ok(self.db.conn.last_insert_rowid())
    }

    /// 再会したときの追記。メモは消さずに足していく(日付つきの履歴)
    pub fn add_encounter(&self, id: i64, e: &NewEncounter) -> Result<i64, CoreError> {
        let eid = self.insert_encounter(id, e)?;
        self.reindex(id)?;
        Ok(eid)
    }

    pub fn add_tag(&self, id: i64, name: &str) -> Result<(), CoreError> {
        let name = name.trim();
        if name.is_empty() {
            return Ok(());
        }
        let c = &self.db.conn;
        c.execute("INSERT OR IGNORE INTO tags(name) VALUES (?1)", [name]).map_err(db_err)?;
        c.execute("INSERT OR IGNORE INTO person_tags(person_id, tag_id) SELECT ?1, id FROM tags WHERE name=?2", (id, name)).map_err(db_err)?;
        self.reindex(id)
    }

    pub fn remove_tag(&self, id: i64, name: &str) -> Result<(), CoreError> {
        self.db
            .conn
            .execute("DELETE FROM person_tags WHERE person_id=?1 AND tag_id=(SELECT id FROM tags WHERE name=?2)", (id, name))
            .map_err(db_err)?;
        self.reindex(id)
    }

    /// 検索用の索引を作り直す。確定済みの人だけが索引に入る。
    fn reindex(&self, id: i64) -> Result<(), CoreError> {
        let c = &self.db.conn;
        c.execute("DELETE FROM people_fts WHERE rowid=?1", [id]).map_err(db_err)?;
        let v = self.person_view(id)?;
        if v.status != "confirmed" {
            return Ok(());
        }
        let f = &v.fields;
        let memo_all: Vec<String> = v.encounters.iter().map(|e| format!("{} {}", e.memo, e.how_met).trim().to_string()).collect();
        let place_all: Vec<&str> = v.encounters.iter().map(|e| e.place.as_str()).filter(|p| !p.is_empty()).collect();
        // 「田中 太郎」を「田中太郎」でも探せるよう、空白を除いた形も索引に足す
        let both = |s: &str| {
            let t: String = s.split_whitespace().collect();
            if t != s { format!("{s} {t}") } else { s.to_string() }
        };
        let cols = [
            both(&f.name), both(&f.name_kana), f.company.clone(), f.department.clone(), f.title.clone(),
            memo_all.join("\n"), place_all.join(" "), v.tags.join(" "),
        ];
        let [a, b, c1, d, e, g, h, i] = cols.map(|s| fold_for_search(&s));
        c.execute(
            "INSERT INTO people_fts(rowid, name, name_kana, company, department, title, memo_all, place_all, tags_all) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            (id, a, b, c1, d, e, g, h, i),
        )
        .map_err(db_err)?;
        Ok(())
    }

    pub fn person_view(&self, id: i64) -> Result<PersonView, CoreError> {
        let c = &self.db.conn;
        let (status, fields, conf) = c
            .query_row(
                "SELECT status,name,name_kana,company,department,title,email,phone,mobile,postal_code,address,url,field_confidence_json FROM people WHERE id=?1",
                [id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        PersonFields {
                            name: r.get(1)?, name_kana: r.get(2)?, company: r.get(3)?, department: r.get(4)?, title: r.get(5)?,
                            email: r.get(6)?, phone: r.get(7)?, mobile: r.get(8)?, postal_code: r.get(9)?, address: r.get(10)?, url: r.get(11)?,
                        },
                        r.get::<_, String>(12)?,
                    ))
                },
            )
            .map_err(|e| if e.to_string().contains("no rows") { CoreError::Db("その人が見つかりません".into()) } else { db_err(e) })?;
        let images = c
            .prepare("SELECT id, side, sha256, stored_path FROM card_images WHERE person_id=?1 ORDER BY side DESC, id")
            .map_err(db_err)?
            .query_map([id], |r| Ok(CardImage { id: r.get(0)?, side: r.get(1)?, sha256: r.get(2)?, stored_path: r.get(3)? }))
            .map_err(db_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_err)?;
        let encounters = c
            .prepare("SELECT id, met_on, place, how_met, memo, created_at FROM encounters WHERE person_id=?1 ORDER BY met_on IS NULL, met_on DESC, id DESC")
            .map_err(db_err)?
            .query_map([id], |r| Ok(Encounter { id: r.get(0)?, met_on: r.get(1)?, place: r.get(2)?, how_met: r.get(3)?, memo: r.get(4)?, created_at: r.get(5)? }))
            .map_err(db_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_err)?;
        let tags = c
            .prepare("SELECT t.name FROM person_tags pt JOIN tags t ON t.id=pt.tag_id WHERE pt.person_id=?1 ORDER BY t.name")
            .map_err(db_err)?
            .query_map([id], |r| r.get::<_, String>(0))
            .map_err(db_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_err)?;
        Ok(PersonView { id, status, fields, field_confidence_json: conf, images, encounters, tags })
    }

    fn summary(&self, id: i64) -> Result<Summary, CoreError> {
        let v = self.person_view(id)?;
        Ok(Summary {
            id,
            name: v.fields.name,
            company: v.fields.company,
            title: v.fields.title,
            thumbnail_path: v.images.iter().find(|i| i.side == "front").or(v.images.first()).map(|i| i.stored_path.clone()),
            latest_memo: v.encounters.iter().map(|e| e.memo.clone()).find(|m| !m.is_empty()).unwrap_or_default(),
        })
    }

    /// ③ 1つの検索窓(氏名・かな・会社・部署・役職・メモ・きっかけ・場所・タグ)。確定済みの人だけ。
    /// 3文字以上の語は全文検索、3文字未満(「田中」など)は LIKE。空白区切りは AND。
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Summary>, CoreError> {
        let ids = self.db.search_rowids("people_fts", &FTS_COLUMNS, query, limit)?;
        ids.into_iter().map(|i| self.summary(i)).collect()
    }

    /// 一覧。最近追加した順 / 会った日の順 / 会社別。タグで絞れる。確定済みのみ。
    pub fn list(&self, order: Order, tag: Option<&str>) -> Result<Vec<Summary>, CoreError> {
        let order_sql = match order {
            Order::RecentlyAdded => "p.created_at DESC, p.id DESC",
            Order::MetOn => "(SELECT max(met_on) FROM encounters e WHERE e.person_id=p.id) IS NULL, (SELECT max(met_on) FROM encounters e WHERE e.person_id=p.id) DESC, p.id DESC",
            Order::Company => "p.company = '', p.company, p.name",
        };
        let sql = format!(
            "SELECT p.id FROM people p WHERE p.status='confirmed' AND (?1 IS NULL OR p.id IN (SELECT pt.person_id FROM person_tags pt JOIN tags t ON t.id=pt.tag_id WHERE t.name=?1)) ORDER BY {order_sql}"
        );
        let ids = self
            .db
            .conn
            .prepare(&sql)
            .map_err(db_err)?
            .query_map([tag], |r| r.get::<_, i64>(0))
            .map_err(db_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_err)?;
        ids.into_iter().map(|i| self.summary(i)).collect()
    }

    /// 1人を画像・メモごと削除する。消すべき画像ファイルのパスを返す(ファイルの削除は呼び出し側)
    pub fn delete_person(&self, id: i64) -> Result<Vec<String>, CoreError> {
        let paths: Vec<String> = self.person_view(id)?.images.into_iter().map(|i| i.stored_path).collect();
        self.db.conn.execute("DELETE FROM people_fts WHERE rowid=?1", [id]).map_err(db_err)?;
        self.db.conn.execute("DELETE FROM people WHERE id=?1", [id]).map_err(db_err)?;
        Ok(paths)
    }

    /// 全データ削除(設定画面の「全データを削除」)。画像のパスを返す
    pub fn delete_all(&self) -> Result<Vec<String>, CoreError> {
        let paths = self
            .db
            .conn
            .prepare("SELECT stored_path FROM card_images")
            .map_err(db_err)?
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_err)?;
        for t in ["people_fts", "people", "tags", "jobs"] {
            self.db.conn.execute(&format!("DELETE FROM {t}"), []).map_err(db_err)?;
        }
        Ok(paths)
    }

    /// CSV / vCard 用に、確定済みの全員を取り出す
    pub fn export_rows(&self) -> Result<Vec<crate::csv_export::ExportPerson>, CoreError> {
        let ids = self.list(Order::RecentlyAdded, None)?;
        ids.into_iter()
            .map(|s| {
                let v = self.person_view(s.id)?;
                let f = v.fields;
                let first = v.encounters.iter().rev().find(|e| e.met_on.is_some());
                Ok(crate::csv_export::ExportPerson {
                    name: f.name, name_kana: f.name_kana, company: f.company, department: f.department, title: f.title,
                    email: f.email, phone: f.phone, mobile: f.mobile, postal_code: f.postal_code, address: f.address, url: f.url,
                    first_met_on: first.and_then(|e| e.met_on.clone()).unwrap_or_default(),
                    place: first.map(|e| e.place.clone()).unwrap_or_default(),
                    tags: v.tags,
                    memos: v.encounters.iter().filter(|e| !e.memo.is_empty()).map(|e| (e.met_on.clone().unwrap_or_default(), e.memo.clone())).collect(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::Field;

    fn fld(v: &str) -> Option<Field> {
        Some(Field { value: v.into(), confidence: 0.9, line: 0 })
    }
    fn ext(name: &str, company: &str, email: &str) -> Extraction {
        Extraction {
            name: fld(name),
            company: fld(company),
            title: fld("課長"),
            emails: if email.is_empty() { vec![] } else { vec![Field { value: email.into(), confidence: 0.9, line: 0 }] },
            ..Default::default()
        }
    }
    fn sha(n: u8) -> String {
        format!("{:02x}", n).repeat(32)
    }
    fn confirmed(s: &Store, n: u8, name: &str, company: &str, email: &str) -> i64 {
        let ImportOutcome::Imported { person_id, .. } = s.import_card(&sha(n), &format!("cards/{n}.jpg"), "front", &ext(name, company, email)).unwrap() else { panic!() };
        let (f, _) = PersonFields::from_extraction(&ext(name, company, email));
        s.confirm(person_id, &f, None, &[]).unwrap();
        person_id
    }

    #[test]
    fn 誰だっけ_取り込み_確定_検索で画像とメモが見える_3操作() {
        let s = Store::open_in_memory().unwrap();
        // 操作1: 取り込み
        let ImportOutcome::Imported { person_id, .. } = s.import_card(&sha(1), "cards/1.jpg", "front", &ext("田中 太郎", "株式会社ひなた工房", "tanaka@example.com")).unwrap() else { panic!() };
        // 確定前は検索に出ない
        assert!(s.search("田中", 10).unwrap().is_empty());
        // 操作2: 確認して確定(会った日・場所・きっかけ・メモを残す)
        let (f, _) = PersonFields::from_extraction(&ext("田中 太郎", "株式会社ひなた工房", "tanaka@example.com"));
        let enc = NewEncounter { met_on: Some("2026-09-30".into()), place: "横浜の展示会".into(), how_met: "隣のブースだった".into(), memo: "猫を飼っている。来月ロゴの相談".into() };
        s.confirm(person_id, &f, Some(enc), &["展示会".into()]).unwrap();
        // 操作3: 検索 → 画像とメモ
        let hits = s.search("猫", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].thumbnail_path.as_deref(), Some("cards/1.jpg"));
        assert!(hits[0].latest_memo.contains("ロゴの相談"));
        let v = s.person_view(hits[0].id).unwrap();
        assert_eq!(v.images[0].stored_path, "cards/1.jpg");
        assert_eq!(v.encounters[0].place, "横浜の展示会");
        assert_eq!(v.tags, vec!["展示会"]);
    }

    #[test]
    fn 二文字の氏名田中で必ず当たる_一文字_全角_かな_カタカナも() {
        let s = Store::open_in_memory().unwrap();
        let a = confirmed(&s, 1, "田中 太郎", "株式会社A", "");
        let _b = confirmed(&s, 2, "佐藤 花子", "株式会社B", "");
        let c = confirmed(&s, 3, "田中", "有限会社C", ""); // 名だけ・姓だけの表記
        let ids = |q: &str| -> Vec<i64> { s.search(q, 10).unwrap().into_iter().map(|h| h.id).collect() };
        assert_eq!(ids("田中"), vec![a, c]);
        assert_eq!(ids("田"), vec![a, c]); // 1文字
        assert_eq!(ids("田中太郎"), vec![a]); // 空白なしで入力しても当たる
        assert_eq!(ids("田中 太郎"), vec![a]); // 空白区切りはAND
        assert_eq!(ids("太郎"), vec![a]);
    }

    #[test]
    fn 全角半角を同じに扱う() {
        let s = Store::open_in_memory().unwrap();
        let a = confirmed(&s, 1, "Tanaka Taro", "ＡＢＣ商事", "");
        let ids = |q: &str| -> Vec<i64> { s.search(q, 10).unwrap().into_iter().map(|h| h.id).collect() };
        assert_eq!(ids("ABC"), vec![a]); // 保存は全角、検索は半角
        assert_eq!(ids("ＴＡＮＡＫＡ"), vec![a]); // 検索は全角、保存は半角
        assert_eq!(ids("tanaka"), vec![a], "大文字小文字は区別しない(trigram の既定)");
    }

    #[test]
    fn メモ履歴は追記され_新しい順で見え_検索にも効く() {
        let s = Store::open_in_memory().unwrap();
        let id = confirmed(&s, 1, "井上 健太", "合同会社みなと設計", "");
        s.add_encounter(id, &NewEncounter { met_on: Some("2026-09-01".into()), place: "神田".into(), how_met: "紹介".into(), memo: "初回の打合せ".into() }).unwrap();
        s.add_encounter(id, &NewEncounter { met_on: Some("2026-10-02".into()), place: "大阪".into(), how_met: "".into(), memo: "再会。独立したと聞いた".into() }).unwrap();
        let v = s.person_view(id).unwrap();
        assert_eq!(v.encounters.len(), 2, "前のメモは消えない");
        assert_eq!(v.encounters[0].met_on.as_deref(), Some("2026-10-02"));
        assert_eq!(s.search("独立", 10).unwrap().len(), 1);
        assert_eq!(s.search("初回の打合せ", 10).unwrap().len(), 1);
        assert_eq!(s.search("大阪", 10).unwrap().len(), 1);
        assert_eq!(s.search("神田", 10).unwrap().len(), 1);
        assert_eq!(s.search("紹介", 10).unwrap().len(), 1);
    }

    #[test]
    fn 同じ画像は取り込まない_裏面は同じ人に足せる() {
        let s = Store::open_in_memory().unwrap();
        let ImportOutcome::Imported { person_id, .. } = s.import_card(&sha(1), "a.jpg", "front", &ext("A", "B", "")).unwrap() else { panic!() };
        assert_eq!(s.import_card(&sha(1), "a2.jpg", "front", &ext("A", "B", "")).unwrap(), ImportOutcome::AlreadyImported { person_id });
        assert!(s.attach_image(person_id, "back", &sha(2), "b.jpg").unwrap());
        assert!(!s.attach_image(person_id, "back", &sha(2), "b.jpg").unwrap());
        assert_eq!(s.person_view(person_id).unwrap().images.len(), 2);
    }

    #[test]
    fn 重複候補は出るが_自動で統合しない() {
        let s = Store::open_in_memory().unwrap();
        let first = confirmed(&s, 1, "山田 太郎", "株式会社テスト", "taro@example.com");
        let out = s.import_card(&sha(2), "2.jpg", "front", &ext("山田太郎", "(株)テスト", "TARO@example.com")).unwrap();
        let ImportOutcome::Imported { person_id, duplicates, .. } = out else { panic!() };
        assert_eq!(duplicates.len(), 1);
        assert_eq!(duplicates[0].existing_id, first);
        assert_eq!(duplicates[0].reasons.len(), 2);
        // 統合されず、別の下書きとして残る
        assert_ne!(person_id, first);
        let n: i64 = s.db().conn.query_row("SELECT count(*) FROM people", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 2);
        assert_eq!(s.duplicates_for(person_id).unwrap().len(), 1);
    }

    #[test]
    fn 下書きは検索にも一覧にも出ない() {
        let s = Store::open_in_memory().unwrap();
        s.import_card(&sha(1), "1.jpg", "front", &ext("山田 太郎", "株式会社テスト", "")).unwrap();
        assert!(s.search("山田", 10).unwrap().is_empty());
        assert!(s.list(Order::RecentlyAdded, None).unwrap().is_empty());
    }

    #[test]
    fn 一覧は並びとタグで絞れる() {
        let s = Store::open_in_memory().unwrap();
        let a = confirmed(&s, 1, "A 一", "ウ会社", "");
        let b = confirmed(&s, 2, "B 二", "ア会社", "");
        s.add_encounter(a, &NewEncounter { met_on: Some("2026-10-01".into()), ..Default::default() }).unwrap();
        s.add_encounter(b, &NewEncounter { met_on: Some("2026-09-01".into()), ..Default::default() }).unwrap();
        s.add_tag(b, "取引先").unwrap();
        let ids = |v: Vec<Summary>| -> Vec<i64> { v.into_iter().map(|h| h.id).collect() };
        assert_eq!(ids(s.list(Order::Company, None).unwrap()), vec![b, a]);
        assert_eq!(ids(s.list(Order::MetOn, None).unwrap()), vec![a, b]);
        assert_eq!(ids(s.list(Order::RecentlyAdded, None).unwrap()), vec![b, a]);
        assert_eq!(ids(s.list(Order::Company, Some("取引先")).unwrap()), vec![b]);
        s.remove_tag(b, "取引先").unwrap();
        assert!(s.list(Order::Company, Some("取引先")).unwrap().is_empty());
        assert!(s.search("取引先", 10).unwrap().is_empty());
    }

    #[test]
    fn 削除は画像とメモと索引ごと_全削除もできる() {
        let s = Store::open_in_memory().unwrap();
        let a = confirmed(&s, 1, "山田 太郎", "株式会社テスト", "");
        s.add_encounter(a, &NewEncounter { memo: "メモ".into(), ..Default::default() }).unwrap();
        let paths = s.delete_person(a).unwrap();
        assert_eq!(paths, vec!["cards/1.jpg"]);
        for t in ["people", "card_images", "encounters", "people_fts"] {
            let n: i64 = s.db().conn.query_row(&format!("SELECT count(*) FROM {t}"), [], |r| r.get(0)).unwrap();
            assert_eq!(n, 0, "{t}");
        }
        confirmed(&s, 2, "佐藤 花子", "株式会社B", "");
        assert_eq!(s.delete_all().unwrap().len(), 1);
        assert!(s.search("佐藤", 10).unwrap().is_empty());
    }

    #[test]
    fn 修正すると検索が追従し_csvに書き出せる() {
        let s = Store::open_in_memory().unwrap();
        let id = confirmed(&s, 1, "山田 太郎", "株式会社テスト", "");
        let mut v = s.person_view(id).unwrap().fields;
        v.name = "=cmd|' /C calc'!A0".into();
        s.update_person(id, &v).unwrap();
        assert!(s.search("山田", 10).unwrap().is_empty());
        let rows = s.export_rows().unwrap();
        let out = crate::csv_export::people_to_csv(&rows, factory_core::export::CsvEncoding::Utf8Bom).unwrap();
        assert!(String::from_utf8_lossy(&out).contains("'=cmd"));
    }
}
