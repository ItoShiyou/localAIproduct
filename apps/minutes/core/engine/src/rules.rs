//! ルールによる抽出(モデルを使わない部分)。
//!
//! 想定する最低ライン(2022年頃のPC、Core i5-12500級、メモリ16GB、VRAMほぼなし)では、
//! モデルの生成は遅い。日付・金額・税率・登録番号は、文字のパターンで確定できることが多いので、
//! 先にここで取り出し、モデルには「ルールでは取れない項目」だけを任せる。
//!
//! 取り出せなかった項目は None のまま返す(推測で埋めない)。

use crate::receipt::is_valid_date;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct RuleExtract {
    pub date: Option<String>,
    pub total: Option<i64>,
    pub tax_rate: Option<u8>,
    pub invoice_no: Option<String>,
}

/// 全角の英数字・記号を半角にそろえる(OCRの出力は全角が混ざる)。
pub fn normalize(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '０'..='９' => char::from(b'0' + (c as u32 - '０' as u32) as u8),
            'Ａ'..='Ｚ' => char::from(b'A' + (c as u32 - 'Ａ' as u32) as u8),
            'ａ'..='ｚ' => char::from(b'a' + (c as u32 - 'ａ' as u32) as u8),
            '￥' => '¥',
            '，' => ',',
            '．' => '.',
            '／' => '/',
            '－' | '−' => '-',
            '％' => '%',
            '　' => ' ',
            _ => c,
        })
        .collect()
}

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).expect("正規表現"))
}

fn find_date(text: &str) -> Option<String> {
    static WESTERN: OnceLock<Regex> = OnceLock::new();
    static ERA: OnceLock<Regex> = OnceLock::new();

    let western = re(&WESTERN, r"(20\d{2})\s*[/\-.年]\s*(\d{1,2})\s*[/\-.月]\s*(\d{1,2})");
    let era = re(&ERA, r"(令和|平成|R|H)\s*(元|\d{1,2})\s*年\s*(\d{1,2})\s*月\s*(\d{1,2})");

    // 文章中の出現順で、最初に実在する日付を採る
    let mut candidates: Vec<(usize, String)> = Vec::new();
    for c in western.captures_iter(text) {
        let pos = c.get(0).map(|m| m.start()).unwrap_or(0);
        let s = format!("{}-{:0>2}-{:0>2}", &c[1], &c[2], &c[3]);
        if is_valid_date(&s) {
            candidates.push((pos, s));
        }
    }
    for c in era.captures_iter(text) {
        let pos = c.get(0).map(|m| m.start()).unwrap_or(0);
        let n: i32 = if &c[2] == "元" { 1 } else { c[2].parse().unwrap_or(0) };
        let base = match &c[1] {
            "令和" | "R" => 2018,
            _ => 1988, // 平成
        };
        let s = format!("{}-{:0>2}-{:0>2}", base + n, &c[3], &c[4]);
        if is_valid_date(&s) {
            candidates.push((pos, s));
        }
    }
    candidates.sort_by_key(|(p, _)| *p);
    candidates.into_iter().next().map(|(_, s)| s)
}

fn amounts_in(line: &str) -> Vec<i64> {
    static NUM: OnceLock<Regex> = OnceLock::new();
    let r = re(&NUM, r"[0-9]{1,3}(?:,[0-9]{3})+|[0-9]+");
    r.find_iter(line)
        .filter_map(|m| m.as_str().replace(',', "").parse::<i64>().ok())
        .collect()
}

/// 合計金額。「小計」は税抜きのことがあるので使わない。キーワードは上から優先する。
fn find_total(text: &str) -> Option<i64> {
    const KEYWORDS: [&str; 8] = [
        "税込合計", "ご請求金額", "請求金額", "領収金額", "お買上", "ご利用金額", "総額", "合計",
    ];
    let lines: Vec<&str> = text.lines().collect();
    for kw in KEYWORDS {
        for (i, line) in lines.iter().enumerate() {
            if !line.contains(kw) {
                continue;
            }
            // 同じ行に金額がなければ、次の行を見る(表形式のOCRで起きる)
            let mut nums = amounts_in(line);
            if nums.is_empty() {
                if let Some(next) = lines.get(i + 1) {
                    nums = amounts_in(next);
                }
            }
            if let Some(max) = nums.into_iter().max() {
                if max > 0 {
                    return Some(max);
                }
            }
        }
    }
    None
}

/// 税率。10%と8%が両方あるときは(混在なので)決めない。
fn find_tax_rate(text: &str) -> Option<u8> {
    static TEN: OnceLock<Regex> = OnceLock::new();
    static EIGHT: OnceLock<Regex> = OnceLock::new();
    let ten = re(&TEN, r"(?:^|[^0-9])10\s*%").is_match(text);
    let eight = re(&EIGHT, r"(?:^|[^0-9])8\s*%").is_match(text);
    match (ten, eight) {
        (true, false) => Some(10),
        (false, true) => Some(8),
        _ => None,
    }
}

fn find_invoice_no(text: &str) -> Option<String> {
    static INV: OnceLock<Regex> = OnceLock::new();
    let r = re(&INV, r"T[- ]?(\d{13})");
    r.captures(text).map(|c| format!("T{}", &c[1]))
}

pub fn extract_by_rules(text: &str) -> RuleExtract {
    let t = normalize(text);
    RuleExtract {
        date: find_date(&t),
        total: find_total(&t),
        tax_rate: find_tax_rate(&t),
        invoice_no: find_invoice_no(&t),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "株式会社サンプル商店\n登録番号 T1234567890123\n2026年10月3日 14:22\nコピー用紙 A4      ¥580\nインク            ¥620\n小計   ¥1,200\n(10%対象 ¥1,200)\n合計   ¥1,320\n";

    #[test]
    fn 典型的なレシートから4項目を取り出す() {
        let r = extract_by_rules(SAMPLE);
        assert_eq!(r.date.as_deref(), Some("2026-10-03"));
        assert_eq!(r.total, Some(1320)); // 小計(1,200)ではなく合計
        assert_eq!(r.tax_rate, Some(10));
        assert_eq!(r.invoice_no.as_deref(), Some("T1234567890123"));
    }

    #[test]
    fn 全角の数字も読める() {
        let r = extract_by_rules("２０２６／１０／０３\n合計　￥１，２００\nＴ１２３４５６７８９０１２３");
        assert_eq!(r.date.as_deref(), Some("2026-10-03"));
        assert_eq!(r.total, Some(1200));
        assert_eq!(r.invoice_no.as_deref(), Some("T1234567890123"));
    }

    #[test]
    fn 和暦を西暦に直す() {
        assert_eq!(extract_by_rules("令和8年10月3日").date.as_deref(), Some("2026-10-03"));
        assert_eq!(extract_by_rules("令和元年5月1日").date.as_deref(), Some("2019-05-01"));
        assert_eq!(extract_by_rules("平成30年12月31日").date.as_deref(), Some("2018-12-31"));
    }

    #[test]
    fn 実在しない日付は採らず_次の候補を見る() {
        let r = extract_by_rules("有効期限 2026/13/45\n発行日 2026-10-03");
        assert_eq!(r.date.as_deref(), Some("2026-10-03"));
    }

    #[test]
    fn 税込合計は合計より優先される() {
        let r = extract_by_rules("合計 ¥1,000\n税込合計 ¥1,100");
        assert_eq!(r.total, Some(1100));
    }

    #[test]
    fn 金額が次の行にあっても拾う() {
        assert_eq!(extract_by_rules("合計\n¥3,980").total, Some(3980));
    }

    #[test]
    fn 税率が混在していたら決めない() {
        assert_eq!(extract_by_rules("8%対象 ¥500\n10%対象 ¥1,000").tax_rate, None);
        assert_eq!(extract_by_rules("8%対象 ¥500").tax_rate, Some(8));
        assert_eq!(extract_by_rules("税率なし").tax_rate, None);
        // 18% や 108円 を 8% と誤認しない
        assert_eq!(extract_by_rules("18%引き").tax_rate, None);
    }

    #[test]
    fn 見つからない項目はnoneのまま() {
        let r = extract_by_rules("何も書かれていない");
        assert_eq!(r, RuleExtract::default());
    }
}
