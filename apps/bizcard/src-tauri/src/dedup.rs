//! 重複の検知。**候補を返すだけで、統合はしない**。
//!
//! SPEC.md の方針: 「メールが同じ」または「氏名と会社が同じ」を「同じ人かもしれません」と
//! 提示し、統合するかは利用者が確認画面で決める。このモジュールには統合・削除・上書きを行う
//! 関数を置かない(入力はすべて読み取り専用の参照)。
//!
//! 注意: `info@` のような共有メールでは別人でも一致する。理由(`Reason`)を画面に出して、
//! 利用者が判断できるようにする。

use crate::extract::squash;

#[derive(Debug, Clone, PartialEq)]
pub struct PersonKey {
    pub id: i64,
    pub name: String,
    pub company: String,
    pub emails: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Reason {
    /// 同じメール(正規化後の値)
    SameEmail(String),
    SameNameAndCompany,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DuplicateCandidate {
    pub existing_id: i64,
    pub reasons: Vec<Reason>,
}

pub fn normalize_email(e: &str) -> String {
    let t = squash(e).to_lowercase();
    t.strip_prefix("mailto:").unwrap_or(&t).to_string()
}

/// 氏名の比較用の形。日本語は空白を除く。英字は語の順を問わず(Taro Yamada / YAMADA Taro)
pub fn normalize_person_name(n: &str) -> String {
    let n = crate::extract::normalize(n);
    if n.chars().any(|c| !c.is_ascii()) {
        return n.chars().filter(|c| !c.is_whitespace()).collect();
    }
    let mut toks: Vec<String> = n
        .split_whitespace()
        .map(|t| t.trim_matches(|c: char| ".,".contains(c)).to_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    toks.sort();
    toks.join(" ")
}

const LEGAL: &[&str] = &[
    "株式会社", "有限会社", "合同会社", "合資会社", "合名会社", "(株)", "(有)", "㈱", "㈲", "一般社団法人", "一般財団法人",
];

/// 会社名の比較用の形。法人格の語・空白・記号・大文字小文字の違いを無視する
pub fn normalize_company(c: &str) -> String {
    let mut s = crate::extract::normalize(c);
    for l in LEGAL {
        s = s.replace(l, "");
    }
    let re = regex::Regex::new(r"(?i)\b(inc|incorporated|corp|corporation|co|ltd|limited|llc|gmbh)\b\.?").unwrap();
    let s = re.replace_all(&s, "");
    s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase()
}

fn reasons(a: &PersonKey, b: &PersonKey) -> Vec<Reason> {
    let mut out = Vec::new();
    let ea: Vec<String> = a.emails.iter().map(|e| normalize_email(e)).filter(|e| !e.is_empty()).collect();
    for e in b.emails.iter().map(|e| normalize_email(e)).filter(|e| !e.is_empty()) {
        if ea.contains(&e) && !out.contains(&Reason::SameEmail(e.clone())) {
            out.push(Reason::SameEmail(e));
        }
    }
    let (na, nb) = (normalize_person_name(&a.name), normalize_person_name(&b.name));
    let (ca, cb) = (normalize_company(&a.company), normalize_company(&b.company));
    if !na.is_empty() && na == nb && !ca.is_empty() && ca == cb {
        out.push(Reason::SameNameAndCompany);
    }
    out
}

/// 新しく読み取った人(`candidate`)と、保存済みの人たちを比べ、同じ人かもしれない候補を返す。
pub fn find_duplicates(candidate: &PersonKey, existing: &[PersonKey]) -> Vec<DuplicateCandidate> {
    existing
        .iter()
        .filter(|e| e.id != candidate.id)
        .filter_map(|e| {
            let r = reasons(candidate, e);
            (!r.is_empty()).then(|| DuplicateCandidate { existing_id: e.id, reasons: r })
        })
        .collect()
}

/// 保存済みの全員の中から、同じ人かもしれない組(id の小さいほうが先)を返す。
pub fn find_duplicate_pairs(all: &[PersonKey]) -> Vec<(i64, i64, Vec<Reason>)> {
    let mut out = Vec::new();
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            if a.id == b.id {
                continue;
            }
            let r = reasons(a, b);
            if !r.is_empty() {
                out.push((a.id.min(b.id), a.id.max(b.id), r));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(id: i64, name: &str, company: &str, emails: &[&str]) -> PersonKey {
        PersonKey { id, name: name.into(), company: company.into(), emails: emails.iter().map(|s| s.to_string()).collect() }
    }

    #[test]
    fn メールが同じなら候補になる_大文字小文字と空白は無視() {
        let new = p(0, "山田 太郎", "株式会社A", &["Taro@Example.com"]);
        let ex = vec![p(1, "別の人", "B社", &[" taro@example.com"]), p(2, "他", "C社", &["x@example.com"])];
        let r = find_duplicates(&new, &ex);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].existing_id, 1);
        assert_eq!(r[0].reasons, vec![Reason::SameEmail("taro@example.com".into())]);
    }

    #[test]
    fn 氏名と会社が同じなら候補になる_表記ゆれを吸収する() {
        let new = p(0, "山田太郎", "(株)テスト商事", &[]);
        let ex = vec![p(1, "山田 太郎", "株式会社テスト商事", &[])];
        assert_eq!(find_duplicates(&new, &ex)[0].reasons, vec![Reason::SameNameAndCompany]);
        let new = p(0, "Taro Yamada", "Test Trading Inc.", &[]);
        let ex = vec![p(1, "YAMADA Taro", "TEST TRADING Co., Ltd.", &[])];
        assert_eq!(find_duplicates(&new, &ex).len(), 1);
    }

    #[test]
    fn 氏名だけ同じ_または会社だけ同じでは候補にしない() {
        let new = p(0, "山田 太郎", "株式会社A", &[]);
        let ex = vec![p(1, "山田 太郎", "株式会社B", &[]), p(2, "山田 次郎", "株式会社A", &[])];
        assert!(find_duplicates(&new, &ex).is_empty());
    }

    #[test]
    fn 空の氏名や会社同士は一致とみなさない() {
        let new = p(0, "", "", &[""]);
        let ex = vec![p(1, "", "", &[""])];
        assert!(find_duplicates(&new, &ex).is_empty());
    }

    #[test]
    fn 自分自身は候補にしない() {
        let a = p(1, "山田 太郎", "株式会社A", &["a@example.com"]);
        assert!(find_duplicates(&a, &[a.clone()]).is_empty());
    }

    #[test]
    fn 全員の中から組を返し_入力は変更されない() {
        let all = vec![
            p(1, "山田 太郎", "株式会社A", &["a@example.com"]),
            p(2, "山田 太郎", "株式会社A", &[]),
            p(3, "佐藤 花子", "株式会社C", &["a@example.com"]),
        ];
        let before = all.clone();
        let pairs = find_duplicate_pairs(&all);
        assert_eq!(pairs.len(), 2);
        assert_eq!((pairs[0].0, pairs[0].1), (1, 2));
        assert_eq!((pairs[1].0, pairs[1].1), (1, 3));
        assert_eq!(all, before, "検知は入力を書き換えない(統合しない)");
    }
}
