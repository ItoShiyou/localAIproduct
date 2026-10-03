//! OCRの出力(行ごとのテキストと位置)を表す型。
//!
//! `core` の `ocr` トレイトは未実装なので、ここでは「行+位置情報」という最小の形だけを
//! 仮に決めている。`core::ocr` の型が確定したら、変換関数を1つ書いて差し替える(要確認)。
//! 座標は画像の左上を原点とするピクセル。縦書きの行は `vertical = true` で、
//! `bbox.w` が文字の大きさ、`bbox.h` が行の長さになる。

use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct OcrLine {
    pub text: String,
    pub bbox: BBox,
    #[serde(default)]
    pub vertical: bool,
    /// OCRエンジンが返す信頼度(0〜1)。無ければ 1.0 とみなす
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

#[derive(Debug, Clone, Deserialize)]
pub struct OcrCard {
    #[serde(default)]
    pub id: String,
    /// "front" | "back"
    #[serde(default)]
    pub side: String,
    pub width: f32,
    pub height: f32,
    pub lines: Vec<OcrLine>,
}
