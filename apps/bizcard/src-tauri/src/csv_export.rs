//! 連絡先のCSV書き出し。数式として解釈される先頭文字の無害化は `core` の
//! `factory_core::export::sanitize_cell` を使い、文字コードの種類も `core` の `CsvEncoding` を使う。
//!
//! 要確認: `core/README.md` は「`journal_to_csv` を汎用の `write_csv(header, rows, encoding)` に
//! 切り出す」と書いているが、いまの `core` には汎用版が無い。`core/` は変更しない決まりなので、
//! 暫定で行の組み立てと文字コード変換だけをここに持つ(無害化は core のものを必ず通す)。
//! core に `write_csv` が入ったら、`write_rows` を core の呼び出しに置き換える。

use factory_core::error::CoreError;
use factory_core::export::{sanitize_cell, CsvEncoding};

pub const HEADER: [&str; 15] = [
    "氏名", "ふりがな", "会社", "部署", "役職", "メール", "電話", "携帯", "郵便番号", "住所", "URL", "最初に会った日", "場所",
    "タグ", "メモ(日付つき・新しい順)",
];

#[derive(Debug, Clone, Default)]
pub struct ExportPerson {
    pub name: String,
    pub name_kana: String,
    pub company: String,
    pub department: String,
    pub title: String,
    pub email: String,
    pub phone: String,
    pub mobile: String,
    pub postal_code: String,
    pub address: String,
    pub url: String,
    pub first_met_on: String,
    pub place: String,
    pub tags: Vec<String>,
    /// (日付, メモ)。古い順で渡してよい(出力は新しい順に並べ直す)
    pub memos: Vec<(String, String)>,
}

fn memo_text(m: &[(String, String)]) -> String {
    let mut v: Vec<&(String, String)> = m.iter().collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v.iter().map(|(d, t)| format!("{d} {t}")).collect::<Vec<_>>().join("\n")
}

/// 連絡先をCSVにする。すべてのセルを無害化してから書く。
pub fn people_to_csv(people: &[ExportPerson], enc: CsvEncoding) -> Result<Vec<u8>, CoreError> {
    let rows: Vec<Vec<String>> = people
        .iter()
        .map(|p| {
            vec![
                p.name.clone(), p.name_kana.clone(), p.company.clone(), p.department.clone(), p.title.clone(),
                p.email.clone(), p.phone.clone(), p.mobile.clone(), p.postal_code.clone(), p.address.clone(),
                p.url.clone(), p.first_met_on.clone(), p.place.clone(), p.tags.join(" "), memo_text(&p.memos),
            ]
        })
        .collect();
    write_rows(&HEADER, &rows, enc)
}

/// 暫定の汎用書き出し(core に `write_csv` が入るまで)。ヘッダ以外の全セルを無害化する。
fn write_rows(header: &[&str], rows: &[Vec<String>], enc: CsvEncoding) -> Result<Vec<u8>, CoreError> {
    let mut w = csv::WriterBuilder::new().terminator(csv::Terminator::CRLF).from_writer(Vec::new());
    w.write_record(header).map_err(|e| CoreError::Csv(e.to_string()))?;
    for r in rows {
        let cells: Vec<String> = r.iter().map(|c| sanitize_cell(c)).collect();
        w.write_record(&cells).map_err(|e| CoreError::Csv(e.to_string()))?;
    }
    let bytes = w.into_inner().map_err(|e| CoreError::Csv(e.to_string()))?;
    let text = String::from_utf8(bytes).map_err(|e| CoreError::Csv(e.to_string()))?;
    match enc {
        CsvEncoding::Utf8Bom => {
            let mut out = vec![0xEF, 0xBB, 0xBF];
            out.extend_from_slice(text.as_bytes());
            Ok(out)
        }
        CsvEncoding::ShiftJis => {
            let (bytes, _, had_errors) = encoding_rs::SHIFT_JIS.encode(&text);
            if had_errors {
                return Err(CoreError::Encoding(
                    "Shift_JISで表せない文字が含まれています。UTF-8(BOMつき)で書き出してください".to_string(),
                ));
            }
            Ok(bytes.into_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person() -> ExportPerson {
        ExportPerson { name: "青木 遥".into(), company: "株式会社ひなた工房".into(), email: "aoki@example.com".into(), ..Default::default() }
    }
    fn parse(out: &[u8]) -> Vec<Vec<String>> {
        let text = String::from_utf8(out[3..].to_vec()).unwrap();
        csv::ReaderBuilder::new().has_headers(false).from_reader(text.as_bytes()).records().map(|r| r.unwrap().iter().map(String::from).collect()).collect()
    }

    #[test]
    fn 先頭が数式記号のセルは全項目で無害化される() {
        let mut p = person();
        p.name = "=HYPERLINK(\"http://x.invalid\")".into();
        p.company = "+cmd".into();
        p.title = "-1+1".into();
        p.email = "@SUM(A1)".into();
        p.phone = "+81-3-0000-0000".into();
        p.address = "\t=1+1".into();
        p.place = "\r=1".into();
        p.url = "=1".into();
        p.memos = vec![("2026-10-01".into(), "x".into())];
        p.tags = vec!["=tag".into()];
        let out = people_to_csv(&[p], CsvEncoding::Utf8Bom).unwrap();
        let rows = parse(&out);
        for cell in &rows[1] {
            assert!(!matches!(cell.chars().next(), Some('=' | '+' | '-' | '@' | '\t' | '\r')), "無害化されていないセル: {cell:?}");
        }
        assert_eq!(rows[1][0], "'=HYPERLINK(\"http://x.invalid\")");
        assert_eq!(rows[1][6], "'+81-3-0000-0000");
    }

    #[test]
    fn メモの各行に数式を入れても_先頭行以外は残るが_セル先頭は無害化される() {
        // 改行を含むセルは先頭だけが数式の判定対象になる。2行目以降の `=` は表計算ソフトでは数式にならない
        let mut p = person();
        p.memos = vec![("2026-10-01".into(), "初回".into()), ("2026-10-05".into(), "=1+1".into())];
        let rows = parse(&people_to_csv(&[p], CsvEncoding::Utf8Bom).unwrap());
        assert_eq!(rows[1][14], "2026-10-05 =1+1\n2026-10-01 初回");
    }

    #[test]
    fn ヘッダはbom付きutf8で_日本語がそのまま入る() {
        let out = people_to_csv(&[person()], CsvEncoding::Utf8Bom).unwrap();
        assert_eq!(&out[..3], &[0xEF, 0xBB, 0xBF]);
        let rows = parse(&out);
        assert_eq!(rows[0][0], "氏名");
        assert_eq!(rows[1][2], "株式会社ひなた工房");
    }

    #[test]
    fn shift_jisで書け_表せない文字はエラー() {
        let out = people_to_csv(&[person()], CsvEncoding::ShiftJis).unwrap();
        let (back, _, errs) = encoding_rs::SHIFT_JIS.decode(&out);
        assert!(!errs && back.contains("青木"));
        let mut p = person();
        p.name = "😀".into();
        assert!(matches!(people_to_csv(&[p], CsvEncoding::ShiftJis), Err(CoreError::Encoding(_))));
    }

    #[test]
    fn カンマと改行を含むセルでも列がずれない() {
        let mut p = person();
        p.address = "東京都, 千代田区\n1-1".into();
        let rows = parse(&people_to_csv(&[p], CsvEncoding::Utf8Bom).unwrap());
        assert_eq!(rows[1].len(), HEADER.len());
        assert_eq!(rows[1][9], "東京都, 千代田区\n1-1");
    }
}
