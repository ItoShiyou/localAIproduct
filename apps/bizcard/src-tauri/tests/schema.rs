//! SQLite スキーマ案(migrations/0001_init.sql)の動作確認。

use rusqlite::Connection;

fn open() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", "ON").unwrap();
    c.execute_batch(include_str!("../migrations/0001_init.sql")).unwrap();
    c
}

fn fts(c: &Connection, q: &str) -> Vec<i64> {
    let mut s = c.prepare("SELECT rowid FROM people_fts WHERE people_fts MATCH ?1 ORDER BY rowid").unwrap();
    s.query_map([format!("\"{q}\"")], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect()
}

fn add_person(c: &Connection, name: &str, company: &str, status: &str) -> i64 {
    c.execute("INSERT INTO people(name, company, status) VALUES (?1, ?2, ?3)", [name, company, status]).unwrap();
    c.last_insert_rowid()
}

#[test]
fn 適用するとバージョンが1になる() {
    let c = open();
    let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(v, 1);
    let sqlite: String = c.query_row("SELECT sqlite_version()", [], |r| r.get(0)).unwrap();
    println!("テストに使った SQLite: {sqlite}");
}

#[test]
fn 下書きは検索に出ず_確定すると出る() {
    let c = open();
    let id = add_person(&c, "青木 遥", "株式会社ひなた工房", "draft");
    assert!(fts(&c, "ひなた工房").is_empty(), "確認画面を通す前の下書きは検索に出さない");
    c.execute("UPDATE people SET status='confirmed' WHERE id=?1", [id]).unwrap();
    assert_eq!(fts(&c, "ひなた工房"), vec![id]);
    c.execute("UPDATE people SET status='draft' WHERE id=?1", [id]).unwrap();
    assert!(fts(&c, "ひなた工房").is_empty());
}

#[test]
fn 日本語の3文字以上の部分一致ができる_2文字はmatchに当たらない() {
    let c = open();
    let id = add_person(&c, "青木 遥", "株式会社ひなた工房", "confirmed");
    assert_eq!(fts(&c, "ひなた"), vec![id]); // 部分一致
    assert_eq!(fts(&c, "式会社"), vec![id]);
    // 2文字は trigram の MATCH では見つからない(既知の制約。アプリ側でLIKEにフォールバックする)
    assert!(fts(&c, "工房").is_empty());
    let n: i64 = c.query_row("SELECT count(*) FROM people WHERE company LIKE '%' || ?1 || '%'", ["工房"], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
}

#[test]
fn メモ_場所_タグも同じ検索窓で探せ_更新が追従する() {
    let c = open();
    let id = add_person(&c, "井上 健太", "合同会社みなと設計", "confirmed");
    c.execute("INSERT INTO encounters(person_id, met_on, place, how_met, memo) VALUES (?1,'2026-09-30','横浜みなとみらい','展示会で名刺交換','猫を飼っている話をした')", [id]).unwrap();
    assert_eq!(fts(&c, "猫を飼"), vec![id]);
    assert_eq!(fts(&c, "みなとみらい"), vec![id]);
    assert_eq!(fts(&c, "展示会"), vec![id]);

    c.execute("INSERT INTO tags(name) VALUES ('展示会2026秋')", []).unwrap();
    let tag = c.last_insert_rowid();
    c.execute("INSERT INTO person_tags(person_id, tag_id) VALUES (?1, ?2)", [id, tag]).unwrap();
    assert_eq!(fts(&c, "2026秋"), vec![id]);
    c.execute("UPDATE tags SET name='名刺交換会' WHERE id=?1", [tag]).unwrap();
    assert!(fts(&c, "2026秋").is_empty());
    assert_eq!(fts(&c, "名刺交換会"), vec![id]);

    c.execute("DELETE FROM encounters WHERE person_id=?1", [id]).unwrap();
    assert!(fts(&c, "猫を飼").is_empty());
}

#[test]
fn メモは複数追加でき_日付つきの履歴になる() {
    let c = open();
    let id = add_person(&c, "森 康介", "株式会社A", "confirmed");
    for (d, m) in [("2026-09-01", "初回"), ("2026-10-02", "再会。独立したと聞いた")] {
        c.execute("INSERT INTO encounters(person_id, met_on, memo) VALUES (?1, ?2, ?3)", [&id.to_string(), d, m]).unwrap();
    }
    let n: i64 = c.query_row("SELECT count(*) FROM encounters WHERE person_id=?1 ORDER BY met_on", [id], |r| r.get(0)).unwrap();
    assert_eq!(n, 2);
    assert!(c.execute("INSERT INTO encounters(person_id, met_on) VALUES (?1, '2026/10/02')", [id]).is_err(), "日付の形式を検査する");
}

#[test]
fn 人を削除すると画像_メモ_タグの紐づけ_検索の行が消える() {
    let c = open();
    let id = add_person(&c, "渡辺 優子", "株式会社さくら食品", "confirmed");
    let sha = "a".repeat(64);
    c.execute("INSERT INTO card_images(person_id, side, sha256, stored_path) VALUES (?1,'front',?2,'cards/1.jpg')", [&id.to_string(), &sha]).unwrap();
    c.execute("INSERT INTO encounters(person_id, memo) VALUES (?1,'メモ')", [id]).unwrap();
    c.execute("INSERT INTO tags(name) VALUES ('t')", []).unwrap();
    c.execute("INSERT INTO person_tags VALUES (?1, 1)", [id]).unwrap();
    c.execute("DELETE FROM people WHERE id=?1", [id]).unwrap();
    for t in ["card_images", "encounters", "person_tags", "people_fts"] {
        let n: i64 = c.query_row(&format!("SELECT count(*) FROM {t}"), [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0, "{t} に行が残っている");
    }
    // タグ自体は残る(他の人が使うかもしれない)
    let n: i64 = c.query_row("SELECT count(*) FROM tags", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
}

#[test]
fn 同じ画像のsha256は2回登録できない() {
    let c = open();
    let id = add_person(&c, "a", "b", "draft");
    let sha = "b".repeat(64);
    let ins = |side: &str| c.execute("INSERT INTO card_images(person_id, side, sha256, stored_path) VALUES (?1,?2,?3,'p')", [&id.to_string(), side, &sha]);
    assert!(ins("front").is_ok());
    assert!(ins("back").is_err());
}

#[test]
fn 不正な値は制約で弾く() {
    let c = open();
    assert!(c.execute("INSERT INTO people(status) VALUES ('done')", []).is_err());
    assert!(c.execute("INSERT INTO people(field_confidence_json) VALUES ('{broken')", []).is_err());
    let id = add_person(&c, "a", "b", "draft");
    assert!(c.execute("INSERT INTO card_images(person_id, side, sha256, stored_path) VALUES (?1,'left','x','p')", [id]).is_err());
    assert!(c.execute("INSERT INTO card_images(person_id, side, sha256, stored_path) VALUES (999,'front',?1,'p')", ["c".repeat(64)]).is_err(), "存在しない人への紐づけは外部キーで弾く");
}

#[test]
fn updated_atは更新で進む() {
    let c = open();
    let id = add_person(&c, "a", "b", "draft");
    c.execute("UPDATE people SET updated_at='2000-01-01T00:00:00.000Z' WHERE id=?1", [id]).unwrap();
    // 明示的に変えた値はそのまま保たれる(トリガーは「変えていないとき」だけ動く)
    c.execute("UPDATE people SET title='課長' WHERE id=?1", [id]).unwrap();
    let u: String = c.query_row("SELECT updated_at FROM people WHERE id=?1", [id], |r| r.get(0)).unwrap();
    assert!(u.as_str() > "2000-01-01T00:00:00.000Z");
}
