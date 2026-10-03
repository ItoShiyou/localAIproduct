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
    /// 推論のスレッド数(最低ラインの機械では 6)
    pub threads: usize,
}

pub struct PpOcr {
    det: Mutex<Session>,
    rec: Mutex<Session>,
    /// 0 = CTC の空白、最後 = 空白文字
    chars: Vec<String>,
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
        let rec = load(&cfg.rec_model)?;
        let dict_text = match &cfg.dict {
            Some(p) => std::fs::read_to_string(p).map_err(ort_err)?,
            None => rec
                .metadata()
                .map_err(ort_err)?
                .custom("character")
                .ok_or_else(|| CoreError::Ocr("認識モデルに辞書(character)がありません。辞書ファイルを指定してください".into()))?,
        };
        let mut chars = vec![String::new()];
        chars.extend(dict_text.lines().map(|s| s.to_string()));
        chars.push(" ".to_string());
        Ok(Self { det: Mutex::new(det), rec: Mutex::new(rec), chars })
    }

    /// 検出: 文字がありそうな領域(元画像の座標)を返す
    fn detect(&self, rgb: &RgbImage) -> Result<Vec<BBox>, CoreError> {
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
        let boxes = boxes_from_prob(&flat, pw, ph, 0.3, 0.5, 1.6);
        Ok(boxes
            .into_iter()
            .filter_map(|b| {
                let x0 = (b.x / sx).max(0.0);
                let y0 = (b.y / sy).max(0.0);
                let x1 = ((b.x + b.w) / sx).min(w as f32);
                let y1 = ((b.y + b.h) / sy).min(h as f32);
                (x1 - x0 >= 4.0 && y1 - y0 >= 4.0).then_some(BBox { x: x0, y: y0, w: x1 - x0, h: y1 - y0 })
            })
            .collect())
    }

    /// 認識: 切り出した1行の画像から (文字列, 信頼度)
    fn recognize_crop(&self, crop: &RgbImage) -> Result<(String, f32), CoreError> {
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
        let mut sess = self.rec.lock().map_err(|_| CoreError::Ocr("認識モデルが使えません".into()))?;
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
            let crop = imageops::crop_imm(&rgb, b.x as u32, b.y as u32, (b.w as u32).max(1), (b.h as u32).max(1)).to_image();
            // 縦長の領域は縦書きとみなし、横にして読む(反時計回りに90度)
            let vertical = b.h >= 2.0 * b.w;
            let crop = if vertical { imageops::rotate270(&crop) } else { crop };
            let (text, conf) = self.recognize_crop(&crop)?;
            if text.trim().is_empty() {
                continue;
            }
            lines.push(OcrLine { text, bbox: b, vertical, confidence: conf });
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

/// DB の確率マップから文字領域を作る。しきい値で2値化 → 2x2膨張 → 連結成分 → 平均スコアで選別 →
/// 周長と面積から外側へ広げる(unclip)。戻り値は確率マップ上の座標。
pub fn boxes_from_prob(prob: &[f32], w: usize, h: usize, thresh: f32, box_thresh: f32, unclip: f32) -> Vec<BBox> {
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
    for sy in 0..h {
        for sx in 0..w {
            let i = sy * w + sx;
            if !dil[i] || seen[i] {
                continue;
            }
            let (mut x0, mut y0, mut x1, mut y1) = (sx, sy, sx, sy);
            let (mut sum, mut n) = (0.0f32, 0usize);
            let mut q = VecDeque::from([(sx, sy)]);
            seen[i] = true;
            while let Some((x, y)) = q.pop_front() {
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
                sum += prob[y * w + x];
                n += 1;
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
            let (bw, bh) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
            if bw.min(bh) < 3.0 || (sum / n as f32) < box_thresh {
                continue;
            }
            let d = bw * bh * unclip / (2.0 * (bw + bh));
            let (ex0, ey0) = ((x0 as f32 - d).max(0.0), (y0 as f32 - d).max(0.0));
            let (ex1, ey1) = ((x1 as f32 + 1.0 + d).min(w as f32), (y1 as f32 + 1.0 + d).min(h as f32));
            out.push(BBox { x: ex0, y: ey0, w: ex1 - ex0, h: ey1 - ey0 });
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
        let b = boxes_from_prob(&p, w, h, 0.3, 0.5, 1.6);
        assert_eq!(b.len(), 1);
        assert!(b[0].x <= 5.0 && b[0].x + b[0].w >= 35.0 && b[0].y <= 8.0 && b[0].y + b[0].h >= 12.0);
        // 低スコアの領域は捨てる
        let low: Vec<f32> = p.iter().map(|v| if *v > 0.0 { 0.4 } else { 0.0 }).collect();
        assert!(boxes_from_prob(&low, w, h, 0.3, 0.5, 1.6).is_empty());
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
