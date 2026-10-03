//! 取り込み(PDF・JPEG/PNG・フォルダ)と、読み取りジョブの実行。
//!
//! - 元のファイルは変更しない。アプリのデータフォルダ(`originals/`)にコピーして保持する。
//! - 同一ファイル(SHA-256)は取り込まない。壊れたファイル・空・大きすぎるファイルは、理由を付けて飛ばし、続行する。
//! - 読み取りは `core::jobs` で1書類1ジョブ。中断しても `recover` で再開できる。

use crate::extract::{draft_from_text, read_document};
use crate::store::Store;
use factory_core::jobs::{CancelToken, Jobs, Summary};
use factory_core::ocr::Ocr;
use factory_core::CoreError;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub const MAX_BYTES: u64 = 100 * 1024 * 1024;
pub const JOB_KIND: &str = "read_document";

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Imported { document_id: i64 },
    Duplicate,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportResult {
    pub path: PathBuf,
    pub outcome: Outcome,
}

fn kind_of(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(b"%PDF") {
        Some(("pdf", "pdf"))
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(("jpeg", "jpg"))
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some(("png", "png"))
    } else {
        None
    }
}

/// 取り込み対象のファイルを集める(フォルダは再帰。隠しファイルは除く。拡張子 pdf/jpg/jpeg/png)。
pub fn collect_inputs(inputs: &[PathBuf]) -> Vec<PathBuf> {
    fn walk(p: &Path, out: &mut Vec<PathBuf>, depth: usize) {
        if p.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(false) {
            return;
        }
        if p.is_dir() {
            if depth > 8 {
                return;
            }
            if let Ok(rd) = fs::read_dir(p) {
                let mut v: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
                v.sort();
                for c in v {
                    walk(&c, out, depth + 1);
                }
            }
        } else {
            out.push(p.to_path_buf());
        }
    }
    let mut all = Vec::new();
    for i in inputs {
        walk(i, &mut all, 0);
    }
    all.into_iter()
        .filter(|p| {
            // フォルダ内のファイルは対応拡張子だけ。直接指定されたファイルは中身で判定して理由を出す
            let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            inputs.contains(p) || ["pdf", "jpg", "jpeg", "png"].contains(&ext.as_str())
        })
        .collect()
}

pub fn import_file(store: &Store, data_dir: &Path, path: &Path) -> Outcome {
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return Outcome::Failed("ファイルを開けません".into()),
    };
    if meta.len() == 0 {
        return Outcome::Failed("空のファイルです".into());
    }
    if meta.len() > MAX_BYTES {
        return Outcome::Failed("ファイルが大きすぎます(100MBまで)".into());
    }
    let Ok(bytes) = fs::read(path) else { return Outcome::Failed("ファイルを読めません".into()) };
    let Some((kind, ext)) = kind_of(&bytes) else {
        return Outcome::Failed("PDF・JPEG・PNG のいずれでもありません(または壊れています)".into());
    };
    let sha = format!("{:x}", Sha256::digest(&bytes));
    match store.has_document(&sha) {
        Ok(true) => return Outcome::Duplicate,
        Ok(false) => {}
        Err(e) => return Outcome::Failed(e.to_string()),
    }
    let dir = data_dir.join("originals");
    if fs::create_dir_all(&dir).is_err() {
        return Outcome::Failed("保存先を作れません".into());
    }
    let stored = dir.join(format!("{sha}.{ext}"));
    if fs::write(&stored, &bytes).is_err() {
        return Outcome::Failed("原本のコピーを保存できません(空き容量を確認してください)".into());
    }
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    match store.add_document(&sha, &name, &stored.to_string_lossy(), kind) {
        Ok(Some(id)) => {
            match Jobs::new(&store.db).enqueue(JOB_KIND, &format!("{{\"document_id\":{id}}}")) {
                Ok(_) => Outcome::Imported { document_id: id },
                Err(e) => Outcome::Failed(e.to_string()),
            }
        }
        Ok(None) => {
            let _ = fs::remove_file(&stored);
            Outcome::Duplicate
        }
        Err(e) => {
            let _ = fs::remove_file(&stored);
            Outcome::Failed(e.to_string())
        }
    }
}

/// 複数の入力(ファイル・フォルダ)を取り込む。1件の失敗で止めない。
pub fn import_all(store: &Store, data_dir: &Path, inputs: &[PathBuf]) -> Vec<ImportResult> {
    collect_inputs(inputs).into_iter().map(|p| ImportResult { outcome: import_file(store, data_dir, &p), path: p }).collect()
}

/// 待ちの読み取りジョブを実行し、結果を「下書き」で保存する。
pub fn run_read_jobs(store: &Store, ocr: &dyn Ocr, cancel: &CancelToken, on_progress: impl FnMut(&Summary)) -> Result<usize, CoreError> {
    Jobs::new(&store.db).run_pending(
        cancel,
        |job| {
            let v: serde_json::Value = serde_json::from_str(&job.payload_json).map_err(|e| e.to_string())?;
            let id = v["document_id"].as_i64().ok_or("ジョブの内容が不正です")?;
            let (path, kind): (String, String) = store
                .db
                .conn
                .query_row("SELECT stored_path, kind FROM documents WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(|_| "書類が見つかりません".to_string())?;
            let bytes = fs::read(&path).map_err(|_| "原本のコピーを読めません".to_string())?;
            let pages = read_document(&bytes, kind == "pdf", ocr)?;
            for p in pages {
                let (r, conf) = draft_from_text(&p);
                store.add_draft(id, p.page, &r, &conf).map_err(|e| e.to_string())?;
            }
            Ok(())
        },
        on_progress,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Search;
    use factory_core::ocr::{BBox, FakeOcr, OcrLine, OcrPage};

    fn tmp(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rdb-imp-{n}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn png_bytes(seed: u8) -> Vec<u8> {
        let mut b = Vec::new();
        let img = image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(8, 8, image::Luma([seed])));
        img.write_to(&mut std::io::Cursor::new(&mut b), image::ImageFormat::Png).unwrap();
        b
    }

    fn fake() -> FakeOcr {
        let l = |t: &str, y: f32| OcrLine { text: t.into(), bbox: BBox { x: 0.0, y, w: 100.0, h: 20.0 }, vertical: false, confidence: 0.95, alt_text: None };
        FakeOcr { page: OcrPage { width: 100.0, height: 100.0, lines: vec![l("有限会社みどり商店", 0.0), l("2026/04/01", 25.0), l("合計 ¥880", 50.0)] } }
    }

    #[test]
    fn フォルダを取り込み_重複と壊れたファイルは飛ばして続行する() {
        let src = tmp("src");
        let data = tmp("data");
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("a.png"), png_bytes(10)).unwrap();
        fs::write(src.join("sub/b.png"), png_bytes(20)).unwrap();
        fs::write(src.join("sub/dup.png"), png_bytes(20)).unwrap(); // b と同一
        fs::write(src.join("broken.pdf"), b"not a pdf").unwrap();
        fs::write(src.join("empty.png"), b"").unwrap();
        fs::write(src.join("memo.txt"), b"x").unwrap(); // 対象外(フォルダ内)
        fs::write(src.join(".hidden.png"), png_bytes(30)).unwrap();
        let store = Store::open_in_memory().unwrap();
        let res = import_all(&store, &data, &[src.clone()]);
        let imported = res.iter().filter(|r| matches!(r.outcome, Outcome::Imported { .. })).count();
        let dup = res.iter().filter(|r| r.outcome == Outcome::Duplicate).count();
        let failed: Vec<_> = res.iter().filter_map(|r| if let Outcome::Failed(m) = &r.outcome { Some(m.clone()) } else { None }).collect();
        assert_eq!((imported, dup, failed.len()), (2, 1, 2));
        assert!(failed.iter().any(|m| m.contains("空")));
        assert_eq!(fs::read_dir(data.join("originals")).unwrap().count(), 2);
        assert!(src.join("a.png").exists()); // 元のファイルはそのまま
        fs::remove_dir_all(&src).ok();
        fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn 読み取りジョブは下書きを作り_一件の失敗で止まらず_再実行できる() {
        let src = tmp("src2");
        let data = tmp("data2");
        fs::write(src.join("a.png"), png_bytes(1)).unwrap();
        fs::write(src.join("b.png"), png_bytes(2)).unwrap();
        let store = Store::open_in_memory().unwrap();
        import_all(&store, &data, &[src.clone()]);
        // 片方の原本のコピーを消して、失敗させる
        let victim: String = store.db.conn.query_row("SELECT stored_path FROM documents ORDER BY id LIMIT 1", [], |r| r.get(0)).unwrap();
        fs::remove_file(victim).unwrap();
        let mut last = None;
        let n = run_read_jobs(&store, &fake(), &CancelToken::new(), |s| last = Some(s.clone())).unwrap();
        assert_eq!(n, 2);
        let s = Jobs::new(&store.db).summary(Some(JOB_KIND)).unwrap();
        assert_eq!((s.done, s.failed), (1, 1));
        assert_eq!(last.unwrap().fraction(), 1.0);
        let rows = store.search(&Search::default()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "draft"); // 自動では確定しない
        assert_eq!(rows[0].receipt.vendor.as_deref(), Some("有限会社みどり商店"));
        assert_eq!((rows[0].receipt.date.as_deref(), rows[0].receipt.total), (Some("2026-04-01"), Some(880)));
        fs::remove_dir_all(&src).ok();
        fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn 中断したジョブは再開で続きから() {
        let src = tmp("src3");
        let data = tmp("data3");
        for i in 0..3u8 {
            fs::write(src.join(format!("{i}.png")), png_bytes(i + 50)).unwrap();
        }
        let store = Store::open_in_memory().unwrap();
        import_all(&store, &data, &[src.clone()]);
        let c = CancelToken::new();
        let mut n = 0;
        run_read_jobs(&store, &fake(), &c, |_| { n += 1; if n == 1 { c.cancel(); } }).unwrap();
        assert_eq!(Jobs::new(&store.db).summary(None).unwrap().pending, 2);
        Jobs::new(&store.db).recover().unwrap();
        run_read_jobs(&store, &fake(), &CancelToken::new(), |_| {}).unwrap();
        assert_eq!(store.search(&Search::default()).unwrap().len(), 3);
        fs::remove_dir_all(&src).ok();
        fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn テストセットのスキャンpdfから埋め込みjpegを取り出せる() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset/files/31_invoice_scanpdf.pdf");
        let bytes = fs::read(p).unwrap();
        assert!(crate::extract::pdf_text_layer(&bytes).is_none());
        let j = crate::extract::pdf_embedded_jpegs(&bytes);
        assert_eq!(j.len(), 1);
        assert!(factory_core::ocr::OcrImage::decode(&j[0].1).is_ok());
        let t = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset/files/16_invoice_textpdf.pdf")).unwrap();
        let text = crate::extract::pdf_text_layer(&t).unwrap();
        assert!(text.contains("請求書"));
    }
}
