//! 無料版と有料版(買い切り)の違いと、無料版の「使った量」の記録。
//!
//! 方針(2026-10-04、競合調査にもとづき確定。`docs/plans.md`):
//! - 無料版はお試し。**累計で文字起こしした音声の長さ(60 分)**で制限する。議事録を消しても戻らない
//!   (保存数で制限すると「書き出して消す」で無限に使えるため)。1件は最大 15 分。精度は小さなモデル。
//!   書き出しはテキストのみ(末尾に「無料版で作成」)。話者の判別・ノイズ除去・用語辞書・Word/PDF などは有料版。
//! - 有料版はすべての機能と正確なモデル、上限なし。
//! - 使った量は、署名(HMAC)を付けて複数の場所に保存し、いちばん大きい値を使う。署名が合わない記録があれば
//!   「使い切った」とみなす。全データの削除でも消さない。
//!   **限界**: 端末の中だけの仕組みなので、記録の場所をすべて消す・アプリを改造する、といった手間をかければ破れる。
//!   目的は「割に合わない」ようにすること。

use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pro,
}

#[cfg(all(feature = "edition-free", feature = "edition-pro"))]
compile_error!("Select only one minutes edition");

/// Distribution edition is fixed at compile time, never by a runtime environment variable.
pub fn distribution_tier() -> Option<Tier> {
    if cfg!(feature = "edition-free") { Some(Tier::Free) }
    else if cfg!(feature = "edition-pro") { Some(Tier::Pro) }
    else { None }
}

pub const FREE_TOTAL_MS: u64 = 60 * 60 * 1000;
pub const FREE_MEETING_MS: u64 = 15 * 60 * 1000;
/// 無料版の書き出しの末尾に入れる一文
pub const FREE_FOOTER: &str = "— この議事録はminutes無料版で作成しました。";

/// 機能ごとの可否(画面にもそのまま渡す)
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entitlements {
    pub tier: Tier,
    /// 正確なモデル(large-v3-turbo)を使えるか。使えなければ小さなモデル
    pub accurate_model: bool,
    pub diarize: bool,
    pub denoise: bool,
    pub glossary: bool,
    /// 書き出せる形式(md / txt / srt / docx / pdf / wav)
    pub exports: Vec<&'static str>,
    pub summary: bool,
    /// 累計の上限(ミリ秒)。None なら無制限
    pub total_limit_ms: Option<u64>,
    /// 1件の上限(ミリ秒)。None なら無制限
    pub meeting_limit_ms: Option<u64>,
}

impl Entitlements {
    pub fn of(tier: Tier) -> Self {
        match tier {
            Tier::Pro => Self {
                tier, accurate_model: true, diarize: true, denoise: true, glossary: true,
                exports: vec!["docx", "pdf", "md", "txt", "srt", "wav"], summary: true, total_limit_ms: None, meeting_limit_ms: None,
            },
            Tier::Free => Self {
                tier, accurate_model: false, diarize: false, denoise: false, glossary: false,
                exports: vec!["txt"], summary: false, total_limit_ms: Some(FREE_TOTAL_MS), meeting_limit_ms: Some(FREE_MEETING_MS),
            },
        }
    }

    pub fn can_export(&self, format: &str) -> bool {
        self.exports.contains(&format)
    }
}

/// 版の決定。有効なライセンスがあれば有料版。無ければ、デバッグビルドだけ環境変数 `MINUTES_TIER`(pro / free)を見る
/// (未指定なら有料版)。**配布用のビルドは環境変数を無視して無料版**(環境変数で有料版にできない)。
pub fn resolve_tier(license_valid: bool, env: Option<&str>, debug_build: bool) -> Tier {
    if let Some(tier) = distribution_tier() { return tier; }
    if license_valid {
        return Tier::Pro;
    }
    if !debug_build {
        return Tier::Free;
    }
    match env {
        Some("free") => Tier::Free,
        _ => Tier::Pro,
    }
}

pub const PRO_ONLY: &str = "有料版の機能です。有料版にすると使えます";

// ---------------- 使った量の記録 ----------------

/// 記録を置く場所の抽象(ファイル・OS のキーチェーンなど)。テストでは差し替える
pub trait Slot: Send + Sync {
    fn read(&self) -> Option<String>;
    fn write(&self, v: &str);
}

pub struct FileSlot(pub PathBuf);

impl Slot for FileSlot {
    fn read(&self) -> Option<String> {
        std::fs::read_to_string(&self.0).ok()
    }
    fn write(&self, v: &str) {
        if let Some(d) = self.0.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let tmp = self.0.with_extension("tmp");
        if std::fs::write(&tmp, v).is_ok() {
            let _ = std::fs::rename(&tmp, &self.0);
        }
    }
}

/// OS の資格情報ストア(macOS のキーチェーン、Windows の資格情報マネージャー)
#[cfg(feature = "tauri")]
pub struct KeychainSlot {
    pub service: String,
}

#[cfg(feature = "tauri")]
impl Slot for KeychainSlot {
    fn read(&self) -> Option<String> {
        keyring::Entry::new(&self.service, "usage").ok()?.get_password().ok()
    }
    fn write(&self, v: &str) {
        if let Ok(e) = keyring::Entry::new(&self.service, "usage") {
            let _ = e.set_password(v);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// 累計で文字起こしした音声の長さ
    pub used_ms: u64,
    /// 累計の文字起こしの件数(参考)
    pub count: u64,
    /// 記録が改ざんされている(使い切った扱い)
    pub tampered: bool,
}

/// 署名の鍵(バイナリに埋め込む。見つけにくくするだけで、秘密を守るものではない)
fn key() -> Vec<u8> {
    let a = *b"m1nutes-l0cal-usage-ledger-v1::";
    a.iter().enumerate().map(|(i, b)| b ^ (0x5a ^ i as u8)).collect()
}

fn sign(used: u64, count: u64) -> String {
    let mut m = Hmac::<Sha256>::new_from_slice(&key()).expect("HMAC");
    m.update(format!("v1|{used}|{count}").as_bytes());
    m.finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

fn encode(u: Usage) -> String {
    format!("v1|{}|{}|{}", u.used_ms, u.count, sign(u.used_ms, u.count))
}

/// 1つの記録を読む: Ok(None)=記録なし、Ok(Some)=正しい記録、Err=改ざん
fn decode(s: &str) -> Result<Option<Usage>, ()> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let p: Vec<&str> = s.split('|').collect();
    if p.len() != 4 || p[0] != "v1" {
        return Err(());
    }
    let (Ok(used), Ok(count)) = (p[1].parse::<u64>(), p[2].parse::<u64>()) else { return Err(()) };
    if sign(used, count) != p[3] {
        return Err(());
    }
    Ok(Some(Usage { used_ms: used, count, tampered: false }))
}

pub struct Ledger {
    slots: Vec<Box<dyn Slot>>,
    cache: Mutex<Usage>,
}

impl Ledger {
    pub fn new(slots: Vec<Box<dyn Slot>>) -> Self {
        let l = Self { slots, cache: Mutex::new(Usage::default()) };
        let u = l.load();
        *l.cache.lock().unwrap() = u;
        // 欠けている場所があれば書き戻す(どこか1つ消されても戻る)
        if !u.tampered {
            l.save(u);
        }
        l
    }

    /// すべての場所を読み、いちばん大きい値を使う。署名の合わない記録があれば「使い切った」扱い。
    fn load(&self) -> Usage {
        let mut best = Usage::default();
        for s in &self.slots {
            match s.read().map(|v| decode(&v)) {
                Some(Ok(Some(u))) => {
                    if u.used_ms > best.used_ms {
                        best.used_ms = u.used_ms;
                    }
                    best.count = best.count.max(u.count);
                }
                Some(Err(())) => best.tampered = true,
                _ => {}
            }
        }
        best
    }

    fn save(&self, u: Usage) {
        let v = encode(u);
        for s in &self.slots {
            s.write(&v);
        }
    }

    pub fn usage(&self) -> Usage {
        // 別の場所の値が大きくなっていれば合わせる(複数のアプリの起動などに備えて読み直す)
        let mut c = self.cache.lock().unwrap();
        let disk = self.load();
        c.used_ms = c.used_ms.max(disk.used_ms);
        c.count = c.count.max(disk.count);
        c.tampered |= disk.tampered;
        *c
    }

    /// 文字起こしした長さを足す
    pub fn add(&self, ms: u64, new_meeting: bool) {
        let mut u = self.usage();
        u.used_ms += ms;
        if new_meeting {
            u.count += 1;
        }
        *self.cache.lock().unwrap() = u;
        if !u.tampered {
            self.save(u);
        }
    }

    /// 無料版の残り(ミリ秒)
    pub fn remaining(&self, e: &Entitlements) -> Option<u64> {
        let u = self.usage();
        e.total_limit_ms.map(|lim| if u.tampered { 0 } else { lim.saturating_sub(u.used_ms) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[derive(Clone, Default)]
    struct Mem(Arc<Mutex<Option<String>>>);
    impl Slot for Mem {
        fn read(&self) -> Option<String> {
            self.0.lock().unwrap().clone()
        }
        fn write(&self, v: &str) {
            *self.0.lock().unwrap() = Some(v.to_string());
        }
    }

    #[test]
    fn 使った量は消しても戻らず_どこか1つ消されても戻る() {
        let (a, b, c) = (Mem::default(), Mem::default(), Mem::default());
        let l = Ledger::new(vec![Box::new(a.clone()), Box::new(b.clone()), Box::new(c.clone())]);
        let e = Entitlements::of(Tier::Free);
        assert_eq!(l.remaining(&e), Some(FREE_TOTAL_MS));
        l.add(10 * 60_000, true);
        l.add(5 * 60_000, false);
        assert_eq!(l.usage().used_ms, 15 * 60_000);
        // 2か所を消しても、残りの1か所から戻る
        *a.0.lock().unwrap() = None;
        *b.0.lock().unwrap() = None;
        let l2 = Ledger::new(vec![Box::new(a.clone()), Box::new(b.clone()), Box::new(c.clone())]);
        assert_eq!((l2.usage().used_ms, l2.usage().count), (15 * 60_000, 1));
        assert!(a.read().is_some(), "書き戻される");
        assert_eq!(l2.remaining(&e), Some(45 * 60_000));
        assert_eq!(l2.remaining(&Entitlements::of(Tier::Pro)), None);
    }

    #[test]
    fn 書き換えた記録は使い切った扱い() {
        let (a, b) = (Mem::default(), Mem::default());
        let l = Ledger::new(vec![Box::new(a.clone()), Box::new(b.clone())]);
        l.add(30 * 60_000, true);
        // 数値だけ書き換える(署名が合わない)
        let v = a.read().unwrap().replace(&format!("{}", 30 * 60_000), "0");
        *a.0.lock().unwrap() = Some(v);
        let l2 = Ledger::new(vec![Box::new(a.clone()), Box::new(b.clone())]);
        assert!(l2.usage().tampered);
        assert_eq!(l2.remaining(&Entitlements::of(Tier::Free)), Some(0));
        assert!(decode("v1|1|1|00").is_err() && decode("x").is_err() && decode("").unwrap().is_none());
    }

    #[test]
    #[cfg(not(any(feature = "edition-free", feature = "edition-pro")))]
    fn 配布用のビルドは環境変数で有料版にならず_ライセンスだけが有料版にする() {
        assert_eq!(resolve_tier(false, Some("pro"), false), Tier::Free);
        assert_eq!(resolve_tier(false, None, false), Tier::Free);
        assert_eq!(resolve_tier(true, Some("free"), false), Tier::Pro);
        assert_eq!(resolve_tier(false, None, true), Tier::Pro);
        assert_eq!(resolve_tier(false, Some("free"), true), Tier::Free);
    }

    #[test]
    fn distribution_edition_is_fixed() {
        if let Some(tier) = distribution_tier() {
            for license in [false, true] {
                for debug in [false, true] {
                    for env in [None, Some("free"), Some("pro")] {
                        assert_eq!(resolve_tier(license, env, debug), tier);
                    }
                }
            }
            let ent = Entitlements::of(tier);
            assert_eq!(ent.summary, cfg!(feature = "edition-pro"));
        }
    }

    #[test]
    fn 無料版と有料版の機能() {
        let f = Entitlements::of(Tier::Free);
        assert!(f.can_export("txt") && !f.can_export("docx") && !f.can_export("pdf") && !f.diarize && !f.accurate_model);
        let p = Entitlements::of(Tier::Pro);
        assert!(p.can_export("docx") && p.diarize && p.total_limit_ms.is_none());
    }
}
