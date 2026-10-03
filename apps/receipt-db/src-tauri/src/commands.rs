//! 画面(`ui/src/api.ts` の `Api`)から呼ばれるコマンドの中身。Tauri に依存しない純Rustで、
//! `tauri_glue.rs`(feature `tauri`)の `#[tauri::command]` が、これを呼ぶ。型は camelCase の JSON(`ui/src/types.ts` と同じ形)。
//!
//! - 取り込みは data URL(画面のファイル選択・ドラッグ&ドロップ)で受け取り、`import::import_bytes` に渡す。
//! - 読み取り(OCR)は別の接続で開いた `Store` で行う。読み取り中も一覧・確認画面の操作を止めないため。
//! - 外部への通信は無い。ログに領収書の内容を書かない。

use crate::extract::low_confidence_fields;
use crate::import::{import_bytes, run_read_jobs, Outcome, JOB_KIND};
use crate::store::{Row, Search, Store};
use base64::Engine;
use factory_core::export::CsvEncoding;
use factory_core::jobs::{CancelToken, Jobs, Summary};
use factory_core::ocr::Ocr;
use factory_core::settings::{AppData, NetworkEntry, Settings};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub const DB_FILE: &str = "receipts.sqlite3";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFileDto {
    pub name: String,
    pub data_url: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportResultDto {
    pub name: String,
    /// "imported" | "duplicate" | "failed"
    pub outcome: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProgressDto {
    pub pending: usize,
    pub running: usize,
    pub done: usize,
    pub failed: usize,
    pub busy: bool,
    /// 失敗したジョブの理由(書類名つき。新しい順に最大20件)
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IssueDto {
    pub field: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptDto {
    pub id: i64,
    pub document_id: i64,
    pub original_name: String,
    /// "pdf" | "jpeg" | "png"
    pub kind: String,
    pub date: Option<String>,
    pub vendor: Option<String>,
    pub total: Option<i64>,
    pub tax_rate: Option<u8>,
    pub invoice_no: Option<String>,
    pub summary: Option<String>,
    pub account_candidate: Option<String>,
    /// "draft" | "confirmed"
    pub status: String,
    /// 信頼度が低い項目(画面で印を付ける)。キーは store の列名(snake_case)
    pub low_confidence: Vec<String>,
    pub issues: Vec<IssueDto>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchDto {
    #[serde(default)]
    pub text: String,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub min_total: Option<i64>,
    pub max_total: Option<i64>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RejectedDto {
    pub id: i64,
    pub fields: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub update_check: bool,
    pub network: Vec<NetworkEntry>,
    pub data_dir: String,
    /// "real"(ONNX Runtime 上の PP-OCR)| "fake"(モデル未設定。読み取りは空で、手入力で確認する)
    pub ocr: &'static str,
}

pub struct AppState {
    pub store: Mutex<Store>,
    pub ocr: Box<dyn Ocr + Send + Sync>,
    pub ocr_kind: &'static str,
    pub app: AppData,
    pub cancel: Mutex<CancelToken>,
    pub busy: AtomicBool,
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn open_store(path: &std::path::Path) -> Result<Store, String> {
    let s = Store::open(path).map_err(err)?;
    // 読み取り用の接続と画面用の接続が同じファイルを使うので、ロック待ちを許す
    s.db.conn.busy_timeout(std::time::Duration::from_secs(10)).map_err(err)?;
    Ok(s)
}

fn decode_data_url(s: &str) -> Result<Vec<u8>, String> {
    let (head, body) = s.split_once(',').ok_or("ファイルのデータが不正です")?;
    if !head.ends_with(";base64") {
        return Err("ファイルのデータが不正です".into());
    }
    base64::engine::general_purpose::STANDARD.decode(body.trim()).map_err(|_| "ファイルのデータが不正です".to_string())
}

impl AppState {
    pub fn new(data_dir: PathBuf, ocr: Box<dyn Ocr + Send + Sync>, ocr_kind: &'static str) -> Result<Self, String> {
        let app = AppData::init(&data_dir).map_err(err)?;
        let store = open_store(&data_dir.join(DB_FILE))?;
        // 前回、読み取りの途中で終了していたら続きから
        Jobs::new(&store.db).recover().map_err(err)?;
        Ok(Self { store: Mutex::new(store), ocr, ocr_kind, app, cancel: Mutex::new(CancelToken::new()), busy: AtomicBool::new(false) })
    }

    fn db_path(&self) -> PathBuf {
        self.app.root.join(DB_FILE)
    }

    fn store(&self) -> std::sync::MutexGuard<'_, Store> {
        self.store.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn import_files(&self, files: Vec<ImportFileDto>) -> Vec<ImportResultDto> {
        let store = self.store();
        files
            .into_iter()
            .map(|f| {
                let outcome = match decode_data_url(&f.data_url) {
                    Ok(bytes) => import_bytes(&store, &self.app.root, &f.name, &bytes),
                    Err(m) => Outcome::Failed(m),
                };
                let (outcome, message) = match outcome {
                    Outcome::Imported { .. } => ("imported", String::new()),
                    Outcome::Duplicate => ("duplicate", "取り込み済みのファイルです".to_string()),
                    Outcome::Failed(m) => ("failed", m),
                };
                ImportResultDto { name: f.name, outcome, message }
            })
            .collect()
    }

    /// 待ちの読み取りジョブをすべて実行する(呼び出し側の別スレッドで)。すでに実行中なら何もしない。
    pub fn run_jobs(&self) -> Result<usize, String> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Ok(0);
        }
        let token = {
            let mut c = self.cancel.lock().unwrap_or_else(|p| p.into_inner());
            *c = CancelToken::new();
            c.clone()
        };
        let res = open_store(&self.db_path()).and_then(|worker| run_read_jobs(&worker, self.ocr.as_ref(), &token, |_| {}).map_err(err));
        self.busy.store(false, Ordering::SeqCst);
        res
    }

    /// 読み取りを止める(実行中の1件が終わったところで止まる。残りは「待ち」のまま、次回に再開できる)。
    pub fn cancel_jobs(&self) {
        self.cancel.lock().unwrap_or_else(|p| p.into_inner()).cancel();
    }

    pub fn retry_failed(&self) -> Result<usize, String> {
        Jobs::new(&self.store().db).retry_failed().map_err(err)
    }

    pub fn progress(&self) -> Result<ProgressDto, String> {
        let store = self.store();
        let s: Summary = Jobs::new(&store.db).summary(Some(JOB_KIND)).map_err(err)?;
        let mut stmt = store
            .db
            .conn
            .prepare(
                "SELECT j.error, d.original_name FROM jobs j LEFT JOIN documents d ON d.id = json_extract(j.payload_json, '$.document_id')
                 WHERE j.kind=?1 AND j.state='failed' ORDER BY j.id DESC LIMIT 20",
            )
            .map_err(err)?;
        let errors = stmt
            .query_map([JOB_KIND], |r| Ok(format!("{}: {}", r.get::<_, Option<String>>(1)?.unwrap_or_default(), r.get::<_, Option<String>>(0)?.unwrap_or_default())))
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(ProgressDto { pending: s.pending, running: s.running, done: s.done, failed: s.failed, busy: self.busy.load(Ordering::SeqCst), errors })
    }

    fn to_dto(store: &Store, r: Row) -> Result<ReceiptDto, String> {
        let (original_name, kind): (String, String) = store
            .db
            .conn
            .query_row("SELECT original_name, kind FROM documents WHERE id=?1", [r.document_id], |x| Ok((x.get(0)?, x.get(1)?)))
            .map_err(err)?;
        let issues = r.receipt.validate().into_iter().map(|i| IssueDto { field: i.field, message: i.message }).collect();
        let t = r.receipt;
        Ok(ReceiptDto {
            id: r.id, document_id: r.document_id, original_name, kind,
            date: t.date, vendor: t.vendor, total: t.total, tax_rate: t.tax_rate, invoice_no: t.invoice_no, summary: t.summary,
            account_candidate: r.account_candidate, status: r.status, low_confidence: low_confidence_fields(&r.confidence), issues,
        })
    }

    pub fn list(&self, q: SearchDto) -> Result<Vec<ReceiptDto>, String> {
        let store = self.store();
        let blank = |s: Option<String>| s.filter(|x| !x.trim().is_empty());
        let rows = store
            .search(&Search { text: q.text, date_from: blank(q.date_from), date_to: blank(q.date_to), min_total: q.min_total, max_total: q.max_total, status: blank(q.status) })
            .map_err(err)?;
        rows.into_iter().map(|r| Self::to_dto(&store, r)).collect()
    }

    pub fn get(&self, id: i64) -> Result<ReceiptDto, String> {
        let store = self.store();
        let r = store.get(id).map_err(err)?.ok_or("見つかりません")?;
        Self::to_dto(&store, r)
    }

    /// 原本のコピーを data URL で返す(確認画面で並べて表示する)。
    pub fn original(&self, document_id: i64) -> Result<String, String> {
        let (path, kind): (String, String) = self
            .store()
            .db
            .conn
            .query_row("SELECT stored_path, kind FROM documents WHERE id=?1", [document_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|_| "書類が見つかりません".to_string())?;
        let bytes = std::fs::read(path).map_err(|_| "原本のコピーを読めません".to_string())?;
        let mime = match kind.as_str() {
            "pdf" => "application/pdf",
            "png" => "image/png",
            _ => "image/jpeg",
        };
        Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
    }

    pub fn edit(&self, id: i64, field: &str, value: Option<String>) -> Result<ReceiptDto, String> {
        {
            let store = self.store();
            let v = value.as_deref().map(str::trim).filter(|s| !s.is_empty());
            store.edit(id, field, v).map_err(err)?;
        }
        self.get(id)
    }

    pub fn undo(&self, id: i64) -> Result<ReceiptDto, String> {
        self.store().undo(id).map_err(err)?;
        self.get(id)
    }

    /// 確定(一括可)。利用者が確認画面で押したときだけ呼ぶ。形に問題がある行は確定せずに返す。
    pub fn confirm(&self, ids: Vec<i64>) -> Result<Vec<RejectedDto>, String> {
        Ok(self.store().confirm(&ids).map_err(err)?.into_iter().map(|(id, fields)| RejectedDto { id, fields }).collect())
    }

    pub fn unconfirm(&self, id: i64) -> Result<(), String> {
        self.store().unconfirm(id).map_err(err)
    }

    pub fn delete(&self, id: i64) -> Result<(), String> {
        self.store().delete_receipt(id).map(|_| ()).map_err(err)
    }

    /// CSV(確定済みだけ)を `dest` に書く。
    pub fn export_csv(&self, encoding: &str, dest: &std::path::Path) -> Result<(), String> {
        let enc = if encoding == "sjis" { CsvEncoding::ShiftJis } else { CsvEncoding::Utf8Bom };
        let bytes = self.store().export_csv(enc).map_err(err)?;
        std::fs::write(dest, bytes).map_err(|_| "書き出し先に保存できません".to_string())
    }

    pub fn export_sqlite(&self, dest: &std::path::Path) -> Result<(), String> {
        self.store().export_sqlite(dest).map_err(err)
    }

    pub fn settings(&self) -> SettingsDto {
        SettingsDto {
            update_check: self.app.load_settings().update_check,
            network: crate::app_settings::network_list(&self.app),
            data_dir: self.app.root.to_string_lossy().into(),
            ocr: self.ocr_kind,
        }
    }

    pub fn set_update_check(&self, on: bool) -> Result<SettingsDto, String> {
        let s = Settings { update_check: on, ..self.app.load_settings() };
        self.app.save_settings(&s).map_err(err)?;
        Ok(self.settings())
    }

    /// 全データ(原本のコピー・データベース・設定)を消す。読み取り中は消さない。
    pub fn delete_all(&self) -> Result<usize, String> {
        if self.busy.load(Ordering::SeqCst) {
            return Err("読み取り中です。止めてから消してください".into());
        }
        let mut store = self.store();
        // 開いている DB を閉じてから消し、空の DB を開き直す
        *store = Store::open_in_memory().map_err(err)?;
        let n = self.app.delete_all().map_err(err)?;
        *store = open_store(&self.db_path())?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use factory_core::ocr::{BBox, FakeOcr, OcrLine, OcrPage};

    fn tmp(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rdb-cmd-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn fake() -> Box<FakeOcr> {
        let l = |t: &str, y: f32| OcrLine { text: t.into(), bbox: BBox { x: 0.0, y, w: 100.0, h: 20.0 }, vertical: false, confidence: 0.95, alt_text: None };
        Box::new(FakeOcr { page: OcrPage { width: 100.0, height: 100.0, lines: vec![l("有限会社みどり商店", 0.0), l("2026/04/01", 25.0), l("合計 ¥880", 50.0)] } })
    }

    fn png_url(seed: u8) -> String {
        let mut b = Vec::new();
        image::DynamicImage::ImageLuma8(image::GrayImage::from_pixel(8, 8, image::Luma([seed])))
            .write_to(&mut std::io::Cursor::new(&mut b), image::ImageFormat::Png)
            .unwrap();
        format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b))
    }

    #[test]
    fn 取り込み_読み取り_修正_確定_書き出し_全削除まで通る() {
        let d = tmp("flow");
        let s = AppState::new(d.clone(), fake(), "fake").unwrap();
        let res = s.import_files(vec![
            ImportFileDto { name: "a.png".into(), data_url: png_url(1) },
            ImportFileDto { name: "a2.png".into(), data_url: png_url(1) },
            ImportFileDto { name: "x.txt".into(), data_url: "data:text/plain;base64,eA==".into() },
        ]);
        assert_eq!(res.iter().map(|r| r.outcome).collect::<Vec<_>>(), ["imported", "duplicate", "failed"]);
        assert_eq!(s.run_jobs().unwrap(), 1);
        assert_eq!(s.progress().unwrap().done, 1);

        let rows = s.list(SearchDto::default()).unwrap();
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!((r.status.as_str(), r.original_name.as_str(), r.total), ("draft", "a.png", Some(880)));
        assert!(s.original(r.document_id).unwrap().starts_with("data:image/png;base64,"));

        // 下書きは CSV に出ない
        let csv = d.join("out.csv");
        s.export_csv("utf8", &csv).unwrap();
        assert!(!std::fs::read_to_string(&csv).unwrap().contains("みどり"));

        let e = s.edit(r.id, "total", Some("1,100".into())).unwrap();
        assert_eq!(e.total, Some(1100));
        assert_eq!(s.undo(r.id).unwrap().total, Some(880));
        assert!(s.confirm(vec![r.id]).unwrap().is_empty());
        s.export_csv("utf8", &csv).unwrap();
        assert!(std::fs::read_to_string(&csv).unwrap().contains("有限会社みどり商店"));
        assert_eq!(s.list(SearchDto { text: "みどり".into(), ..Default::default() }).unwrap().len(), 1);

        std::fs::remove_file(&csv).unwrap();
        s.delete_all().unwrap();
        assert!(s.list(SearchDto::default()).unwrap().is_empty());
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 形に問題がある行は確定しない() {
        let d = tmp("reject");
        let s = AppState::new(d.clone(), fake(), "fake").unwrap();
        s.import_files(vec![ImportFileDto { name: "a.png".into(), data_url: png_url(9) }]);
        s.run_jobs().unwrap();
        let id = s.list(SearchDto::default()).unwrap()[0].id;
        s.edit(id, "date", Some("2026-02-30".into())).unwrap();
        let rej = s.confirm(vec![id]).unwrap();
        assert_eq!(rej, vec![RejectedDto { id, fields: vec!["date"] }]);
        assert_eq!(s.get(id).unwrap().status, "draft");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 設定_更新確認を止められる() {
        let d = tmp("set");
        let s = AppState::new(d.clone(), fake(), "fake").unwrap();
        assert!(s.settings().update_check);
        let after = s.set_update_check(false).unwrap();
        assert!(!after.update_check);
        assert!(after.network.iter().any(|e| e.stoppable && !e.enabled));
        std::fs::remove_dir_all(&d).ok();
    }
}
