//! 領収書から、仕訳の「候補」を作る。
//!
//! 税務の判断はしない。キーワードに当たった勘定科目を提案し、迷う場合や読み取りに不備がある
//! 場合は `needs_review` を立てて、人が確認する前提にする。ルールは利用者が差し替えられる。

use crate::receipt::Receipt;

#[derive(Debug, Clone, PartialEq)]
pub struct KeywordRule {
    pub keyword: String,
    pub account: String,
    /// 科目の判断が分かれやすいもの(会議費か接待交際費か、など)は要確認にする
    pub review: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JournalCandidate {
    pub date: String,
    pub debit_account: String,
    pub credit_account: String,
    pub amount: i64,
    pub memo: String,
    pub needs_review: bool,
}

pub const UNDECIDED: &str = "要確認";

fn rule(keyword: &str, account: &str, review: bool) -> KeywordRule {
    KeywordRule { keyword: keyword.to_string(), account: account.to_string(), review }
}

/// 既定のルール(上から順に最初に当たったものを採用)。
pub fn default_rules() -> Vec<KeywordRule> {
    let mut v = Vec::new();
    for k in ["電車", "地下鉄", "新幹線", "タクシー", "バス", "高速道路", "ETC"] {
        v.push(rule(k, "旅費交通費", false));
    }
    for k in ["切手", "郵便", "宅急便", "ヤマト", "佐川", "インターネット", "携帯"] {
        v.push(rule(k, "通信費", false));
    }
    for k in ["書店", "書籍", "新聞", "図書"] {
        v.push(rule(k, "新聞図書費", false));
    }
    for k in ["文具", "事務用品", "コピー用紙", "インク", "電池"] {
        v.push(rule(k, "消耗品費", false));
    }
    for k in ["電気", "ガス", "水道"] {
        v.push(rule(k, "水道光熱費", false));
    }
    for k in ["家賃", "駐車場"] {
        v.push(rule(k, "地代家賃", false));
    }
    for k in ["振込手数料", "手数料"] {
        v.push(rule(k, "支払手数料", false));
    }
    for k in ["カフェ", "珈琲", "コーヒー", "ランチ", "弁当", "レストラン", "居酒屋"] {
        v.push(rule(k, "会議費", true));
    }
    for k in ["Amazon", "アマゾン"] {
        v.push(rule(k, "消耗品費", true));
    }
    v
}

/// 仕訳候補を作る。`credit_account` は支払い方法に応じた貸方(例: 現金、事業主借)。
pub fn suggest_journal(r: &Receipt, rules: &[KeywordRule], credit_account: &str) -> JournalCandidate {
    let haystack = format!(
        "{} {}",
        r.vendor.as_deref().unwrap_or(""),
        r.summary.as_deref().unwrap_or("")
    );
    let matched = rules.iter().find(|k| haystack.contains(&k.keyword));
    let has_issues = !r.validate().is_empty();

    let (debit, rule_review) = match matched {
        Some(k) => (k.account.clone(), k.review),
        None => (UNDECIDED.to_string(), true),
    };

    let memo = match (r.vendor.as_deref(), r.summary.as_deref()) {
        (Some(v), Some(s)) => format!("{v} {s}"),
        (Some(v), None) => v.to_string(),
        (None, Some(s)) => s.to_string(),
        (None, None) => String::new(),
    };

    JournalCandidate {
        date: r.date.clone().unwrap_or_default(),
        debit_account: debit,
        credit_account: credit_account.to_string(),
        amount: r.total.unwrap_or(0),
        memo,
        needs_review: has_issues || rule_review,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(vendor: &str, summary: &str) -> Receipt {
        Receipt {
            date: Some("2026-10-03".into()),
            vendor: Some(vendor.into()),
            total: Some(580),
            tax_rate: Some(10),
            invoice_no: None,
            summary: Some(summary.into()),
        }
    }

    #[test]
    fn キーワードに当たれば科目を提案し_問題なければ確認不要() {
        let j = suggest_journal(&receipt("JR東日本", "電車運賃"), &default_rules(), "現金");
        assert_eq!(j.debit_account, "旅費交通費");
        assert_eq!(j.credit_account, "現金");
        assert_eq!(j.amount, 580);
        assert!(!j.needs_review);
    }

    #[test]
    fn 判断が分かれる科目は要確認() {
        let j = suggest_journal(&receipt("スターカフェ", "コーヒー"), &default_rules(), "現金");
        assert_eq!(j.debit_account, "会議費");
        assert!(j.needs_review);
    }

    #[test]
    fn 当たらなければ未確定にして要確認() {
        let j = suggest_journal(&receipt("謎の店", "不明"), &default_rules(), "現金");
        assert_eq!(j.debit_account, UNDECIDED);
        assert!(j.needs_review);
    }

    #[test]
    fn 読み取りに不備があれば要確認() {
        let mut r = receipt("JR東日本", "電車運賃");
        r.date = None;
        assert!(suggest_journal(&r, &default_rules(), "現金").needs_review);
    }

    #[test]
    fn 利用者のルールで差し替えられる() {
        let rules = vec![KeywordRule { keyword: "謎の店".into(), account: "雑費".into(), review: false }];
        let j = suggest_journal(&receipt("謎の店", "不明"), &rules, "事業主借");
        assert_eq!(j.debit_account, "雑費");
        assert_eq!(j.credit_account, "事業主借");
    }
}
