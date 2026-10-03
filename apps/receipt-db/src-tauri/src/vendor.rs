//! 支払先の候補をルールで出す(生成モデルは使わない)。
//!
//! 先頭付近の行から、「株式会社」「有限会社」などの語を含む会社名らしい行を選ぶ。
//! 「宛」「御中」「様」で終わる行は宛名(請求先)なので除く。見つからなければ、先頭付近の
//! 文字だけの行を低い信頼度で返す。確定は人が行う前提なので、迷うときは `confident: false`。

use crate::rules::normalize;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq)]
pub struct VendorGuess {
    pub name: String,
    pub confident: bool,
}

const ENTITY_WORDS: [&str; 9] = [
    "株式会社", "有限会社", "合同会社", "合資会社", "一般社団法人", "一般財団法人", "社会福祉法人", "(株)", "(有)",
];
const SKIP_WORDS: [&str; 10] = [
    "領収書", "請求書", "納品書", "レシート", "登録番号", "合計", "小計", "TEL", "電話", "ご利用",
];

fn is_addressee(line: &str) -> bool {
    let t = line.trim_end();
    t.ends_with('宛') || t.ends_with("御中") || t.ends_with('様') || t.ends_with("殿")
}

fn looks_like_data(line: &str) -> bool {
    static DATE_OR_NUM: OnceLock<Regex> = OnceLock::new();
    let r = DATE_OR_NUM.get_or_init(|| Regex::new(r"\d{3,}|[¥]|\d+[:/年月日]").expect("正規表現"));
    r.is_match(line)
}

fn clean(line: &str) -> String {
    line.trim_matches(|c: char| c.is_whitespace() || "*＊・.-_=#".contains(c)).to_string()
}

pub fn guess_vendor(text: &str) -> Option<VendorGuess> {
    let t = normalize(text);
    let lines: Vec<String> = t.lines().map(clean).filter(|l| !l.is_empty()).take(8).collect();

    for l in &lines {
        if is_addressee(l) || SKIP_WORDS.iter().any(|w| l.contains(w)) || looks_like_data(l) {
            continue;
        }
        if ENTITY_WORDS.iter().any(|w| l.contains(w)) {
            return Some(VendorGuess { name: l.clone(), confident: true });
        }
    }
    for l in &lines {
        if is_addressee(l) || SKIP_WORDS.iter().any(|w| l.contains(w)) || looks_like_data(l) {
            continue;
        }
        if l.chars().count() >= 2 {
            return Some(VendorGuess { name: l.clone(), confident: false });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 先頭の会社名らしい行を採る() {
        let v = guess_vendor("株式会社サンプル商店\n登録番号 T1234567890123\n2026年10月3日").unwrap();
        assert_eq!(v, VendorGuess { name: "株式会社サンプル商店".into(), confident: true });
    }

    #[test]
    fn 宛名は支払先にしない() {
        let v = guess_vendor("請求書\n有限会社みどり商店\n株式会社テスト宛\n発行日 2026-10-03").unwrap();
        assert_eq!(v.name, "有限会社みどり商店");
    }

    #[test]
    fn 法人格の語がなければ低い信頼度で先頭行() {
        let v = guess_vendor("\nこもれび珈琲\n2026/10/03 10:00\n合計 ¥450").unwrap();
        assert_eq!(v, VendorGuess { name: "こもれび珈琲".into(), confident: false });
    }

    #[test]
    fn 数字や日付だけの行は候補にしない() {
        assert_eq!(guess_vendor("2026/10/03\n¥1,200\nT1234567890123"), None);
    }

    #[test]
    fn 空なら何も返さない() {
        assert_eq!(guess_vendor(""), None);
    }
}
