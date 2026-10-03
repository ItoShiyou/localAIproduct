-- 0001_init.sql: 名刺管理の初期スキーマ(案)。
--
-- 前提: core の `Db::open` が `PRAGMA foreign_keys = ON` と `user_version` の管理を行う(このファイルに書かない)。
--   SQLite 3.34 以上(FTS5 の trigram トークナイザ)
-- 日時は UTC の ISO 8601 文字列(TEXT)。会った日 met_on は 'YYYY-MM-DD'(不明なら NULL)。
-- 個人情報を含む。ログにこの表の値を書かない。
--
-- 方針:
--   * people.status = 'draft' の行は、確認画面を通す前の下書き。検索(people_fts)にも出さない。
--   * 全文検索は、確定済み(confirmed)の人だけを people_fts に入れる。更新はトリガーで行う。
--   * 画像はファイルとして別に保存し、ここにはパスとSHA-256だけを持つ。

CREATE TABLE people (
  id                    INTEGER PRIMARY KEY,
  name                  TEXT NOT NULL DEFAULT '',
  name_kana             TEXT NOT NULL DEFAULT '',
  company               TEXT NOT NULL DEFAULT '',
  department            TEXT NOT NULL DEFAULT '',
  title                 TEXT NOT NULL DEFAULT '',
  email                 TEXT NOT NULL DEFAULT '',
  phone                 TEXT NOT NULL DEFAULT '',
  mobile                TEXT NOT NULL DEFAULT '',
  postal_code           TEXT NOT NULL DEFAULT '',
  address               TEXT NOT NULL DEFAULT '',
  url                   TEXT NOT NULL DEFAULT '',
  status                TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'confirmed')),
  field_confidence_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(field_confidence_json)),
  created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_people_email   ON people (email COLLATE NOCASE) WHERE email <> '';       -- 重複検知(メール一致)
CREATE INDEX idx_people_namecom ON people (name, company) WHERE name <> '' AND company <> ''; -- 重複検知(氏名+会社)
CREATE INDEX idx_people_company ON people (company);                                       -- 会社別の一覧
CREATE INDEX idx_people_created  ON people (created_at);                                   -- 最近追加した順

CREATE TABLE card_images (
  id          INTEGER PRIMARY KEY,
  person_id   INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
  side        TEXT NOT NULL DEFAULT 'front' CHECK (side IN ('front', 'back')),
  sha256      TEXT NOT NULL UNIQUE CHECK (length(sha256) = 64),  -- 同一画像は取り込まない
  stored_path TEXT NOT NULL,                                       -- アプリのデータフォルダからの相対パス
  imported_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_card_images_person ON card_images (person_id);

-- 会った日・場所・きっかけ・メモ(日付つきの履歴。再会のたびに行を足す)
CREATE TABLE encounters (
  id         INTEGER PRIMARY KEY,
  person_id  INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
  met_on     TEXT CHECK (met_on IS NULL OR met_on GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
  place      TEXT NOT NULL DEFAULT '',
  how_met    TEXT NOT NULL DEFAULT '',
  memo       TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_encounters_person ON encounters (person_id, met_on);
CREATE INDEX idx_encounters_met_on ON encounters (met_on);   -- 会った日の順

CREATE TABLE tags (
  id   INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE CHECK (name <> '')
);
CREATE TABLE person_tags (
  person_id INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
  tag_id    INTEGER NOT NULL REFERENCES tags (id) ON DELETE CASCADE,
  PRIMARY KEY (person_id, tag_id)
);
CREATE INDEX idx_person_tags_tag ON person_tags (tag_id);

-- 長い処理(取り込み・読み取り)。core の jobs が確定したら、その形に合わせて作り直す(要確認)
CREATE TABLE jobs (
  id           INTEGER PRIMARY KEY,
  kind         TEXT NOT NULL,
  state        TEXT NOT NULL DEFAULT 'queued' CHECK (state IN ('queued', 'running', 'paused', 'done', 'failed', 'cancelled')),
  progress     REAL NOT NULL DEFAULT 0 CHECK (progress >= 0 AND progress <= 1),
  error        TEXT,                                   -- 個人情報(氏名・メール等)を書かない
  payload_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(payload_json))
);

-- 全文検索。trigram は3文字以上の部分一致のみ。3文字未満の語は core の `Db::search_rowids` が LIKE に切り替える。
-- 索引の更新は、SQLite のトリガーではなくアプリ側(`Store::reindex`)が行う:
-- 保存する文字列と検索語を NFKC でそろえる(core の `fold_for_search`)必要があり、SQL では書けないため。
CREATE VIRTUAL TABLE people_fts USING fts5(
  name, name_kana, company, department, title, memo_all, place_all, tags_all,
  tokenize = 'trigram'
);

CREATE TRIGGER people_touch AFTER UPDATE ON people
WHEN NEW.updated_at = OLD.updated_at
BEGIN
  UPDATE people SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;
