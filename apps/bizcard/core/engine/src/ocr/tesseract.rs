//! Tesseract の実行ファイルを呼ぶ実装。
//!
//! - 画像は PNG にして標準入力へ渡す(一時ファイルを作らない)。
//! - `OMP_THREAD_LIMIT=1` を必ず付ける。コア数ぴったりのスレッド数にすると、OpenMP の空回りで
//!   1枚が 0.25 秒から 25 秒に悪化することを実測した(`docs/spike-results.md`)。
//! - 実行ファイルと言語データ(jpn.traineddata)は、アプリに同梱するか model_manager で取得する。

use super::{BBox, Ocr, OcrImage, OcrLine, OcrPage};
use crate::error::CoreError;
use std::collections::BTreeMap;
use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub struct TesseractOcr {
    pub exe: PathBuf,
    pub tessdata_dir: Option<PathBuf>,
    pub lang: String,
    /// ページ分割モード。6 = 1つの文字ブロック(レシート向き)、4 = 可変サイズの列
    pub psm: u8,
}

impl TesseractOcr {
    pub fn new(exe: impl Into<PathBuf>) -> Self {
        Self { exe: exe.into(), tessdata_dir: None, lang: "jpn".into(), psm: 6 }
    }
}

impl Ocr for TesseractOcr {
    fn name(&self) -> &str {
        "tesseract"
    }

    fn recognize(&self, image: &OcrImage) -> Result<OcrPage, CoreError> {
        let mut png = Vec::new();
        image
            .img
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|e| CoreError::Ocr(e.to_string()))?;
        let mut cmd = Command::new(&self.exe);
        cmd.args(["stdin", "stdout", "-l", &self.lang, "--psm", &self.psm.to_string()]);
        if let Some(d) = &self.tessdata_dir {
            cmd.arg("--tessdata-dir").arg(d);
        }
        cmd.arg("tsv")
            .env("OMP_THREAD_LIMIT", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = cmd.spawn().map_err(|e| CoreError::Ocr(format!("Tesseract を起動できません: {e}")))?;
        let mut stdin = child.stdin.take().ok_or_else(|| CoreError::Ocr("標準入力を開けません".into()))?;
        // 書き込みを別スレッドで行い、出力の読み取りと詰まらないようにする
        let writer = std::thread::spawn(move || {
            let _ = stdin.write_all(&png);
        });
        let out = child.wait_with_output().map_err(|e| CoreError::Ocr(e.to_string()))?;
        let _ = writer.join();
        if !out.status.success() {
            return Err(CoreError::Ocr("Tesseract が失敗しました".into()));
        }
        let tsv = String::from_utf8_lossy(&out.stdout);
        Ok(parse_tsv(&tsv, image.width() as f32, image.height() as f32))
    }
}

fn ascii_alnum(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

/// 単語をつなぐ。日本語どうしの間には空白を入れない(Tesseract は字の間に空白を出すことがある)。
fn join_words(words: &[&str]) -> String {
    let mut s = String::new();
    for w in words {
        if let (Some(a), Some(b)) = (s.chars().last(), w.chars().next()) {
            if ascii_alnum(a) && ascii_alnum(b) {
                s.push(' ');
            }
        }
        s.push_str(w);
    }
    s
}

/// `tesseract ... tsv` の出力から、行ごとの結果を作る。
pub fn parse_tsv(tsv: &str, width: f32, height: f32) -> OcrPage {
    struct Acc<'a> {
        words: Vec<&'a str>,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        conf: f32,
        n: f32,
    }
    let mut lines: BTreeMap<(u32, u32, u32), Acc> = BTreeMap::new();
    for row in tsv.lines().skip(1) {
        let c: Vec<&str> = row.split('\t').collect();
        if c.len() < 12 || c[0] != "5" {
            continue;
        }
        let text = c[11].trim();
        let conf: f32 = c[10].parse().unwrap_or(-1.0);
        if text.is_empty() || conf < 0.0 {
            continue;
        }
        let p = |i: usize| c[i].parse::<f32>().unwrap_or(0.0);
        let key = (c[2].parse().unwrap_or(0), c[3].parse().unwrap_or(0), c[4].parse().unwrap_or(0));
        let (l, t, w, h) = (p(6), p(7), p(8), p(9));
        let a = lines.entry(key).or_insert(Acc { words: vec![], x0: f32::MAX, y0: f32::MAX, x1: 0.0, y1: 0.0, conf: 0.0, n: 0.0 });
        a.words.push(text);
        a.x0 = a.x0.min(l);
        a.y0 = a.y0.min(t);
        a.x1 = a.x1.max(l + w);
        a.y1 = a.y1.max(t + h);
        a.conf += conf / 100.0;
        a.n += 1.0;
    }
    let lines = lines
        .into_values()
        .map(|a| {
            let (w, h) = (a.x1 - a.x0, a.y1 - a.y0);
            OcrLine {
                text: join_words(&a.words),
                bbox: BBox { x: a.x0, y: a.y0, w, h },
                vertical: h > 2.0 * w && a.words.len() == 1 && h > 40.0,
                confidence: a.conf / a.n,
                alt_text: None,
            }
        })
        .collect();
    OcrPage { width, height, lines }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TSV: &str = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
1\t1\t0\t0\t0\t0\t0\t0\t480\t300\t-1\t\n\
4\t1\t1\t1\t1\t0\t20\t30\t300\t26\t-1\t\n\
5\t1\t1\t1\t1\t1\t20\t30\t100\t26\t96.5\t株式会社\n\
5\t1\t1\t1\t1\t2\t125\t30\t120\t26\t90.0\tさくら書店\n\
5\t1\t1\t1\t2\t1\t20\t80\t60\t26\t80.0\tT123\n\
5\t1\t1\t1\t2\t2\t90\t80\t90\t26\t70.0\t4567\n\
5\t1\t1\t1\t3\t1\t20\t120\t50\t26\t-1\t \n";

    #[test]
    fn tsvから行と位置と信頼度を作る() {
        let p = parse_tsv(TSV, 480.0, 300.0);
        assert_eq!(p.lines.len(), 2);
        assert_eq!(p.lines[0].text, "株式会社さくら書店"); // 日本語どうしに空白を入れない
        assert_eq!(p.lines[0].bbox, BBox { x: 20.0, y: 30.0, w: 225.0, h: 26.0 });
        assert!((p.lines[0].confidence - 0.9325).abs() < 1e-4);
        assert_eq!(p.lines[1].text, "T123 4567"); // 英数字どうしは空白を残す
        assert!(!p.lines[1].vertical);
        assert_eq!(p.to_text(), "株式会社さくら書店\nT123 4567");
    }

    #[test]
    fn 実行ファイルが無ければエラー() {
        let ocr = TesseractOcr::new("/nonexistent/tesseract");
        let img = OcrImage::from_dynamic(image::DynamicImage::new_luma8(8, 8));
        assert!(matches!(ocr.recognize(&img), Err(CoreError::Ocr(_))));
    }

    /// Tesseract が入っている環境でだけ動く(無ければ何もしない)。
    #[test]
    fn 実機のtesseractで空白の画像でも落ちない() {
        let Ok(exe) = which("tesseract") else { return };
        let ocr = TesseractOcr::new(exe);
        let img = OcrImage::from_dynamic(image::DynamicImage::new_luma8(64, 64));
        if let Ok(p) = ocr.recognize(&img) {
            assert!(p.lines.is_empty());
        }
    }

    fn which(name: &str) -> Result<PathBuf, ()> {
        let path = std::env::var_os("PATH").ok_or(())?;
        std::env::split_paths(&path).map(|d| d.join(name)).find(|p| p.is_file()).ok_or(())
    }
}
