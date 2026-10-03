//! テストセット(`testset/cards/*.ocr.json` と `*.expected.json`)に対する抽出結果の採点。
//!
//! テストセットは**架空・手書きのOCR出力もどき**であり、実画像のOCR結果ではない。
//! また、ルールはこのテストセットを見ながら書いたので、ここでの正答率は
//! 「ルールが想定した形にどれだけ合っているか」を示すだけで、実際の名刺での精度を表さない。

use crate::extract::{extract, squash, Extraction};
use crate::ocr_input::OcrCard;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Expected {
    pub name: Option<String>,
    pub name_kana: Option<String>,
    pub company: Option<String>,
    pub department: Option<String>,
    pub title: Option<String>,
    #[serde(default)]
    pub emails: Vec<String>,
    #[serde(default)]
    pub phones: Vec<String>,
    #[serde(default)]
    pub mobiles: Vec<String>,
    #[serde(default)]
    pub faxes: Vec<String>,
    pub postal_code: Option<String>,
    pub address: Option<String>,
    #[serde(default)]
    pub urls: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Meta {
    #[serde(default)]
    hard: bool,
    #[serde(default)]
    note: String,
}

pub const FIELDS: [&str; 12] = [
    "name", "name_kana", "company", "department", "title", "emails", "phones", "mobiles", "faxes", "postal_code",
    "address", "urls",
];

pub struct CardResult {
    pub id: String,
    pub hard: bool,
    pub note: String,
    /// FIELDS と同じ順の正誤
    pub ok: [bool; 12],
    pub extracted: Extraction,
    pub expected: Expected,
}

pub fn testset_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset/cards")
}

fn one(f: &Option<crate::extract::Field>) -> Option<String> {
    f.as_ref().map(|f| squash(&f.value))
}
fn many(v: &[crate::extract::Field]) -> Vec<String> {
    let mut o: Vec<String> = v.iter().map(|f| squash(&f.value).to_lowercase()).collect();
    o.sort();
    o
}
fn exp_one(s: &Option<String>) -> Option<String> {
    s.as_ref().map(|s| squash(s))
}
fn exp_many(v: &[String]) -> Vec<String> {
    let mut o: Vec<String> = v.iter().map(|s| squash(s).to_lowercase()).collect();
    o.sort();
    o
}

pub fn compare(x: &Extraction, e: &Expected) -> [bool; 12] {
    [
        one(&x.name) == exp_one(&e.name),
        one(&x.name_kana) == exp_one(&e.name_kana),
        one(&x.company) == exp_one(&e.company),
        one(&x.department) == exp_one(&e.department),
        one(&x.title) == exp_one(&e.title),
        many(&x.emails) == exp_many(&e.emails),
        many(&x.phones) == exp_many(&e.phones),
        many(&x.mobiles) == exp_many(&e.mobiles),
        many(&x.faxes) == exp_many(&e.faxes),
        one(&x.postal_code) == exp_one(&e.postal_code),
        one(&x.address) == exp_one(&e.address),
        many(&x.urls) == exp_many(&e.urls),
    ]
}

pub fn run_testset() -> Vec<CardResult> {
    let dir = testset_dir();
    let mut ids: Vec<String> = std::fs::read_dir(&dir)
        .expect("testset/cards が読めません")
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".ocr.json").map(String::from)))
        .collect();
    ids.sort();
    ids.into_iter()
        .map(|id| {
            let ocr_txt = std::fs::read_to_string(dir.join(format!("{id}.ocr.json"))).unwrap();
            let card: OcrCard = serde_json::from_str(&ocr_txt).unwrap();
            let meta: Meta = serde_json::from_str(&ocr_txt).unwrap();
            let expected: Expected =
                serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{id}.expected.json"))).unwrap()).unwrap();
            let extracted = extract(&card);
            let ok = compare(&extracted, &expected);
            CardResult { id, hard: meta.hard, note: meta.note, ok, extracted, expected }
        })
        .collect()
}

/// 項目ごとの正答数 (正解数, 枚数)
pub fn tally(results: &[CardResult], only: impl Fn(&CardResult) -> bool) -> Vec<(&'static str, usize, usize)> {
    FIELDS
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let rs: Vec<&CardResult> = results.iter().filter(|r| only(r)).collect();
            (*name, rs.iter().filter(|r| r.ok[i]).count(), rs.len())
        })
        .collect()
}

/// メール・電話系(emails, phones, mobiles, faxes)をまとめた (全部正解の枚数, 枚数)
pub fn contact_tally(results: &[CardResult]) -> (usize, usize) {
    let ok = results.iter().filter(|r| r.ok[5] && r.ok[6] && r.ok[7] && r.ok[8]).count();
    (ok, results.len())
}

/// 会社名と氏名がともに正解の枚数
pub fn company_name_tally(results: &[CardResult]) -> (usize, usize, usize) {
    let c = results.iter().filter(|r| r.ok[2]).count();
    let n = results.iter().filter(|r| r.ok[0]).count();
    (c, n, results.len())
}
