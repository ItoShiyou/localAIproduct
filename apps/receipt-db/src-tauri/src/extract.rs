//! 書類 → 本文 → 読み取り結果(下書き)。
//!
//! - PDF にテキスト層があれば、それを直接読む(OCR しない)。
//! - テキスト層が無い PDF は、ページに埋め込まれた JPEG 画像(スキャナー出力に多い形式)を取り出して OCR に渡す。
//!   **JPEG 以外の埋め込み形式や、ベクター描画だけのページは対応しない**(エラーとして返し、確認画面で手入力できる)。
//! - 画像(JPEG/PNG)は OCR に渡す。
//! - OCR が2つ目の認識モデルの結果(`alt_text`)を持っていれば、数字の項目はそちら、支払先は通常の結果から取る。

use crate::receipt::Receipt;
use crate::rules::extract_by_rules;
use crate::vendor::guess_vendor;
use factory_core::ocr::{Ocr, OcrImage, OcrPage};
use std::collections::BTreeMap;

/// これを下回る信頼度の項目には、確認画面で印を付ける。
pub const LOW_CONFIDENCE: f32 = 0.8;

#[derive(Debug, Clone, PartialEq)]
pub struct PageText {
    pub page: i64,
    /// 支払先など、文字の読みやすさ重視の本文
    pub text: String,
    /// 日付・金額などの数字重視の本文(併用モデルが無ければ `text` と同じ)
    pub numeric_text: String,
    /// 本文全体の信頼度(テキスト層は 1.0、OCR は行の平均)
    pub confidence: f32,
    pub from_text_layer: bool,
}

pub fn pdf_text_layer(bytes: &[u8]) -> Option<String> {
    let t = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(bytes)).ok()?.ok()?;
    (t.chars().filter(|c| !c.is_whitespace()).count() >= 10).then_some(t)
}

/// ページに埋め込まれた JPEG(DCTDecode)の画像を、ページ順に取り出す。
pub fn pdf_embedded_jpegs(bytes: &[u8]) -> Vec<(i64, Vec<u8>)> {
    let Ok(doc) = lopdf::Document::load_mem(bytes) else { return vec![] };
    let mut out = Vec::new();
    for (num, page_id) in doc.get_pages() {
        let Ok(images) = doc.get_page_images(page_id) else { continue };
        for im in images {
            let is_jpeg = im.filters.as_ref().map(|f| f.iter().any(|x| x == "DCTDecode")).unwrap_or(false);
            if is_jpeg && im.width > 100 && im.height > 100 {
                out.push((num as i64, im.content.to_vec()));
                break; // 1ページ1枚(最初の画像)
            }
        }
    }
    out
}

fn page_text(page: i64, p: &OcrPage) -> PageText {
    PageText { page, text: p.to_text(), numeric_text: p.to_alt_text(), confidence: p.mean_confidence(), from_text_layer: false }
}

/// 書類(PDF / 画像のバイト列)から、ページごとの本文を作る。失敗の理由は利用者に見せられる文面。
pub fn read_document(bytes: &[u8], is_pdf: bool, ocr: &dyn Ocr) -> Result<Vec<PageText>, String> {
    if is_pdf {
        if let Some(t) = pdf_text_layer(bytes) {
            return Ok(vec![PageText { page: 1, numeric_text: t.clone(), text: t, confidence: 1.0, from_text_layer: true }]);
        }
        let jpegs = pdf_embedded_jpegs(bytes);
        if jpegs.is_empty() {
            return Err("テキストを読み取れないPDFです(画像のみのPDFは、JPEG画像を埋め込んだ形式だけに対応しています)".into());
        }
        let mut pages = Vec::new();
        for (n, jpeg) in jpegs {
            let img = OcrImage::decode(&jpeg).map_err(|e| e.to_string())?;
            pages.push(page_text(n, &ocr.recognize(&img).map_err(|e| e.to_string())?));
        }
        Ok(pages)
    } else {
        let img = OcrImage::decode(bytes).map_err(|e| e.to_string())?;
        Ok(vec![page_text(1, &ocr.recognize(&img).map_err(|e| e.to_string())?)])
    }
}

/// 本文から下書きを作る。項目ごとの信頼度(0〜1)も返す。見つからなかった項目は None・信頼度 0。
pub fn draft_from_text(p: &PageText) -> (Receipt, BTreeMap<String, f32>) {
    let r = extract_by_rules(&p.numeric_text);
    let v = guess_vendor(&p.text);
    let mut conf = BTreeMap::new();
    let mut put = |k: &str, found: bool, c: f32| {
        conf.insert(k.to_string(), if found { c } else { 0.0 });
    };
    put("date", r.date.is_some(), p.confidence);
    put("total", r.total.is_some(), p.confidence);
    put("tax_rate", r.tax_rate.is_some(), p.confidence);
    put("invoice_no", r.invoice_no.is_some(), p.confidence);
    let (vendor, vconf) = match &v {
        Some(g) => (Some(g.name.clone()), if g.confident { p.confidence } else { p.confidence.min(0.5) }),
        None => (None, 0.0),
    };
    put("vendor", vendor.is_some(), vconf);
    put("summary", false, 0.0);
    (Receipt { date: r.date, vendor, total: r.total, tax_rate: r.tax_rate, invoice_no: r.invoice_no, summary: None }, conf)
}

pub fn low_confidence_fields(conf: &BTreeMap<String, f32>) -> Vec<String> {
    conf.iter().filter(|(_, c)| **c < LOW_CONFIDENCE).map(|(k, _)| k.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use factory_core::ocr::{BBox, FakeOcr, OcrLine};

    fn line(t: &str, y: f32) -> OcrLine {
        OcrLine { text: t.into(), bbox: BBox { x: 10.0, y, w: 200.0, h: 20.0 }, vertical: false, confidence: 0.9, alt_text: None }
    }

    fn png() -> Vec<u8> {
        let mut b = Vec::new();
        image::DynamicImage::new_luma8(8, 8).write_to(&mut std::io::Cursor::new(&mut b), image::ImageFormat::Png).unwrap();
        b
    }

    /// テスト用の最小PDF(テキスト層あり)
    fn text_pdf(text: &str) -> Vec<u8> {
        let content = format!("BT /F1 12 Tf 50 700 Td ({text}) Tj ET");
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_string(),
            format!("<< /Length {} >>\nstream\n{}\nendstream", content.len(), content),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        ];
        let mut out = b"%PDF-1.4\n".to_vec();
        let mut offs = vec![];
        for (i, o) in objs.iter().enumerate() {
            offs.push(out.len());
            out.extend(format!("{} 0 obj\n{}\nendobj\n", i + 1, o).as_bytes());
        }
        let x = out.len();
        out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
        for o in offs {
            out.extend(format!("{o:010} 00000 n \n").as_bytes());
        }
        out.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n", objs.len() + 1).as_bytes());
        out
    }

    #[test]
    fn テキスト層ありのpdfはocrせずに読む() {
        let ocr = FakeOcr { page: OcrPage { width: 1.0, height: 1.0, lines: vec![] } };
        let pdf = text_pdf("Invoice Total 1,320 yen Date 2026/10/03 T1234567890123");
        let pages = read_document(&pdf, true, &ocr).unwrap();
        assert!(pages[0].from_text_layer && pages[0].text.contains("2026/10/03"));
        let (r, c) = draft_from_text(&pages[0]);
        assert_eq!(r.date.as_deref(), Some("2026-10-03"));
        assert_eq!(r.invoice_no.as_deref(), Some("T1234567890123"));
        assert_eq!(c["date"], 1.0);
    }

    #[test]
    fn テキストも画像も無いpdfは理由つきのエラー() {
        let ocr = FakeOcr { page: OcrPage { width: 1.0, height: 1.0, lines: vec![] } };
        let err = read_document(&text_pdf("x"), true, &ocr).unwrap_err();
        assert!(err.contains("画像のみのPDF"));
        assert!(read_document(b"%PDF-broken", true, &ocr).is_err());
    }

    #[test]
    fn 画像はocrの結果から下書きを作り_併用モデルの数字を使う() {
        let mut a = line("登録番号T123", 10.0);
        let mut total = line("合計 ¥l,320", 40.0);
        total.alt_text = Some("合計 ¥1,320".into());
        a.alt_text = Some("登録番号T1234567890123".into());
        let ocr = FakeOcr {
            page: OcrPage { width: 300.0, height: 100.0, lines: vec![line("株式会社さくら商店", 0.0), a, line("2026年10月3日", 25.0), total] },
        };
        let pages = read_document(&png(), false, &ocr).unwrap();
        let (r, c) = draft_from_text(&pages[0]);
        assert_eq!(r.vendor.as_deref(), Some("株式会社さくら商店"));
        assert_eq!((r.total, r.invoice_no.as_deref()), (Some(1320), Some("T1234567890123")));
        assert_eq!(r.date.as_deref(), Some("2026-10-03"));
        assert!(c["date"] > 0.8 && c["summary"] == 0.0);
        assert!(low_confidence_fields(&c).contains(&"summary".to_string()));
    }

    #[test]
    fn 読めない画像はエラー_項目が無ければ信頼度ゼロ() {
        let ocr = FakeOcr { page: OcrPage { width: 1.0, height: 1.0, lines: vec![] } };
        assert!(read_document(b"not image", false, &ocr).is_err());
        let (r, c) = draft_from_text(&PageText { page: 1, text: "".into(), numeric_text: "".into(), confidence: 0.9, from_text_layer: false });
        assert_eq!(r, Receipt::default());
        assert!(c.values().all(|v| *v == 0.0));
    }
}
