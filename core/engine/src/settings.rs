//! 設定・通信一覧・全データの削除。
//!
//! - 設定はアプリのデータフォルダの `settings.json` に保存する(アプリごとに別のフォルダ)。
//! - 通信一覧は、アプリが SPEC.md の「通信一覧」と同じ内容を `NetworkEntry` で渡し、設定画面に表示する。
//!   `stoppable` が true のものは、設定で止められる(更新の確認など)。
//! - 全データの削除は、フォルダに目印ファイル(`.factory-app-data`)があるときだけ行う。
//!   別のフォルダを誤って消さないため。

use crate::error::CoreError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const MARKER: &str = ".factory-app-data";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// 更新の確認(アプリのバージョンだけを送る)。既定はオン
    #[serde(default = "yes")]
    pub update_check: bool,
    /// 確認なしで書き出す先など、アプリ固有の設定は `extra` に入れる
    #[serde(default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

fn yes() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self { update_check: true, extra: Default::default() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NetworkEntry {
    pub purpose: String,
    pub destination: String,
    pub content: String,
    pub stoppable: bool,
    /// 設定で止められる通信のとき、いま許可されているか。止められないものは常に true
    pub enabled: bool,
}

/// アプリのデータフォルダ。
pub struct AppData {
    pub root: PathBuf,
}

impl AppData {
    /// フォルダを作り、目印ファイルを置く。
    pub fn init(root: impl Into<PathBuf>) -> Result<Self, CoreError> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(|e| CoreError::Db(e.to_string()))?;
        let m = root.join(MARKER);
        if !m.exists() {
            fs::write(&m, "factory app data\n").map_err(|e| CoreError::Db(e.to_string()))?;
        }
        Ok(Self { root })
    }

    pub fn load_settings(&self) -> Settings {
        // 読めない・壊れているときは既定値(設定の破損でアプリを止めない)
        fs::read_to_string(self.root.join("settings.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// 一時ファイルに書いてから置き換える。
    pub fn save_settings(&self, s: &Settings) -> Result<(), CoreError> {
        let tmp = self.root.join("settings.json.tmp");
        let text = serde_json::to_string_pretty(s).map_err(|e| CoreError::Parse(e.to_string()))?;
        fs::write(&tmp, text).map_err(|e| CoreError::Db(e.to_string()))?;
        fs::rename(&tmp, self.root.join("settings.json")).map_err(|e| CoreError::Db(e.to_string()))
    }

    /// 通信一覧に、いまの設定(更新の確認のオン/オフ)を反映する。`update` は更新確認の項目を指す目印の目的名。
    pub fn network_list(&self, entries: &[NetworkEntry], update_purpose: &str) -> Vec<NetworkEntry> {
        let s = self.load_settings();
        entries
            .iter()
            .cloned()
            .map(|mut e| {
                if e.purpose == update_purpose {
                    e.enabled = s.update_check;
                }
                e
            })
            .collect()
    }

    /// 更新の確認を許可しているか(通信する前に必ずここを通す)
    pub fn update_check_allowed(&self) -> bool {
        self.load_settings().update_check
    }

    /// データフォルダの中身をすべて消す(目印ファイルを残してフォルダ自体は残す)。
    /// 目印が無いフォルダは消さない。消した項目数を返す。
    pub fn delete_all(&self) -> Result<usize, CoreError> {
        delete_all_in(&self.root)
    }
}

pub fn delete_all_in(root: &Path) -> Result<usize, CoreError> {
    if !root.join(MARKER).is_file() {
        return Err(CoreError::Db("アプリのデータフォルダではないため、削除しませんでした".into()));
    }
    let mut n = 0;
    for e in fs::read_dir(root).map_err(|e| CoreError::Db(e.to_string()))? {
        let p = e.map_err(|e| CoreError::Db(e.to_string()))?.path();
        if p.file_name().map(|f| f == MARKER).unwrap_or(false) {
            continue;
        }
        if p.is_dir() {
            fs::remove_dir_all(&p)
        } else {
            fs::remove_file(&p)
        }
        .map_err(|e| CoreError::Db(e.to_string()))?;
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("factory-set-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn entries() -> Vec<NetworkEntry> {
        vec![
            NetworkEntry { purpose: "ライセンス認証".into(), destination: "販売プラットフォーム".into(), content: "キー".into(), stoppable: false, enabled: true },
            NetworkEntry { purpose: "更新の確認".into(), destination: "配布元".into(), content: "バージョン".into(), stoppable: true, enabled: true },
        ]
    }

    #[test]
    fn 既定は更新確認オン_保存して読める_壊れていても既定() {
        let d = tmp("a");
        let a = AppData::init(&d).unwrap();
        assert!(a.load_settings().update_check);
        let mut s = a.load_settings();
        s.update_check = false;
        s.extra.insert("csv_encoding".into(), "utf8bom".into());
        a.save_settings(&s).unwrap();
        assert_eq!(a.load_settings(), s);
        assert!(!a.update_check_allowed());
        fs::write(d.join("settings.json"), "壊れた").unwrap();
        assert!(a.load_settings().update_check);
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 通信一覧に更新確認の状態が反映される() {
        let d = tmp("b");
        let a = AppData::init(&d).unwrap();
        a.save_settings(&Settings { update_check: false, ..Default::default() }).unwrap();
        let l = a.network_list(&entries(), "更新の確認");
        assert!(l[0].enabled && !l[1].enabled);
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 全削除は目印があるフォルダだけで_目印は残す() {
        let d = tmp("c");
        let a = AppData::init(&d).unwrap();
        fs::create_dir_all(d.join("originals")).unwrap();
        fs::write(d.join("originals/x.pdf"), "x").unwrap();
        fs::write(d.join("db.sqlite"), "x").unwrap();
        assert_eq!(a.delete_all().unwrap(), 2);
        assert!(d.join(MARKER).exists() && !d.join("db.sqlite").exists());
        let other = tmp("d");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("大事なファイル"), "x").unwrap();
        assert!(delete_all_in(&other).is_err());
        assert!(other.join("大事なファイル").exists());
        fs::remove_dir_all(&d).ok();
        fs::remove_dir_all(&other).ok();
    }
}
