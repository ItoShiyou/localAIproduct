//! vCard 3.0(RFC 2426)の書き出し。外部由来の文字列に含まれる改行・`;`・`,` は必ずエスケープし、
//! 値の中から別の項目(`END:VCARD` など)を差し込めないようにする。75オクテットで行を折り返す。

use crate::csv_export::ExportPerson;

/// 値のエスケープ(バックスラッシュ、改行、`;`、`,`)
pub fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => {}
            ';' => o.push_str("\\;"),
            ',' => o.push_str("\\,"),
            _ => o.push(c),
        }
    }
    o
}

/// 75オクテットを超える行を、文字の途中で切らずに折り返す(続きの行は先頭に空白1つ)
fn fold(line: &str) -> String {
    let mut out = String::new();
    let mut cur = 0usize;
    for c in line.chars() {
        let n = c.len_utf8();
        if cur + n > 75 {
            out.push_str("\r\n ");
            cur = 1;
        }
        out.push(c);
        cur += n;
    }
    out.push_str("\r\n");
    out
}

fn split_name(name: &str) -> (String, String) {
    let mut it = name.split_whitespace();
    match (it.next(), it.next()) {
        (Some(a), Some(b)) => {
            let rest: Vec<&str> = it.collect();
            let given = if rest.is_empty() { b.to_string() } else { format!("{b} {}", rest.join(" ")) };
            (a.to_string(), given)
        }
        (Some(a), None) => (a.to_string(), String::new()),
        _ => (String::new(), String::new()),
    }
}

pub fn person_to_vcard(p: &ExportPerson) -> String {
    let (family, given) = split_name(&p.name);
    let mut l: Vec<String> = vec!["BEGIN:VCARD".into(), "VERSION:3.0".into()];
    l.push(format!("N:{};{};;;", escape(&family), escape(&given)));
    l.push(format!("FN:{}", escape(&p.name)));
    if !p.name_kana.is_empty() {
        l.push(format!("X-PHONETIC-NAME:{}", escape(&p.name_kana)));
    }
    if !p.company.is_empty() || !p.department.is_empty() {
        l.push(format!("ORG:{};{}", escape(&p.company), escape(&p.department)));
    }
    if !p.title.is_empty() {
        l.push(format!("TITLE:{}", escape(&p.title)));
    }
    if !p.phone.is_empty() {
        l.push(format!("TEL;TYPE=WORK,VOICE:{}", escape(&p.phone)));
    }
    if !p.mobile.is_empty() {
        l.push(format!("TEL;TYPE=CELL:{}", escape(&p.mobile)));
    }
    if !p.email.is_empty() {
        l.push(format!("EMAIL;TYPE=INTERNET:{}", escape(&p.email)));
    }
    if !p.url.is_empty() {
        l.push(format!("URL:{}", escape(&p.url)));
    }
    if !p.address.is_empty() || !p.postal_code.is_empty() {
        l.push(format!("ADR;TYPE=WORK:;;{};;;{};", escape(&p.address), escape(&p.postal_code)));
    }
    if !p.tags.is_empty() {
        l.push(format!("CATEGORIES:{}", p.tags.iter().map(|t| escape(t)).collect::<Vec<_>>().join(",")));
    }
    let note: Vec<String> = p.memos.iter().map(|(d, m)| format!("{d} {m}").trim().to_string()).collect();
    if !note.is_empty() {
        l.push(format!("NOTE:{}", escape(&note.join("\n"))));
    }
    l.push("END:VCARD".into());
    l.iter().map(|x| fold(x)).collect()
}

pub fn people_to_vcard(people: &[ExportPerson]) -> String {
    people.iter().map(person_to_vcard).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> ExportPerson {
        ExportPerson { name: "青木 遥".into(), company: "株式会社ひなた工房".into(), department: "制作部".into(), email: "aoki@example.com".into(), mobile: "090-0000-1101".into(), ..Default::default() }
    }

    #[test]
    fn 基本の項目が入る() {
        let v = person_to_vcard(&p());
        assert!(v.starts_with("BEGIN:VCARD\r\nVERSION:3.0\r\n"));
        assert!(v.contains("N:青木;遥;;;\r\n"));
        assert!(v.contains("ORG:株式会社ひなた工房;制作部\r\n"));
        assert!(v.contains("TEL;TYPE=CELL:090-0000-1101\r\n"));
        assert!(v.ends_with("END:VCARD\r\n"));
    }

    #[test]
    fn 改行やセミコロンで別の項目を差し込めない() {
        let mut x = p();
        x.name = "悪意\r\nEND:VCARD\r\nBEGIN:VCARD\r\nFN:別人".into();
        x.company = "a;b,c\\d".into();
        let v = person_to_vcard(&x);
        // 行として現れる BEGIN/END は1組だけ(値の中の文字列は FN の1行に閉じ込められる)
        assert_eq!(v.split("\r\n").filter(|l| *l == "BEGIN:VCARD").count(), 1);
        assert_eq!(v.split("\r\n").filter(|l| *l == "END:VCARD").count(), 1);
        assert!(!v.split("\r\n").any(|l| l.starts_with("FN:別人")));
        assert!(v.contains("ORG:a\\;b\\,c\\\\d;制作部"));
    }

    #[test]
    fn 長い行は75オクテットで折り返し_文字を割らない() {
        let mut x = p();
        x.memos = vec![("2026-10-01".into(), "あ".repeat(60))];
        let v = person_to_vcard(&x);
        for line in v.split("\r\n") {
            assert!(line.len() <= 75, "長すぎる行: {}", line.len());
        }
        let unfolded = v.replace("\r\n ", "");
        assert!(unfolded.contains(&format!("NOTE:2026-10-01 {}", "あ".repeat(60))));
    }
}
