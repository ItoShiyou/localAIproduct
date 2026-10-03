//! 文字認識(OCR)の共通部品。
//!
//! 画像を入れると、行ごとのテキスト・位置・縦書きフラグ・信頼度が返る。型は名刺管理アプリの
//! `ocr_input.rs`(行・位置・縦書きフラグ・信頼度)と同じ形にしてある。座標は画像の左上を原点とする
//! ピクセル。縦書きの行は `vertical = true` で、`bbox.w` が文字の大きさ、`bbox.h` が行の長さ。
//!
//! 実装:
//! - [`tesseract::TesseractOcr`] — Tesseract の実行ファイルを呼ぶ(Apache-2.0)。
//! - `ppocr::PpOcr`(feature `onnx`)— ONNX Runtime 上の PP-OCR(検出+認識)。
//! - [`FakeOcr`] — テスト用(決めた結果を返す)。
//!
//! 画像・認識結果は端末の外へ出さない。Tesseract へは標準入力で渡し、一時ファイルを作らない。

use crate::error::CoreError;
use serde::{Deserialize, Serialize};

pub mod tesseract;
#[cfg(feature = "onnx")]
pub mod ppocr;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl BBox {
    pub fn cx(&self) -> f32 {
        self.x + self.w / 2.0
    }
    pub fn cy(&self) -> f32 {
        self.y + self.h / 2.0
    }
    /// 2つの枠のすき間(重なっていれば0)。縦横それぞれのすき間の大きいほう。
    pub fn gap(&self, o: &BBox) -> f32 {
        let dx = (o.x - (self.x + self.w)).max(self.x - (o.x + o.w)).max(0.0);
        let dy = (o.y - (self.y + self.h)).max(self.y - (o.y + o.h)).max(0.0);
        dx.max(dy)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrLine {
    pub text: String,
    pub bbox: BBox,
    #[serde(default)]
    pub vertical: bool,
    /// 信頼度(0〜1)。エンジンが返さなければ 1.0
    #[serde(default = "one")]
    pub confidence: f32,
}

fn one() -> f32 {
    1.0
}

impl OcrLine {
    /// 文字の大きさ(横書きは高さ、縦書きは幅)
    pub fn char_size(&self) -> f32 {
        if self.vertical {
            self.bbox.w
        } else {
            self.bbox.h
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrPage {
    pub width: f32,
    pub height: f32,
    pub lines: Vec<OcrLine>,
}

impl OcrPage {
    /// 読み順(上から下、同じ高さなら左から右)の本文。横書きだけを使い、同じ高さの断片は
    /// 空白2つでつなぐ(表の「品名  ¥580」のような並びを1行にするため)。縦書きは末尾に1行ずつ。
    pub fn to_text(&self) -> String {
        let mut horiz: Vec<&OcrLine> = self.lines.iter().filter(|l| !l.vertical).collect();
        horiz.sort_by(|a, b| a.bbox.cy().total_cmp(&b.bbox.cy()));
        let mut rows: Vec<(f32, f32, Vec<&OcrLine>)> = Vec::new(); // (中心y, 文字の大きさ, 断片)
        for l in horiz {
            match rows.last_mut() {
                Some((cy, size, items)) if (l.bbox.cy() - *cy).abs() < 0.5 * size.max(l.char_size()) => {
                    items.push(l);
                    *size = size.max(l.char_size());
                }
                _ => rows.push((l.bbox.cy(), l.char_size(), vec![l])),
            }
        }
        let mut out: Vec<String> = rows
            .into_iter()
            .map(|(_, _, mut items)| {
                items.sort_by(|a, b| a.bbox.x.total_cmp(&b.bbox.x));
                items.iter().map(|l| l.text.trim()).collect::<Vec<_>>().join("  ")
            })
            .collect();
        for l in self.lines.iter().filter(|l| l.vertical) {
            out.push(l.text.trim().to_string());
        }
        out.join("\n")
    }

    /// 平均の信頼度(行が無ければ 0)
    pub fn mean_confidence(&self) -> f32 {
        if self.lines.is_empty() {
            return 0.0;
        }
        self.lines.iter().map(|l| l.confidence).sum::<f32>() / self.lines.len() as f32
    }
}

/// OCRに渡す画像。PNG/JPEG のバイト列から作る(EXIFの回転は呼び出し側で直す)。
pub struct OcrImage {
    pub(crate) img: image::DynamicImage,
}

impl OcrImage {
    pub fn decode(bytes: &[u8]) -> Result<Self, CoreError> {
        let img = image::load_from_memory(bytes).map_err(|e| CoreError::Ocr(format!("画像を読めません: {e}")))?;
        Ok(Self { img })
    }
    pub fn from_dynamic(img: image::DynamicImage) -> Self {
        Self { img }
    }
    pub fn width(&self) -> u32 {
        self.img.width()
    }
    pub fn height(&self) -> u32 {
        self.img.height()
    }
}

pub trait Ocr {
    /// エンジンの名前(ログと設定画面用。個人データは含めない)
    fn name(&self) -> &str;
    fn recognize(&self, image: &OcrImage) -> Result<OcrPage, CoreError>;
}

/// テスト用。画像の中身に関係なく、決めた結果を返す。
pub struct FakeOcr {
    pub page: OcrPage,
}

impl Ocr for FakeOcr {
    fn name(&self) -> &str {
        "fake"
    }
    fn recognize(&self, _image: &OcrImage) -> Result<OcrPage, CoreError> {
        Ok(self.page.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, x: f32, y: f32, w: f32, h: f32) -> OcrLine {
        OcrLine { text: text.into(), bbox: BBox { x, y, w, h }, vertical: false, confidence: 0.9 }
    }

    #[test]
    fn 同じ高さの断片は1行にまとめ_上から下へ並べる() {
        let page = OcrPage {
            width: 400.0,
            height: 300.0,
            lines: vec![
                line("¥580", 300.0, 102.0, 60.0, 24.0),
                line("合計", 20.0, 200.0, 60.0, 24.0),
                line("コピー用紙", 20.0, 100.0, 120.0, 24.0),
                line("¥1,320", 300.0, 201.0, 70.0, 24.0),
            ],
        };
        assert_eq!(page.to_text(), "コピー用紙  ¥580\n合計  ¥1,320");
    }

    #[test]
    fn 縦書きは末尾に回す() {
        let mut v = line("山田商店", 380.0, 10.0, 30.0, 120.0);
        v.vertical = true;
        let page = OcrPage { width: 400.0, height: 300.0, lines: vec![v, line("合計 100", 10.0, 50.0, 80.0, 20.0)] };
        assert_eq!(page.to_text(), "合計 100\n山田商店");
        assert_eq!(page.lines[0].char_size(), 30.0);
    }

    #[test]
    fn 信頼度の平均と空のページ() {
        let page = OcrPage { width: 1.0, height: 1.0, lines: vec![line("a", 0.0, 0.0, 1.0, 1.0)] };
        assert!((page.mean_confidence() - 0.9).abs() < 1e-6);
        assert_eq!(OcrPage { width: 1.0, height: 1.0, lines: vec![] }.mean_confidence(), 0.0);
        assert_eq!(OcrPage { width: 1.0, height: 1.0, lines: vec![] }.to_text(), "");
    }

    #[test]
    fn 枠のすき間() {
        let a = BBox { x: 0.0, y: 0.0, w: 10.0, h: 10.0 };
        let b = BBox { x: 15.0, y: 0.0, w: 10.0, h: 10.0 };
        assert_eq!(a.gap(&b), 5.0);
        assert_eq!(a.gap(&a), 0.0);
    }

    #[test]
    fn 壊れた画像はエラー() {
        assert!(matches!(OcrImage::decode(b"not an image"), Err(CoreError::Ocr(_))));
    }

    #[test]
    fn 偽のocrは決めた結果を返す() {
        let page = OcrPage { width: 10.0, height: 10.0, lines: vec![line("x", 0.0, 0.0, 1.0, 1.0)] };
        let ocr = FakeOcr { page: page.clone() };
        let img = OcrImage::from_dynamic(image::DynamicImage::new_luma8(2, 2));
        assert_eq!(ocr.recognize(&img).unwrap(), page);
    }
}
