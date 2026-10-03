//! ONNX Runtime 上の PP-OCR(文字領域の検出 DB + 文字列の認識 CRNN/CTC)。
//!
//! - `ort` は `load-dynamic`: 実行時に onnxruntime の共有ライブラリを読み込む(アプリに同梱する)。
//! - 画像処理は `image` クレートだけで書いた(OpenCV を使わない)。検出結果の領域は「軸に平行な長方形」
//!   で近似する。傾いた紙では領域が少し大きくなるが、行の読み取りには足りることを合成画像で確認した
//!   (`docs/spike-results.md`)。実際の撮影画像は未検証。
//! - 認識モデルの辞書は、モデルの metadata `character` から読む(無ければ辞書ファイルを指定する)。
//! - 日本語専用の認識モデルに差し替えるときは、`rec_model` と `dict` のパスを変えるだけでよい。

use super::{BBox, Ocr, OcrImage, OcrLine, OcrPage};
use crate::error::CoreError;
use image::{imageops, imageops::FilterType, RgbImage};
use ndarray::Array4;
use ort::{session::Session, value::Tensor};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct PpOcrConfig {
    /// onnxruntime の共有ライブラリ(libonnxruntime.so / .dylib / onnxruntime.dll)
    pub onnxruntime_lib: PathBuf,
    pub det_model: PathBuf,
    pub rec_model: PathBuf,
    /// 1文字1行の辞書。None なら認識モデルの metadata `character` を使う
    pub dict: Option<PathBuf>,
    /// 数字・記号に強い認識モデルを併用する場合の2つ目(辞書は metadata から)。
    /// 日本語モデルは「T」を「1」、「¥」を「半」と読み違えることがあり、中国語モデルは仮名を落とす。
    /// 両方で読み、2つ目の結果は `OcrLine::alt_text` に入れる(使い分けは呼び出し側)。
    pub alt_rec_model: Option<PathBuf>,
    /// 推論のスレッド数(最低ラインの機械では 6)
    pub threads: usize,
}

struct Recognizer {
    sess: Mutex<Session>,
    /// 0 = CTC の空白、最後 = 空白文字
    chars: Vec<String>,
}

pub struct PpOcr {
    det: Mutex<Session>,
    rec: Recognizer,
    alt: Option<Recognizer>,
}

fn ort_err<E: std::fmt::Display>(e: E) -> CoreError {
    CoreError::Ocr(e.to_string())
}

impl PpOcr {
    pub fn new(cfg: &PpOcrConfig) -> Result<Self, CoreError> {
        static INIT: std::sync::Once = std::sync::Once::new();
        let mut init_err: Option<String> = None;
        INIT.call_once(|| match ort::init_from(&cfg.onnxruntime_lib) {
            Ok(b) => {
                b.commit();
            }
            Err(e) => init_err = Some(e.to_string()),
        });
        if let Some(e) = init_err {
            return Err(CoreError::Ocr(format!("onnxruntime を読み込めません: {e}")));
        }
        let load = |p: &PathBuf| -> Result<Session, CoreError> {
            Session::builder()
                .map_err(ort_err)?
                .with_intra_threads(cfg.threads.max(1))
                .map_err(ort_err)?
                .commit_from_file(p)
                .map_err(ort_err)
        };
        let det = load(&cfg.det_model)?;
        let rec = recognizer(load(&cfg.rec_model)?, cfg.dict.as_ref())?;
        let alt = match &cfg.alt_rec_model {
            Some(p) => Some(recognizer(load(p)?, None)?),
            None => None,
        };
        Ok(Self { det: Mutex::new(det), rec, alt })
    }

    /// 検出: 文字がありそうな領域(元画像の座標)を返す
    fn detect(&self, rgb: &RgbImage) -> Result<Vec<DetBox>, CoreError> {
        let (w, h) = rgb.dimensions();
        let (rw, rh, sx, sy) = det_size(w, h);
        let resized = imageops::resize(rgb, rw, rh, FilterType::Triangle);
        let mut input = Array4::<f32>::zeros((1, 3, rh as usize, rw as usize));
        for (x, y, p) in resized.enumerate_pixels() {
            // モデルは BGR 順、(v/255 - 0.5) / 0.5
            for (c, src) in [2usize, 1, 0].iter().enumerate() {
                input[[0, c, y as usize, x as usize]] = (p[*src] as f32 / 255.0 - 0.5) / 0.5;
            }
        }
        let mut sess = self.det.lock().map_err(|_| CoreError::Ocr("検出モデルが使えません".into()))?;
        let outputs = sess
            .run(ort::inputs![Tensor::from_array(input).map_err(ort_err)?])
            .map_err(ort_err)?;
        let prob = outputs[0].try_extract_array::<f32>().map_err(ort_err)?;
        let shape = prob.shape().to_vec();
        let (ph, pw) = (shape[shape.len() - 2], shape[shape.len() - 1]);
        let flat: Vec<f32> = prob.iter().copied().collect();
        Ok(boxes_from_prob(&flat, pw, ph, 0.3, 0.5, 1.6, sx, sy)
            .into_iter()
            .filter(|b| b.len >= 4.0 && b.thick >= 4.0)
            .collect())
    }

}

fn recognizer(sess: Session, dict: Option<&PathBuf>) -> Result<Recognizer, CoreError> {
    let dict_text = match dict {
        Some(p) => std::fs::read_to_string(p).map_err(ort_err)?,
        None => sess
            .metadata()
            .map_err(ort_err)?
            .custom("character")
            .ok_or_else(|| CoreError::Ocr("認識モデルに辞書(character)がありません。辞書ファイルを指定してください".into()))?,
    };
    let mut chars = vec![String::new()];
    chars.extend(dict_text.lines().map(|s| s.to_string()));
    chars.push(" ".to_string());
    Ok(Recognizer { sess: Mutex::new(sess), chars })
}

impl Recognizer {
    /// 認識: 切り出した1行の画像から (文字列, 信頼度)
    fn run(&self, crop: &RgbImage) -> Result<(String, f32), CoreError> {
        let (cw, ch) = crop.dimensions();
        let new_w = ((48.0 * cw as f32 / ch as f32).ceil() as u32).clamp(8, 2048);
        let pad_w = new_w.max(320);
        let resized = imageops::resize(crop, new_w, 48, FilterType::Triangle);
        let mut input = Array4::<f32>::zeros((1, 3, 48, pad_w as usize));
        for (x, y, p) in resized.enumerate_pixels() {
            for (c, src) in [2usize, 1, 0].iter().enumerate() {
                input[[0, c, y as usize, x as usize]] = (p[*src] as f32 / 255.0 - 0.5) / 0.5;
            }
        }
        let mut sess = self.sess.lock().map_err(|_| CoreError::Ocr("認識モデルが使えません".into()))?;
        let outputs = sess
            .run(ort::inputs![Tensor::from_array(input).map_err(ort_err)?])
            .map_err(ort_err)?;
        let out = outputs[0].try_extract_array::<f32>().map_err(ort_err)?;
        let shape = out.shape().to_vec();
        let (t, k) = (shape[shape.len() - 2], shape[shape.len() - 1]);
        let flat: Vec<f32> = out.iter().copied().collect();
        Ok(ctc_decode(&flat, t, k, &self.chars))
    }
}

impl Ocr for PpOcr {
    fn name(&self) -> &str {
        "ppocr-onnx"
    }

    fn recognize(&self, image: &OcrImage) -> Result<OcrPage, CoreError> {
        let rgb = image.img.to_rgb8();
        let (w, h) = rgb.dimensions();
        let mut lines = Vec::new();
        for b in self.detect(&rgb)? {
            let crop = rotated_crop(&rgb, &b);
            let (text, conf) = self.rec.run(&crop)?;
            let alt_text = match &self.alt {
                Some(a) => Some(a.run(&crop)?.0).filter(|t| !t.trim().is_empty()),
                None => None,
            };
            if text.trim().is_empty() && alt_text.is_none() {
                continue;
            }
            lines.push(OcrLine { text, bbox: b.aabb(w as f32, h as f32), vertical: b.vertical(), confidence: conf, alt_text });
        }
        lines.sort_by(|a, b| a.bbox.y.total_cmp(&b.bbox.y).then(a.bbox.x.total_cmp(&b.bbox.x)));
        Ok(OcrPage { width: w as f32, height: h as f32, lines })
    }
}

/// 検出モデルの入力サイズ。短辺が 736 未満なら 736 まで拡大し、長辺は 2000 まで。32の倍数にする。
/// 戻り値は (幅, 高さ, 横倍率, 縦倍率)。
pub fn det_size(w: u32, h: u32) -> (u32, u32, f32, f32) {
    let (wf, hf) = (w as f32, h as f32);
    let mut ratio = if wf.min(hf) < 736.0 { 736.0 / wf.min(hf) } else { 1.0 };
    if wf.max(hf) * ratio > 2000.0 {
        ratio = 2000.0 / wf.max(hf);
    }
    let r32 = |v: f32| (((v * ratio / 32.0).round() as u32).max(1)) * 32;
    let (rw, rh) = (r32(wf), r32(hf));
    (rw, rh, rw as f32 / wf, rh as f32 / hf)
}

/// 検出した1行の領域。向きのついた長方形(元画像の座標)。
/// `len` は読む方向の長さ、`thick` は文字の高さ(縦書きなら幅)。(ux, uy) は読む方向の単位ベクトル。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DetBox {
    pub cx: f32,
    pub cy: f32,
    pub len: f32,
    pub thick: f32,
    pub ux: f32,
    pub uy: f32,
}

impl DetBox {
    /// 縦書き: 読む方向が縦に近く、細長い
    pub fn vertical(&self) -> bool {
        self.uy.abs() > self.ux.abs() && self.len >= 2.0 * self.thick
    }
    /// 回転した長方形を囲む、軸に平行な長方形
    pub fn aabb(&self, max_w: f32, max_h: f32) -> BBox {
        let (hl, ht) = (self.len / 2.0, self.thick / 2.0);
        let ex = hl * self.ux.abs() + ht * self.uy.abs();
        let ey = hl * self.uy.abs() + ht * self.ux.abs();
        let (x0, y0) = ((self.cx - ex).max(0.0), (self.cy - ey).max(0.0));
        let (x1, y1) = ((self.cx + ex).min(max_w), (self.cy + ey).min(max_h));
        BBox { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
    }
}

/// DB の確率マップから文字領域を作る。しきい値で2値化 → 2x2膨張 → 連結成分 → 平均スコアで選別 →
/// 主成分で向きを求め、周長と面積から外側へ広げる(unclip)。`sx`, `sy` は元画像から確率マップへの倍率で、
/// 戻り値は元画像の座標。
#[allow(clippy::too_many_arguments)]
pub fn boxes_from_prob(prob: &[f32], w: usize, h: usize, thresh: f32, box_thresh: f32, unclip: f32, sx: f32, sy: f32) -> Vec<DetBox> {
    let bin: Vec<bool> = prob.iter().map(|p| *p > thresh).collect();
    let mut dil = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            if bin[y * w + x] {
                for dy in 0..2 {
                    for dx in 0..2 {
                        if y + dy < h && x + dx < w {
                            dil[(y + dy) * w + x + dx] = true;
                        }
                    }
                }
            }
        }
    }
    let mut seen = vec![false; w * h];
    let mut out = Vec::new();
    for sy0 in 0..h {
        for sx0 in 0..w {
            let i = sy0 * w + sx0;
            if !dil[i] || seen[i] {
                continue;
            }
            let mut pts: Vec<(f32, f32)> = Vec::new(); // 元画像の座標
            let mut sum = 0.0f32;
            let mut q = VecDeque::from([(sx0, sy0)]);
            seen[i] = true;
            while let Some((x, y)) = q.pop_front() {
                pts.push(((x as f32 + 0.5) / sx, (y as f32 + 0.5) / sy));
                sum += prob[y * w + x];
                for ny in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                    for nx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                        let j = ny * w + nx;
                        if dil[j] && !seen[j] {
                            seen[j] = true;
                            q.push_back((nx, ny));
                        }
                    }
                }
            }
            if pts.len() < 9 || sum / (pts.len() as f32) < box_thresh {
                continue;
            }
            // 主成分(共分散行列の最大固有ベクトル)= 読む方向
            let n = pts.len() as f32;
            let (mx, my) = (pts.iter().map(|p| p.0).sum::<f32>() / n, pts.iter().map(|p| p.1).sum::<f32>() / n);
            let (mut sxx, mut syy, mut sxy) = (0.0f32, 0.0f32, 0.0f32);
            for p in &pts {
                let (dx, dy) = (p.0 - mx, p.1 - my);
                sxx += dx * dx;
                syy += dy * dy;
                sxy += dx * dy;
            }
            let theta = 0.5 * (2.0 * sxy).atan2(sxx - syy);
            let (mut ux, mut uy) = (theta.cos(), theta.sin());
            if ux < -1e-6 || (ux.abs() < 1e-6 && uy < 0.0) {
                ux = -ux;
                uy = -uy;
            }
            let (vx, vy) = (-uy, ux);
            let (mut u0, mut u1, mut v0, mut v1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
            for p in &pts {
                let (dx, dy) = (p.0 - mx, p.1 - my);
                let (a, b) = (dx * ux + dy * uy, dx * vx + dy * vy);
                u0 = u0.min(a);
                u1 = u1.max(a);
                v0 = v0.min(b);
                v1 = v1.max(b);
            }
            let (len, thick) = (u1 - u0, v1 - v0);
            if len.min(thick) < 3.0 / sx.max(sy) {
                continue;
            }
            let d = len * thick * unclip / (2.0 * (len + thick));
            let (cu, cv) = ((u0 + u1) / 2.0, (v0 + v1) / 2.0);
            out.push(DetBox {
                cx: mx + cu * ux + cv * vx,
                cy: my + cu * uy + cv * vy,
                len: len + 2.0 * d,
                thick: thick + 2.0 * d,
                ux,
                uy,
            });
        }
    }
    out
}

/// 回転した領域を、読む方向が左から右になるように切り出す(双一次補間)。
pub fn rotated_crop(img: &RgbImage, b: &DetBox) -> RgbImage {
    let (ow, oh) = ((b.len.ceil() as u32).max(1), (b.thick.ceil() as u32).max(1));
    let (vx, vy) = (-b.uy, b.ux);
    let (w, h) = img.dimensions();
    let mut out = RgbImage::new(ow, oh);
    for j in 0..oh {
        for i in 0..ow {
            let (du, dv) = (i as f32 + 0.5 - ow as f32 / 2.0, j as f32 + 0.5 - oh as f32 / 2.0);
            let x = (b.cx + du * b.ux + dv * vx - 0.5).clamp(0.0, (w - 1) as f32);
            let y = (b.cy + du * b.uy + dv * vy - 0.5).clamp(0.0, (h - 1) as f32);
            let (x0, y0) = (x.floor() as u32, y.floor() as u32);
            let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
            let (fx, fy) = (x - x0 as f32, y - y0 as f32);
            let mut px = [0u8; 3];
            for (c, slot) in px.iter_mut().enumerate() {
                let v = img.get_pixel(x0, y0)[c] as f32 * (1.0 - fx) * (1.0 - fy)
                    + img.get_pixel(x1, y0)[c] as f32 * fx * (1.0 - fy)
                    + img.get_pixel(x0, y1)[c] as f32 * (1.0 - fx) * fy
                    + img.get_pixel(x1, y1)[c] as f32 * fx * fy;
                *slot = v.round() as u8;
            }
            out.put_pixel(i, j, image::Rgb(px));
        }
    }
    out
}

/// CTC の貪欲デコード。`probs` は (時刻 t, 文字種 k) の行優先。空白(0)と連続する同じ字を落とす。
pub fn ctc_decode(probs: &[f32], t: usize, k: usize, chars: &[String]) -> (String, f32) {
    let mut text = String::new();
    let (mut conf_sum, mut n) = (0.0f32, 0usize);
    let mut prev = 0usize;
    for ti in 0..t {
        let row = &probs[ti * k..(ti + 1) * k];
        let (idx, p) = row.iter().copied().enumerate().fold((0, f32::MIN), |a, (i, v)| if v > a.1 { (i, v) } else { a });
        if idx != 0 && idx != prev {
            if let Some(c) = chars.get(idx) {
                text.push_str(c);
                conf_sum += p;
                n += 1;
            }
        }
        prev = idx;
    }
    (text, if n == 0 { 0.0 } else { conf_sum / n as f32 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 検出の入力サイズは32の倍数で_短辺736まで拡大() {
        let (w, h, sx, sy) = det_size(480, 420);
        assert_eq!((w % 32, h % 32), (0, 0));
        assert!(h >= 736 && sx > 1.0 && sy > 1.0);
        let (w2, h2, _, _) = det_size(4000, 3000);
        assert!(w2.max(h2) <= 2016);
    }

    #[test]
    fn 確率マップから横長の領域を取り出す() {
        let (w, h) = (40, 20);
        let mut p = vec![0.0f32; w * h];
        for y in 8..12 {
            for x in 5..35 {
                p[y * w + x] = 0.9;
            }
        }
        // ノイズ(小さい点)は捨てる
        p[2 * w + 2] = 0.9;
        let b = boxes_from_prob(&p, w, h, 0.3, 0.5, 1.6, 1.0, 1.0);
        assert_eq!(b.len(), 1);
        assert!(b[0].ux > 0.99 && b[0].len >= 30.0 && !b[0].vertical());
        let bb = b[0].aabb(40.0, 20.0);
        assert!(bb.x <= 5.0 && bb.x + bb.w >= 35.0 && bb.y <= 8.0 && bb.y + bb.h >= 12.0);
        // 低スコアの領域は捨てる
        let low: Vec<f32> = p.iter().map(|v| if *v > 0.0 { 0.4 } else { 0.0 }).collect();
        assert!(boxes_from_prob(&low, w, h, 0.3, 0.5, 1.6, 1.0, 1.0).is_empty());
    }

    #[test]
    fn 傾いた行は向きを求めて水平に切り出す() {
        // 約10度傾いた帯
        let (w, h) = (120, 80);
        let mut p = vec![0.0f32; w * h];
        let slope = 10f32.to_radians().tan();
        for x in 10..110 {
            let yc = 20.0 + (x as f32 - 10.0) * slope;
            for dy in -3..=3 {
                p[((yc as i32 + dy) as usize) * w + x] = 0.9;
            }
        }
        let b = boxes_from_prob(&p, w, h, 0.3, 0.5, 1.6, 1.0, 1.0);
        assert_eq!(b.len(), 1);
        let angle = b[0].uy.atan2(b[0].ux).to_degrees();
        assert!((angle - 10.0).abs() < 2.0, "angle={angle}");
        let img = RgbImage::from_pixel(w as u32, h as u32, image::Rgb([255, 255, 255]));
        let crop = rotated_crop(&img, &b[0]);
        assert!(crop.width() > crop.height() * 3);
    }

    #[test]
    fn 縦長の領域は縦書きとして読む方向が下向き() {
        let (w, h) = (40, 80);
        let mut p = vec![0.0f32; w * h];
        for y in 10..70 {
            for x in 18..23 {
                p[y * w + x] = 0.9;
            }
        }
        let b = boxes_from_prob(&p, w, h, 0.3, 0.5, 1.6, 1.0, 1.0);
        assert_eq!(b.len(), 1);
        assert!(b[0].vertical() && b[0].uy > 0.99);
    }

    #[test]
    fn ctcは空白と連続する字を落とす() {
        let chars: Vec<String> = ["", "あ", "い", " "].iter().map(|s| s.to_string()).collect();
        // 時刻ごとの最大: あ あ 空白 あ い い → 「ああい」(空白で区切られた同じ字は2つ)
        let rows = [1usize, 1, 0, 1, 2, 2];
        let mut probs = vec![0.0f32; rows.len() * 4];
        for (t, i) in rows.iter().enumerate() {
            probs[t * 4 + i] = 0.8;
        }
        let (text, conf) = ctc_decode(&probs, rows.len(), 4, &chars);
        assert_eq!(text, "ああい");
        assert!((conf - 0.8).abs() < 1e-6);
        assert_eq!(ctc_decode(&[1.0, 0.0, 0.0, 0.0], 1, 4, &chars), (String::new(), 0.0));
    }

    /// 環境変数 FACTORY_OCR_ORT_LIB / FACTORY_OCR_DET / FACTORY_OCR_REC があるときだけ動く実機テスト。
    #[test]
    fn 実モデルで合成画像の数字を読む() {
        let (Ok(lib), Ok(det), Ok(rec)) = (
            std::env::var("FACTORY_OCR_ORT_LIB"),
            std::env::var("FACTORY_OCR_DET"),
            std::env::var("FACTORY_OCR_REC"),
        ) else {
            return;
        };
        let ocr = PpOcr::new(&PpOcrConfig {
            onnxruntime_lib: lib.into(),
            det_model: det.into(),
            rec_model: rec.into(),
            dict: None,
            alt_rec_model: None,
            threads: 4,
        })
        .unwrap();
        let Ok(path) = std::env::var("FACTORY_OCR_SAMPLE") else { return };
        let img = OcrImage::decode(&std::fs::read(path).unwrap()).unwrap();
        let page = ocr.recognize(&img).unwrap();
        println!("{}", page.to_text());
        assert!(!page.lines.is_empty());
    }
}
