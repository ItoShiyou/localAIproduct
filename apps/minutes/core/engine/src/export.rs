//! CSV書き出し。文字コードの選択と、表計算ソフトでの数式実行(CSVインジェクション)の防止を行う。
//!
//! 支払先や摘要は、OCRとモデルを通った「外部由来の文字列」。`=` や `+` で始まる値は、
//! Excelなどで数式として実行されるおそれがあるため、先頭に `'` を付けて無害化する。
//!
//! 会計ソフトごとの取り込み形式(freee、マネーフォワード、弥生など)にはまだ対応していない。
//! 各社の最新の仕様を確認してから、形式ごとに追加すること。

use crate::error::CoreError;
use crate::journal::JournalCandidate;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CsvEncoding {
    /// UTF-8(BOMつき)。Excelで開いても文字化けしにくい
    Utf8Bom,
    /// Shift_JIS。古い取り込みツール向け。変換できない文字があるとエラーにする
    ShiftJis,
}

const HEADER: [&str; 6] = ["日付", "借方勘定科目", "貸方勘定科目", "金額", "摘要", "要確認"];

/// 数式として解釈されうる先頭文字を無害化する。
pub fn sanitize_cell(s: &str) -> String {
    match s.chars().next() {
        Some('=') | Some('+') | Some('-') | Some('@') | Some('\t') | Some('\r') => format!("'{s}"),
        _ => s.to_string(),
    }
}

pub fn journal_to_csv(rows: &[JournalCandidate], enc: CsvEncoding) -> Result<Vec<u8>, CoreError> {
    let mut w = csv::WriterBuilder::new()
        .terminator(csv::Terminator::CRLF)
        .from_writer(Vec::new());
    w.write_record(HEADER).map_err(|e| CoreError::Csv(e.to_string()))?;
    for r in rows {
        let amount = r.amount.to_string();
        let flag = if r.needs_review { "要確認" } else { "" };
        w.write_record([
            sanitize_cell(&r.date).as_str(),
            sanitize_cell(&r.debit_account).as_str(),
            sanitize_cell(&r.credit_account).as_str(),
            amount.as_str(),
            sanitize_cell(&r.memo).as_str(),
            flag,
        ])
        .map_err(|e| CoreError::Csv(e.to_string()))?;
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

    fn row(memo: &str) -> JournalCandidate {
        JournalCandidate {
            date: "2026-10-03".into(),
            debit_account: "消耗品費".into(),
            credit_account: "現金".into(),
            amount: 1200,
            memo: memo.into(),
            needs_review: false,
        }
    }

    #[test]
    fn 数式になりうる値を無害化する() {
        assert_eq!(sanitize_cell("=SUM(A1)"), "'=SUM(A1)");
        assert_eq!(sanitize_cell("+81-3-0000"), "'+81-3-0000");
        assert_eq!(sanitize_cell("@cmd"), "'@cmd");
        assert_eq!(sanitize_cell("通常の文字"), "通常の文字");
        assert_eq!(sanitize_cell(""), "");
    }

    #[test]
    fn utf8はbomつきでヘッダと行が入る() {
        let out = journal_to_csv(&[row("コピー用紙")], CsvEncoding::Utf8Bom).unwrap();
        assert_eq!(&out[..3], &[0xEF, 0xBB, 0xBF]);
        let text = String::from_utf8(out[3..].to_vec()).unwrap();
        assert!(text.starts_with("日付,借方勘定科目,貸方勘定科目,金額,摘要,要確認\r\n"));
        assert!(text.contains("2026-10-03,消耗品費,現金,1200,コピー用紙,\r\n"));
    }

    #[test]
    fn カンマや引用符を含む摘要はcsvとして壊れない() {
        let out = journal_to_csv(&[row("A,B \"C\"")], CsvEncoding::Utf8Bom).unwrap();
        let text = String::from_utf8(out[3..].to_vec()).unwrap();
        assert!(text.contains("\"A,B \"\"C\"\"\""));
    }

    #[test]
    fn 摘要の数式は出力でも無害化される() {
        let out = journal_to_csv(&[row("=HYPERLINK(\"http://x\")")], CsvEncoding::Utf8Bom).unwrap();
        let text = String::from_utf8(out[3..].to_vec()).unwrap();
        assert!(text.contains("'=HYPERLINK"));
        assert!(!text.contains(",=HYPERLINK"));
    }

    #[test]
    fn shift_jisに変換でき_戻すと元の文字になる() {
        let out = journal_to_csv(&[row("コピー用紙")], CsvEncoding::ShiftJis).unwrap();
        let (back, _, errs) = encoding_rs::SHIFT_JIS.decode(&out);
        assert!(!errs);
        assert!(back.contains("消耗品費") && back.contains("コピー用紙"));
    }

    #[test]
    fn shift_jisで表せない文字はエラーにする() {
        let r = journal_to_csv(&[row("😀")], CsvEncoding::ShiftJis);
        assert!(matches!(r, Err(CoreError::Encoding(_))));
    }

    #[test]
    fn 要確認の行にはフラグが立つ() {
        let mut r = row("x");
        r.needs_review = true;
        let out = journal_to_csv(&[r], CsvEncoding::Utf8Bom).unwrap();
        let text = String::from_utf8(out[3..].to_vec()).unwrap();
        assert!(text.contains(",x,要確認\r\n"));
    }
}
