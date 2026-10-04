//! 画面(`ui/src/api.ts`)から呼ばれるコマンドの中身。Tauri に依存しない純Rust。`tauri_glue.rs` が包む。
//!
//! - 取り込みは、ファイルの場所(OS のダイアログで選んだもの)を受け取り、アプリのデータフォルダにコピーする(元は変更しない)。
//! - 処理(読み込み・ノイズ除去・文字起こし)は別の接続で開いた `Store` で行い、画面の操作を止めない。
//! - 外部への通信は無い。ログに文字起こしの内容を書かない。

use crate::asr::Asr;
use crate::export::{render, Format};
use crate::pipeline::{self, Options, JOB_KIND};
use crate::diarize::Embedder;
use crate::plan::{Entitlements, Ledger, Tier, Usage, PRO_ONLY};
use crate::store::{GlossaryEntry, Meeting, MeetingFilter, ProcessOptions, SearchHit, Segment, Store, Todo};
use factory_core::jobs::Jobs;
use factory_core::model_manager::{ModelManager, ModelSpec, ModelStatus};
use factory_core::settings::{AppData, NetworkEntry, Settings};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub const DB_FILE: &str = "minutes.sqlite3";
pub const UPDATE_PURPOSE: &str = "更新の確認";
/// 取り込める拡張子(動画は音声だけを使う)
pub const AUDIO_EXTS: [&str; 9] = ["m4a", "mp3", "wav", "mp4", "aac", "flac", "ogg", "mov", "m4v"];
const MAX_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// 文字起こしのモデル(whisper.cpp 形式、Whisper large-v3-turbo の q5_0 量子化、MIT)。
/// 取得元は whisper.cpp の公式の配布(Hugging Face `ggerganov/whisper.cpp`)。ハッシュは 2026-10-04 に取得して確認した値。
pub const WHISPER_MODEL: ModelSpec = ModelSpec {
    name: "Whisper large-v3-turbo(q5_0)",
    file_name: "ggml-large-v3-turbo-q5_0.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
    sha256: "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
    size: 574_041_195,
};

/// 話者の判別の特徴を作る関数(本番は ONNX の WeSpeaker、テストは差し替え)。無ければ判別しない
pub type EmbedderLoader = Box<dyn Fn() -> Result<Box<dyn Embedder>, String> + Send + Sync>;

/// モデルの場所から文字起こしエンジンを作る関数(本番は whisper、テストは差し替え)
pub type AsrLoader = Box<dyn Fn(&Path) -> Result<Box<dyn Asr>, String> + Send + Sync>;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelDto {
    #[serde(flatten)]
    pub status: ModelStatus,
    pub downloading: bool,
    /// "bundled"(アプリに同梱)| "env"(開発用に環境変数で指定)| "managed"(設定画面から取得したもの)| "none"
    pub source: &'static str,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProgressDto {
    pub busy: bool,
    pub pending: usize,
    /// 処理中の議事録と、その区間の進み具合
    pub meeting_id: Option<i64>,
    pub done_chunks: i64,
    pub total_chunks: i64,
}

/// 話者の判別し直しの進み具合。`phase` は "prepare"(音声の読み込み)| "embed"(声の特徴を求める)
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RediarizeDto {
    pub meeting_id: i64,
    pub done: i64,
    pub total: i64,
    pub phase: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DetailDto {
    pub meeting: Meeting,
    pub segments: Vec<Segment>,
    pub can_undo: bool,
    pub speakers: Vec<String>,
    pub low_confidence: f64,
    /// 録音中の仮の文字(精度が低い)が含まれる。正確な文字起こしが終わるまで編集できない
    pub provisional: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub update_check: bool,
    pub keep_audio: bool,
    pub denoise_default: bool,
    pub consent_shown: bool,
    pub network: Vec<NetworkEntry>,
    pub data_dir: String,
    /// 文字起こしのモデル名。"none" ならモデル未設定
    pub model: String,
    /// 話者の判別が使えるか(使えなければ理由)
    pub diarize_available: bool,
    pub diarize_error: Option<String>,
}

pub fn network_entries() -> Vec<NetworkEntry> {
    let e = |p: &str, d: &str, c: &str, stoppable: bool| NetworkEntry { purpose: p.into(), destination: d.into(), content: c.into(), stoppable, enabled: true };
    vec![
        e("ライセンス認証・検証", "販売プラットフォームのAPI", "ライセンスキー、端末の識別名", false),
        e(UPDATE_PURPOSE, "配布元", "アプリのバージョン", true),
        e("モデルの取得(操作したときのみ)", "モデルの配布元(文字起こし・要約のモデル)", "モデル名", false),
    ]
}

pub struct AppState {
    pub store: Mutex<Store>,
    /// 文字起こしエンジン(初めて使うときに読み込む。モデルが無ければ None のまま)
    asr: Mutex<Option<Arc<dyn Asr>>>,
    loader: AsrLoader,
    /// 決まった場所のモデル(優先順): 開発用の環境変数 → アプリに同梱したもの。(場所, "env" | "bundled")
    fixed_models: Vec<(PathBuf, &'static str)>,
    pub models: ModelManager,
    /// 第三者ライセンス表記(アプリに同梱した THIRD_PARTY_NOTICES.txt)
    pub notices_path: Mutex<Option<PathBuf>>,
    model_cancel: AtomicBool,
    model_dl: Mutex<(bool, u64, Option<String>)>,
    pub app: AppData,
    pub cancel: Arc<AtomicBool>,
    pub busy: AtomicBool,
    pub current: Mutex<(Option<i64>, i64, i64)>,
    emb: Mutex<Option<Arc<dyn Embedder>>>,
    emb_loader: Mutex<Option<EmbedderLoader>>,
    emb_error: Mutex<Option<String>>,
    /// 話者の判別し直しの進み具合(していなければ None)と、中断の合図
    diar: Mutex<Option<RediarizeDto>>,
    diar_cancel: AtomicBool,
    recorder: Mutex<Option<crate::recorder::LiveRecorder>>,
    /// 録音中の仮の文字に使う、速い小さなモデル(初回に読み込む)と、その場所
    live_asr: Mutex<Option<Arc<dyn Asr>>>,
    live_models: Mutex<Vec<PathBuf>>,
    /// 無料版/有料版(ライセンスで決まる。いまは開発用の判定)
    tier: Mutex<Tier>,
    /// 無料版の使った量の記録
    ledger: Mutex<Ledger>,
    /// 要約(有料版の追加機能。モデルは後から取得、推論は別プロセス)
    pub summary: crate::summary::SummaryRuntime,
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// ライセンスとつなぐまでの判定: 環境変数 MINUTES_TIER(pro / free)→ 開発用のビルドは有料版、配布用は無料版。
fn default_tier() -> Tier {
    match std::env::var("MINUTES_TIER").as_deref() {
        Ok("pro") => Tier::Pro,
        Ok("free") => Tier::Free,
        _ if cfg!(debug_assertions) => Tier::Pro,
        _ => Tier::Free,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlanDto {
    #[serde(flatten)]
    pub ent: Entitlements,
    pub usage: Usage,
    /// 無料版の残り(ミリ秒)。有料版は None
    pub remaining_ms: Option<u64>,
}

fn flag(s: &Settings, key: &str, default: bool) -> bool {
    s.extra.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

impl AppState {
    /// `asr` を渡すと、それを使う(テスト用)。None なら、モデルの場所から `loader` で読み込む。
    pub fn new(data_dir: PathBuf, asr: Option<Box<dyn Asr>>, loader: AsrLoader, fixed_models: Vec<(PathBuf, &'static str)>) -> Result<Self, String> {
        let data_dir_for_ledger = data_dir.clone();
        let app = AppData::init(&data_dir).map_err(err)?;
        let models = ModelManager::new(data_dir.join("models"));
        let store = Store::open(&data_dir.join(DB_FILE)).map_err(err)?;
        // 前回、処理の途中で終了していたら、続きの区間から再開できるようにする
        Jobs::new(&store.db).recover().map_err(err)?;
        // 録音中にアプリが終わった議事録は、そこまでの文字を残して「失敗」にする(音声は作業用のまま残らない)
        store
            .db
            .conn
            .execute("UPDATE meetings SET state='failed', recording=0, error='録音中にアプリが終了しました(そこまでの文字は残っています)' WHERE recording=1", [])
            .map_err(err)?;
        store.db.conn.execute("UPDATE meetings SET state='queued' WHERE state='processing'", []).map_err(err)?;
        Ok(Self {
            store: Mutex::new(store),
            asr: Mutex::new(asr.map(Arc::from)),
            loader,
            fixed_models,
            models,
            notices_path: Mutex::new(None),
            model_cancel: AtomicBool::new(false),
            model_dl: Mutex::new((false, 0, None)),
            app,
            cancel: Arc::new(AtomicBool::new(false)),
            busy: AtomicBool::new(false),
            current: Mutex::new((None, 0, 0)),
            emb: Mutex::new(None),
            emb_loader: Mutex::new(None),
            emb_error: Mutex::new(None),
            diar: Mutex::new(None),
            diar_cancel: AtomicBool::new(false),
            recorder: Mutex::new(None),
            live_asr: Mutex::new(None),
            live_models: Mutex::new(Vec::new()),
            tier: Mutex::new(default_tier()),
            ledger: Mutex::new(Ledger::new(vec![Box::new(crate::plan::FileSlot(data_dir_for_ledger.join("usage.dat")))])),
            summary: crate::summary::SummaryRuntime::new(data_dir_for_ledger.join("models")),
        })
    }

    fn store(&self) -> std::sync::MutexGuard<'_, Store> {
        self.store.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn db_path(&self) -> PathBuf {
        self.app.root.join(DB_FILE)
    }

    /// 話者の判別の特徴を作る関数を設定する(起動時)。
    pub fn set_embedder_loader(&self, l: EmbedderLoader) {
        *self.emb_loader.lock().unwrap_or_else(|p| p.into_inner()) = Some(l);
    }

    /// 話者の判別の特徴(初回に読み込む)。使えなければ None(理由は `embedder_error`)。
    pub fn embedder(&self) -> Option<Arc<dyn Embedder>> {
        let mut g = self.emb.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(e) = g.as_ref() {
            return Some(e.clone());
        }
        let loader = self.emb_loader.lock().unwrap_or_else(|p| p.into_inner());
        match loader.as_ref().map(|l| l()) {
            Some(Ok(e)) => {
                let e: Arc<dyn Embedder> = Arc::from(e);
                *g = Some(e.clone());
                Some(e)
            }
            Some(Err(msg)) => {
                *self.emb_error.lock().unwrap_or_else(|p| p.into_inner()) = Some(msg);
                None
            }
            None => None,
        }
    }

    pub fn embedder_error(&self) -> Option<String> {
        self.emb_error.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    // ---------------- 取り込みと処理 ----------------

    /// 録音ファイルを取り込む(コピーし、処理の待ちに入れる)。議事録の id を返す。
    /// 無料版で使えない設定を外す
    fn allowed_options(&self, o: &ProcessOptions) -> ProcessOptions {
        let e = self.ent();
        let mut o = o.clone();
        o.denoise &= e.denoise;
        o.diarize &= e.diarize;
        o
    }

    pub fn import_audio(&self, path: &Path, opts: &ProcessOptions) -> Result<i64, String> {
        let opts = &self.allowed_options(opts);
        if self.plan().remaining_ms == Some(0) {
            return Err(crate::pipeline::LIMIT_REACHED.into());
        }
        let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).unwrap_or_default();
        if !AUDIO_EXTS.contains(&ext.as_str()) {
            return Err("対応している形式は m4a / mp3 / wav / mp4 などです".into());
        }
        let meta = std::fs::metadata(path).map_err(|_| "ファイルを開けません".to_string())?;
        if meta.len() == 0 {
            return Err("空のファイルです".into());
        }
        if meta.len() > MAX_BYTES {
            return Err("ファイルが大きすぎます(4GBまで)".into());
        }
        let dir = self.app.root.join("audio");
        std::fs::create_dir_all(&dir).map_err(|_| "保存先を作れません".to_string())?;
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let stem = path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "録音".into());
        let store = self.store();
        let id = store.add_meeting(&stem, &name, "", opts.denoise).map_err(err)?;
        if let Err(e) = store.set_options(id, opts) {
            let _ = store.delete_meeting(id);
            return Err(e.to_string());
        }
        let dest = dir.join(format!("{id}.{ext}"));
        if std::fs::copy(path, &dest).is_err() {
            let _ = store.delete_meeting(id);
            return Err("音声のコピーを保存できません(空き容量を確認してください)".into());
        }
        store.db.conn.execute("UPDATE meetings SET audio_path=?2 WHERE id=?1", (id, dest.to_string_lossy())).map_err(err)?;
        pipeline::enqueue(&store, id)?;
        Ok(id)
    }

    /// 待ちの処理をすべて実行する(呼び出し側の別スレッドで)。実行中なら何もしない。
    pub fn run_jobs(&self) -> Result<usize, String> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Ok(0);
        }
        self.cancel.store(false, Ordering::SeqCst);
        let keep_audio = flag(&self.app.load_settings(), "keep_audio", true);
        let asr = match self.engine() {
            Ok(a) => a,
            Err(e) => {
                self.busy.store(false, Ordering::SeqCst);
                return Err(e);
            }
        };
        let ent = self.ent();
        let emb = if ent.diarize { self.embedder() } else { None };
        let remaining = || self.ledger.lock().unwrap_or_else(|p| p.into_inner()).remaining(&ent).unwrap_or(u64::MAX);
        let on_used = |ms: u64, first: bool| self.ledger.lock().unwrap_or_else(|p| p.into_inner()).add(ms, first);
        let res = Store::open(&self.db_path()).map_err(err).and_then(|worker| {
            let free = ent.total_limit_ms.is_some();
            let opts = Options {
                keep_audio,
                embedder: emb.as_deref(),
                meeting_limit_ms: ent.meeting_limit_ms,
                remaining_ms: if free { Some(&remaining) } else { None },
                on_used: if free { Some(&on_used) } else { None },
                use_glossary: ent.glossary,
            };
            pipeline::run_jobs(&worker, asr.as_ref(), &self.app.root.join("work"), &opts, &self.cancel, |mid, d, t| {
                *self.current.lock().unwrap_or_else(|p| p.into_inner()) = (Some(mid), d, t);
            })
        });
        *self.current.lock().unwrap_or_else(|p| p.into_inner()) = (None, 0, 0);
        self.busy.store(false, Ordering::SeqCst);
        res
    }

    /// 中断(処理中の区間が終わるか、文字起こしが途中で止まったところで止まる。続きは次回の再開で)。
    pub fn cancel_jobs(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// 失敗した議事録をやり直す(待ちに戻す)。
    pub fn retry(&self, meeting_id: i64) -> Result<(), String> {
        let store = self.store();
        store
            .db
            .conn
            .execute("DELETE FROM jobs WHERE kind=?1 AND json_extract(payload_json, '$.meeting_id')=?2", (JOB_KIND, meeting_id))
            .map_err(err)?;
        store.set_state(meeting_id, "queued", None).map_err(err)?;
        pipeline::enqueue(&store, meeting_id).map(|_| ())
    }

    pub fn progress(&self) -> Result<ProgressDto, String> {
        let s = Jobs::new(&self.store().db).summary(Some(JOB_KIND)).map_err(err)?;
        let (mid, d, t) = *self.current.lock().unwrap_or_else(|p| p.into_inner());
        Ok(ProgressDto { busy: self.busy.load(Ordering::SeqCst), pending: s.pending + s.running, meeting_id: mid, done_chunks: d, total_chunks: t })
    }

    /// 使うモデル: 開発用の環境変数 → アプリに同梱 → 設定画面から取得したもの の順。
    fn model_source(&self) -> Option<(PathBuf, &'static str)> {
        self.fixed_models
            .iter()
            .find(|(p, _)| std::fs::metadata(p).map(|m| m.len() > 0).unwrap_or(false))
            .cloned()
            .or_else(|| self.models.installed_path(&WHISPER_MODEL).map(|p| (p, "managed")))
    }

    fn model_path(&self) -> Option<PathBuf> {
        self.model_source().map(|(p, _)| p)
    }

    // ---------------- 無料版・有料版 ----------------

    pub fn ent(&self) -> Entitlements {
        Entitlements::of(*self.tier.lock().unwrap_or_else(|p| p.into_inner()))
    }

    pub fn set_tier(&self, t: Tier) {
        *self.tier.lock().unwrap_or_else(|p| p.into_inner()) = t;
    }

    /// 使った量の記録の場所を設定する(起動時。データフォルダの外とキーチェーンにも置く)
    pub fn set_ledger(&self, l: Ledger) {
        *self.ledger.lock().unwrap_or_else(|p| p.into_inner()) = l;
    }

    pub fn plan(&self) -> PlanDto {
        let ent = self.ent();
        let l = self.ledger.lock().unwrap_or_else(|p| p.into_inner());
        PlanDto { remaining_ms: l.remaining(&ent), usage: l.usage(), ent }
    }

    fn require(&self, ok: bool) -> Result<(), String> {
        if ok { Ok(()) } else { Err(PRO_ONLY.into()) }
    }

    /// 文字起こしエンジン。有料版は正確なモデル、無料版は小さなモデル。
    fn engine(&self) -> Result<Arc<dyn Asr>, String> {
        if !self.ent().accurate_model {
            return self.live_engine();
        }
        self.accurate_engine()
    }

    /// 正確なモデル(初回に読み込む)。モデルが無ければ、その旨のエラー(待ちのジョブはそのまま残る)。
    fn accurate_engine(&self) -> Result<Arc<dyn Asr>, String> {
        let mut g = self.asr.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(a) = g.as_ref() {
            return Ok(a.clone());
        }
        let path = self.model_path().ok_or("文字起こしのモデルがありません。設定の「文字起こしのモデル」から取得してください")?;
        let a: Arc<dyn Asr> = Arc::from((self.loader)(&path)?);
        *g = Some(a.clone());
        Ok(a)
    }

    pub fn model_status(&self) -> ModelDto {
        let (downloading, _, error) = self.model_dl.lock().unwrap_or_else(|p| p.into_inner()).clone();
        let mut status = self.models.status(&WHISPER_MODEL);
        let source = match self.model_source() {
            Some((_, src)) => {
                status.installed = true;
                status.downloaded = status.size;
                src
            }
            None => "none",
        };
        if downloading {
            status.downloaded = self.model_dl.lock().unwrap_or_else(|p| p.into_inner()).1;
        }
        ModelDto { status, downloading, source, error }
    }

    /// モデルを取得する(利用者が設定画面で押したときだけ呼ぶ。通信一覧の「モデルの取得」)。終わるまで返らない。
    pub fn download_model(&self) -> Result<ModelDto, String> {
        {
            let mut g = self.model_dl.lock().unwrap_or_else(|p| p.into_inner());
            if g.0 {
                return Err("取得中です".into());
            }
            *g = (true, 0, None);
        }
        self.model_cancel.store(false, Ordering::SeqCst);
        let res = self.models.download(&WHISPER_MODEL, &self.model_cancel, |done, _| {
            self.model_dl.lock().unwrap_or_else(|p| p.into_inner()).1 = done;
        });
        let error = match &res {
            Ok(_) => None,
            Err(factory_core::CoreError::Cancelled) => Some("取得を中断しました(続きから再開できます)".to_string()),
            Err(e) => Some(e.to_string()),
        };
        *self.model_dl.lock().unwrap_or_else(|p| p.into_inner()) = (false, 0, error);
        Ok(self.model_status())
    }

    pub fn cancel_model_download(&self) {
        self.model_cancel.store(true, Ordering::SeqCst);
    }

    /// 取得したモデルを消す(処理中は消さない)。
    pub fn delete_model(&self) -> Result<ModelDto, String> {
        if self.busy.load(Ordering::SeqCst) || self.model_dl.lock().unwrap_or_else(|p| p.into_inner()).0 {
            return Err("処理中・取得中は消せません".into());
        }
        *self.asr.lock().unwrap_or_else(|p| p.into_inner()) = None;
        self.models.delete(&WHISPER_MODEL).map_err(err)?;
        Ok(self.model_status())
    }

    // ---------------- 要約(有料版の追加機能) ----------------

    pub fn summary_status(&self) -> crate::summary::SummaryStatusDto {
        self.summary.status()
    }

    /// 要約のモデルを取得する(利用者が設定画面で押したときだけ。通信一覧の「モデルの取得」)。終わるまで返らない。
    pub fn download_summary_model(&self) -> Result<crate::summary::SummaryStatusDto, String> {
        self.require(self.ent().summary)?;
        self.summary.download()
    }

    pub fn cancel_summary_download(&self) {
        self.summary.cancel_download()
    }

    pub fn delete_summary_model(&self) -> Result<crate::summary::SummaryStatusDto, String> {
        self.summary.delete()
    }

    /// 議事録の文字起こしから、要約の下書きを作る。**保存はしない**(取り込みは画面で確認したあとに `update_notes`)。
    pub fn summarize(&self, id: i64) -> Result<crate::summary::SummaryResult, String> {
        self.require(self.ent().summary)?;
        let (title, segs) = {
            let store = self.store();
            let m = store.meeting(id).map_err(err)?.ok_or("見つかりません")?;
            if m.state != "done" {
                return Err("文字起こしが終わってから要約できます".into());
            }
            let segs: Vec<(String, String)> = store.segments(id).map_err(err)?.into_iter().filter(|s| s.chunk_idx >= 0).map(|s| (s.speaker, s.text)).collect();
            (m.title, segs)
        };
        let paras = crate::summary::paragraphs(&segs);
        self.summary.run(id, &title, &paras)
    }

    pub fn cancel_summarize(&self) {
        self.summary.cancel()
    }

    /// 第三者のソフトウェア・モデルのライセンス表記(全文)。
    pub fn third_party_notices(&self) -> Result<String, String> {
        let p = self.notices_path.lock().unwrap_or_else(|p| p.into_inner()).clone().ok_or("ライセンス表記のファイルが見つかりません")?;
        std::fs::read_to_string(p).map_err(|_| "ライセンス表記のファイルを読めません".to_string())
    }

    // ---------------- アプリ内の録音 ----------------

    /// 録音中に使う小さなモデルの候補の場所(起動時に設定)
    pub fn set_live_models(&self, paths: Vec<PathBuf>) {
        *self.live_models.lock().unwrap_or_else(|p| p.into_inner()) = paths;
    }

    /// 録音中の仮の文字のエンジン。小さなモデルが無ければ、正確なモデルを使う(重くなるが動く)。
    fn live_engine(&self) -> Result<Arc<dyn Asr>, String> {
        let mut g = self.live_asr.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(a) = g.as_ref() {
            return Ok(a.clone());
        }
        let path = self.live_models.lock().unwrap_or_else(|p| p.into_inner()).iter().find(|p| p.exists()).cloned();
        let a: Arc<dyn Asr> = match path {
            Some(p) => Arc::from((self.loader)(&p)?),
            // 小さなモデルが無いとき: 有料版は正確なモデルで代わりに動かす。無料版は代わりにしない(正確なモデルは有料版の機能)
            None if self.ent().accurate_model => self.accurate_engine()?,
            None => return Err("文字起こしのモデル(標準)が見つかりません。アプリを入れ直してください".into()),
        };
        *g = Some(a.clone());
        Ok(a)
    }

    /// マイクの録音を始める(議事録を作り、話しながら文字にしていく)。議事録の id を返す。
    pub fn record_start(&self, opts: &ProcessOptions) -> Result<i64, String> {
        let opts = &self.allowed_options(opts);
        if self.plan().remaining_ms == Some(0) {
            return Err(crate::pipeline::LIMIT_REACHED.into());
        }
        let mut rec = self.recorder.lock().unwrap_or_else(|p| p.into_inner());
        if rec.is_some() {
            return Err("すでに録音中です".into());
        }
        let asr = self.live_engine()?;
        let store = self.store();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let title = format!("録音 {}", now);
        let id = store.add_meeting(&title, "マイク録音", "", opts.denoise).map_err(err)?;
        let mut o = opts.clone();
        o.range_start_ms = None;
        o.range_end_ms = None;
        store.set_options(id, &o).map_err(err)?;
        store.set_state(id, "processing", None).map_err(err)?;
        store.set_recording(id, true).map_err(err)?;
        let hint = store.glossary_hint().map_err(err)?;
        let lang = store.meeting(id).map_err(err)?.map(|m| m.language).unwrap_or_else(|| "ja".into());
        drop(store);
        match crate::recorder::LiveRecorder::start(&self.db_path(), &self.app.root.join("work"), id, asr, hint, lang, opts.denoise) {
            Ok(r) => {
                *rec = Some(r);
                Ok(id)
            }
            Err(e) => {
                let _ = self.store().delete_meeting(id);
                Err(e)
            }
        }
    }

    /// 音を足す。`pcm_b64` は 16kHz モノラル i16(リトルエンディアン)を Base64 にしたもの。
    pub fn record_push(&self, pcm_b64: &str) -> Result<crate::recorder::RecordStatus, String> {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD.decode(pcm_b64).map_err(|_| "音のデータが不正です".to_string())?;
        let x: Vec<f32> = bytes.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect();
        let mut rec = self.recorder.lock().unwrap_or_else(|p| p.into_inner());
        let r = rec.as_mut().ok_or("録音していません")?;
        // 無料版: 1件の上限と、累計の残りのうち短いほうまで(残りを超えて録音しても、その先は文字にならないため)
        let remaining = self.plan().remaining_ms;
        if let Some(lim) = self.ent().meeting_limit_ms {
            let elapsed = r.status().elapsed_ms;
            if elapsed >= lim {
                return Err("無料版で録音できるのは1件 15 分までです。「止めて保存」を押してください(有料版は制限なし)".into());
            }
            if remaining.is_some_and(|rem| elapsed >= rem) {
                return Err("無料版の文字起こしの残り時間まで録音しました。「止めて保存」を押してください(有料版は制限なし)".into());
            }
        }
        r.push(&self.store(), &x)
    }

    pub fn record_status(&self) -> Option<crate::recorder::RecordStatus> {
        self.recorder.lock().unwrap_or_else(|p| p.into_inner()).as_ref().map(|r| r.status())
    }

    /// 止める(残りの仮の文字が出るまで待ち、正確な文字起こしを待ちに入れる。画面はそのあと run_jobs を呼ぶ)。
    pub fn record_stop(&self) -> Result<DetailDto, String> {
        let r = self.recorder.lock().unwrap_or_else(|p| p.into_inner()).take().ok_or("録音していません")?;
        let id = r.meeting_id;
        let store = Store::open(&self.db_path()).map_err(err)?;
        if let Err(e) = r.stop(&store, &self.app.root.join("audio")) {
            let _ = store.set_recording(id, false);
            let _ = store.set_state(id, "failed", Some(&e));
            return Err(e);
        }
        self.detail(id)
    }

    /// 録音を捨てる(議事録ごと消す)。
    pub fn record_discard(&self) -> Result<(), String> {
        let r = self.recorder.lock().unwrap_or_else(|p| p.into_inner()).take().ok_or("録音していません")?;
        let store = Store::open(&self.db_path()).map_err(err)?;
        r.discard(&store)
    }

    // ---------------- 議事録 ----------------

    pub fn meetings(&self) -> Result<Vec<Meeting>, String> {
        self.store().meetings().map_err(err)
    }

    pub fn meetings_filtered(&self, f: &MeetingFilter) -> Result<Vec<Meeting>, String> {
        self.store().meetings_filtered(f).map_err(err)
    }

    pub fn all_tags(&self) -> Result<Vec<(String, i64)>, String> {
        self.store().all_tags().map_err(err)
    }

    pub fn set_tags(&self, id: i64, tags: &[String]) -> Result<DetailDto, String> {
        self.store().set_tags(id, tags).map_err(err)?;
        self.detail(id)
    }

    pub fn update_notes(&self, id: i64, agenda: &str, decisions: &str, todos: &[Todo]) -> Result<DetailDto, String> {
        self.store().update_notes(id, agenda, decisions, todos).map_err(err)?;
        self.detail(id)
    }

    /// 話者を判別し直す(人数を指定できる)。利用者が付けた名前も上書きする(元に戻せる)。
    /// 声の特徴が求めてあればそれを使い(すぐ終わる)、無ければ音声から求める(数十秒〜数分)。
    /// 時間のかかる処理は別の接続で行い、共有の `store` は握らない(他の操作を止めない)。
    pub fn rediarize(&self, id: i64, num_speakers: Option<i64>) -> Result<DetailDto, String> {
        self.require(self.ent().diarize)?;
        // 同時に1件だけ。文字起こし中の議事録は対象にしない
        {
            let mut g = self.diar.lock().unwrap_or_else(|p| p.into_inner());
            if g.is_some() {
                return Err("別の議事録の話者を判別している途中です。終わるまでお待ちください".into());
            }
            let busy_here = self.current.lock().unwrap_or_else(|p| p.into_inner()).0 == Some(id);
            let state = self.store().meeting(id).map_err(err)?.ok_or("見つかりません")?.state;
            if busy_here || state == "processing" || state == "queued" {
                return Err("文字起こし中のため、話者を判別し直せません。終わってからやり直してください".into());
            }
            self.diar_cancel.store(false, Ordering::SeqCst);
            *g = Some(RediarizeDto { meeting_id: id, done: 0, total: 0, phase: "prepare".into() });
        }
        let res = self.rediarize_inner(id, num_speakers);
        *self.diar.lock().unwrap_or_else(|p| p.into_inner()) = None;
        res?;
        self.detail(id)
    }

    fn rediarize_inner(&self, id: i64, num_speakers: Option<i64>) -> Result<(), String> {
        let have = !self.store().windows(id).map_err(err)?.is_empty();
        if have {
            pipeline::relabel(&self.store(), id, num_speakers, true)?;
        } else {
            let emb = self.embedder().ok_or_else(|| self.embedder_error().unwrap_or_else(|| "話者の判別のモデルがありません".into()))?;
            let (audio, _) = self.store().paths(id).map_err(err)?;
            let audio = audio.ok_or("音声を残していないため、話者を判別できません")?;
            let tmp = self.app.root.join("work").join(format!("diarize-{id}.pcm"));
            std::fs::create_dir_all(tmp.parent().unwrap()).map_err(err)?;
            let worker = Store::open(&self.db_path()).map_err(err)?;
            let r = crate::audio::decode_to_pcm16k(Path::new(&audio), &tmp).and_then(|_| {
                if self.diar_cancel.load(Ordering::SeqCst) {
                    return Err(pipeline::CANCELLED.to_string());
                }
                let on_progress = |done: usize, total: usize| {
                    *self.diar.lock().unwrap_or_else(|p| p.into_inner()) =
                        Some(RediarizeDto { meeting_id: id, done: done as i64, total: total as i64, phase: "embed".into() });
                };
                pipeline::diarize_meeting(&worker, emb.as_ref(), &tmp, id, num_speakers, true, Some(&self.diar_cancel), &on_progress)
            });
            let _ = std::fs::remove_file(&tmp);
            r?;
        }
        self.store().db.conn.execute("UPDATE meetings SET num_speakers=?2 WHERE id=?1", (id, num_speakers)).map_err(err)?;
        Ok(())
    }

    /// 話者の判別し直しの進み具合(していなければ None)。
    pub fn rediarize_status(&self) -> Option<RediarizeDto> {
        self.diar.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// 話者の判別し直しを中断する(いまのラベルはそのまま)。
    pub fn cancel_rediarize(&self) {
        self.diar_cancel.store(true, Ordering::SeqCst);
    }

    pub fn rename_speaker(&self, id: i64, old: &str, new: &str) -> Result<DetailDto, String> {
        if new.trim().is_empty() {
            return Err("名前を入力してください".into());
        }
        self.store().rename_speaker(id, old, new).map_err(err)?;
        self.detail(id)
    }

    pub fn find_in(&self, id: i64, query: &str) -> Result<Vec<i64>, String> {
        self.store().find_in(id, query).map_err(err)
    }

    pub fn replace_in(&self, id: i64, find: &str, replace: &str) -> Result<(usize, DetailDto), String> {
        let n = self.store().replace_in(id, find, replace).map_err(err)?;
        Ok((n, self.detail(id)?))
    }

    /// 設定(言語・範囲・話者など)を変えて、最初から文字起こしし直す。それまでの修正は消える(画面で確認してから呼ぶ)。
    pub fn reprocess(&self, id: i64, opts: &ProcessOptions) -> Result<(), String> {
        if self.current.lock().unwrap_or_else(|p| p.into_inner()).0 == Some(id) {
            return Err("処理中です。中断してからやり直してください".into());
        }
        let store = self.store();
        let (audio, pcm) = store.paths(id).map_err(err)?;
        if audio.is_none() {
            return Err("音声を残していないため、文字起こしし直せません".into());
        }
        store.set_options(id, opts).map_err(err)?;
        let tx = store.db.conn.unchecked_transaction().map_err(err)?;
        for sql in [
            "DELETE FROM segments_fts WHERE rowid IN (SELECT id FROM segments WHERE meeting_id=?1)",
            "DELETE FROM diar_windows WHERE meeting_id=?1",
            "DELETE FROM segments WHERE meeting_id=?1",
            "DELETE FROM segment_history WHERE meeting_id=?1",
            "DELETE FROM chunks WHERE meeting_id=?1",
            "DELETE FROM jobs WHERE json_extract(payload_json, '$.meeting_id')=?1",
            "UPDATE meetings SET state='queued', error=NULL, status='draft', pcm_path=NULL WHERE id=?1",
        ] {
            tx.execute(sql, [id]).map_err(err)?;
        }
        tx.commit().map_err(err)?;
        if let Some(p) = pcm {
            let _ = std::fs::remove_file(p);
        }
        pipeline::enqueue(&store, id).map(|_| ())
    }

    pub fn detail(&self, id: i64) -> Result<DetailDto, String> {
        let store = self.store();
        let meeting = store.meeting(id).map_err(err)?.ok_or("見つかりません")?;
        let segments = store.segments(id).map_err(err)?;
        let mut speakers: Vec<String> = Vec::new();
        for s in &segments {
            if !s.speaker.is_empty() && !speakers.contains(&s.speaker) {
                speakers.push(s.speaker.clone());
            }
        }
        let provisional = segments.iter().any(|s| s.chunk_idx < 0);
        Ok(DetailDto { can_undo: store.can_undo(id).map_err(err)?, meeting, segments, speakers, low_confidence: crate::store::LOW_CONFIDENCE, provisional })
    }

    /// 再生用の音声ファイルの場所(画面は asset プロトコルで読む)。残していなければ None。
    pub fn audio_path(&self, id: i64) -> Result<Option<String>, String> {
        Ok(self.store().paths(id).map_err(err)?.0)
    }

    pub fn update_meta(&self, id: i64, title: &str, held_on: Option<String>, participants: &str) -> Result<DetailDto, String> {
        let held = held_on.filter(|s| !s.trim().is_empty());
        self.store().update_meta(id, title, held.as_deref(), participants).map_err(err)?;
        self.detail(id)
    }

    pub fn edit_text(&self, segment_id: i64, text: &str) -> Result<DetailDto, String> {
        let mid = self.store().edit_text(segment_id, text).map_err(err)?;
        self.detail(mid)
    }

    pub fn set_speaker(&self, segment_id: i64, speaker: &str, following: bool) -> Result<DetailDto, String> {
        let mid = self.store().set_speaker(segment_id, speaker, following).map_err(err)?;
        self.detail(mid)
    }

    pub fn merge_next(&self, segment_id: i64) -> Result<DetailDto, String> {
        let mid = self.store().merge_next(segment_id).map_err(err)?;
        self.detail(mid)
    }

    pub fn split(&self, segment_id: i64, at: usize) -> Result<DetailDto, String> {
        let mid = self.store().split(segment_id, at).map_err(err)?;
        self.detail(mid)
    }

    pub fn revert_segment(&self, segment_id: i64) -> Result<DetailDto, String> {
        let mid = self.store().revert_segment(segment_id).map_err(err)?;
        self.detail(mid)
    }

    pub fn undo(&self, id: i64) -> Result<DetailDto, String> {
        self.store().undo(id).map_err(err)?;
        self.detail(id)
    }

    pub fn reapply_glossary(&self, id: i64) -> Result<(usize, DetailDto), String> {
        self.require(self.ent().glossary)?;
        let n = self.store().reapply_glossary(id).map_err(err)?;
        Ok((n, self.detail(id)?))
    }

    /// 確定する(利用者が確認画面で押したときだけ呼ぶ)。
    pub fn confirm(&self, id: i64) -> Result<DetailDto, String> {
        // 話者を判別し直している間に確定すると、終わったときのラベルの更新で下書きに戻ってしまう。終わってから確定してもらう
        if self.diar.lock().unwrap_or_else(|p| p.into_inner()).as_ref().map(|d| d.meeting_id) == Some(id) {
            return Err("話者を判別している途中です。終わってから確定してください".into());
        }
        self.store().confirm(id).map_err(err)?;
        self.detail(id)
    }

    pub fn unconfirm(&self, id: i64) -> Result<DetailDto, String> {
        self.store().unconfirm(id).map_err(err)?;
        self.detail(id)
    }

    pub fn delete_meeting(&self, id: i64) -> Result<(), String> {
        if self.current.lock().unwrap_or_else(|p| p.into_inner()).0 == Some(id) {
            return Err("処理中です。中断してから消してください".into());
        }
        for p in self.store().delete_meeting(id).map_err(err)? {
            let _ = std::fs::remove_file(p);
        }
        if let Ok(rd) = std::fs::read_dir(self.app.root.join("waveforms")) {
            for e in rd.flatten() {
                if e.file_name().to_string_lossy().starts_with(&format!("{id}-")) {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        Ok(())
    }

    /// 再生画面の波形(0〜1 の山の高さを `buckets` 個)。一度求めたらデータフォルダに残して使い回す。
    pub fn waveform(&self, id: i64, buckets: usize) -> Result<Vec<f32>, String> {
        let buckets = buckets.clamp(50, 2000);
        let (audio, _) = self.store().paths(id).map_err(err)?;
        let audio = audio.ok_or("音声を残していません")?;
        let dir = self.app.root.join("waveforms");
        let cache = dir.join(format!("{id}-{buckets}.json"));
        if let Ok(t) = std::fs::read_to_string(&cache) {
            if let Ok(v) = serde_json::from_str::<Vec<f32>>(&t) {
                return Ok(v);
            }
        }
        let tmp = self.app.root.join("work").join(format!("wave-{id}.pcm"));
        std::fs::create_dir_all(tmp.parent().unwrap()).map_err(err)?;
        let ms = crate::audio::decode_to_pcm16k(Path::new(&audio), &tmp)?;
        let rms = crate::audio::frame_rms_of(&tmp)?;
        let _ = std::fs::remove_file(&tmp);
        let _ = ms;
        let n = rms.len().max(1);
        let mut out = vec![0f32; buckets];
        for (i, v) in out.iter_mut().enumerate() {
            let (a, b) = (i * n / buckets, ((i + 1) * n / buckets).max(i * n / buckets + 1).min(n));
            *v = rms[a.min(n - 1)..b].iter().cloned().fold(0.0, f32::max);
        }
        let peak = out.iter().cloned().fold(1e-6, f32::max);
        for v in out.iter_mut() {
            *v = (*v / peak).sqrt(); // 小さい声も見えるように
        }
        std::fs::create_dir_all(&dir).map_err(err)?;
        let _ = std::fs::write(&cache, serde_json::to_string(&out).map_err(err)?);
        Ok(out)
    }

    // ---------------- 書き出し ----------------

    /// 確定した議事録だけを書き出す。
    pub fn export(&self, id: i64, format: &str, dest: &Path) -> Result<(), String> {
        let f = Format::parse(format).ok_or("対応していない形式です")?;
        let store = self.store();
        let m = store.meeting(id).map_err(err)?.ok_or("見つかりません")?;
        if m.status != "confirmed" {
            return Err("確定した議事録だけを書き出せます。確認画面で確定してください".into());
        }
        let ent = self.ent();
        self.require(ent.can_export(format))?;
        let mut body = render(f, &m, &store.segments(id).map_err(err)?)?;
        if ent.tier == Tier::Free {
            body.extend_from_slice(format!("\n{}\n", crate::plan::FREE_FOOTER).as_bytes());
        }
        std::fs::write(dest, body).map_err(|_| "書き出し先に保存できません".to_string())
    }

    pub fn export_name(&self, id: i64, format: &str) -> Result<String, String> {
        let f = Format::parse(format).ok_or("対応していない形式です")?;
        let m = self.store().meeting(id).map_err(err)?.ok_or("見つかりません")?;
        let safe: String = m.title.chars().map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c }).collect();
        Ok(format!("{}.{}", safe, f.ext()))
    }

    /// ノイズ除去後の音声(16kHz WAV)を書き出す。音声を残していない議事録はできない。
    pub fn export_denoised(&self, id: i64, dest: &Path) -> Result<(), String> {
        self.require(self.ent().can_export("wav"))?;
        let (audio, _) = self.store().paths(id).map_err(err)?;
        let audio = audio.ok_or("音声を残していないため書き出せません")?;
        let tmp = self.app.root.join("work").join(format!("export-{id}.pcm"));
        std::fs::create_dir_all(tmp.parent().unwrap()).map_err(err)?;
        let ms = crate::audio::decode_to_pcm16k(Path::new(&audio), &tmp)?;
        let mut out = Vec::new();
        // 長い録音でも一度に全部をメモリに載せないよう、5 分ずつ処理する
        let step = 300_000u64;
        let mut t = 0;
        while t < ms {
            let x = crate::audio::read_pcm16k(&tmp, t, (t + step).min(ms))?;
            out.extend(crate::audio::denoise_16k(&x));
            t += step;
        }
        let _ = std::fs::remove_file(&tmp);
        crate::audio::write_wav16(dest, &out)
    }

    // ---------------- 検索・用語辞書 ----------------

    pub fn search(&self, query: &str) -> Result<Vec<SearchHit>, String> {
        self.store().search(query).map_err(err)
    }

    pub fn glossary(&self) -> Result<Vec<GlossaryEntry>, String> {
        self.store().glossary().map_err(err)
    }

    pub fn export_glossary(&self, dest: &Path) -> Result<(), String> {
        let bytes = self.store().glossary_csv().map_err(err)?;
        std::fs::write(dest, bytes).map_err(|_| "書き出し先に保存できません".to_string())
    }

    /// CSV から用語辞書に取り込む。(取り込んだ数, 飛ばした行の理由, 取り込み後の一覧)
    pub fn import_glossary(&self, src: &Path) -> Result<(usize, Vec<String>, Vec<GlossaryEntry>), String> {
        self.require(self.ent().glossary)?;
        let bytes = std::fs::read(src).map_err(|_| "ファイルを読めません".to_string())?;
        if bytes.len() > 10 * 1024 * 1024 {
            return Err("ファイルが大きすぎます(10MBまで)".into());
        }
        let (n, skipped) = self.store().import_glossary_csv(&bytes).map_err(err)?;
        Ok((n, skipped, self.glossary()?))
    }

    pub fn add_glossary(&self, wrong: &str, right: &str) -> Result<Vec<GlossaryEntry>, String> {
        self.require(self.ent().glossary)?;
        self.store().add_glossary(wrong, right).map_err(err)?;
        self.glossary()
    }

    pub fn delete_glossary(&self, id: i64) -> Result<Vec<GlossaryEntry>, String> {
        self.store().delete_glossary(id).map_err(err)?;
        self.glossary()
    }

    // ---------------- 設定 ----------------

    pub fn settings(&self) -> SettingsDto {
        let s = self.app.load_settings();
        SettingsDto {
            update_check: s.update_check,
            keep_audio: flag(&s, "keep_audio", true),
            denoise_default: flag(&s, "denoise_default", true),
            consent_shown: flag(&s, "consent_shown", false),
            network: self.app.network_list(&network_entries(), UPDATE_PURPOSE),
            data_dir: self.app.root.to_string_lossy().into(),
            model: if self.ent().accurate_model {
                self.model_path().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).unwrap_or_else(|| "none".into())
            } else {
                self.live_models.lock().unwrap_or_else(|p| p.into_inner()).iter().find(|p| p.exists())
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).unwrap_or_else(|| "none".into())
            },
            diarize_available: self.embedder().is_some(),
            diarize_error: self.embedder_error(),
        }
    }

    /// key: update_check | keep_audio | denoise_default | consent_shown
    pub fn set_flag(&self, key: &str, on: bool) -> Result<SettingsDto, String> {
        let mut s = self.app.load_settings();
        match key {
            "update_check" => s.update_check = on,
            "keep_audio" | "denoise_default" | "consent_shown" => {
                s.extra.insert(key.into(), serde_json::Value::Bool(on));
            }
            _ => return Err("不明な設定です".into()),
        }
        self.app.save_settings(&s).map_err(err)?;
        Ok(self.settings())
    }

    /// 全データ(音声のコピー・議事録・用語辞書・設定)を消す。処理中は消さない。
    pub fn delete_all(&self) -> Result<usize, String> {
        if self.busy.load(Ordering::SeqCst) {
            return Err("処理中です。中断してから消してください".into());
        }
        if self.model_dl.lock().unwrap_or_else(|p| p.into_inner()).0 {
            return Err("モデルの取得中です。中断してから消してください".into());
        }
        let mut store = self.store();
        *store = Store::open_in_memory().map_err(err)?;
        // 取得したモデルもデータフォルダの中にあるので、一緒に消える
        if self.model_source().map(|(_, s)| s == "managed").unwrap_or(true) {
            *self.asr.lock().unwrap_or_else(|p| p.into_inner()) = None;
        }
        let n = self.app.delete_all().map_err(err)?;
        *store = Store::open(&self.db_path()).map_err(err)?;
        // 無料版の使った量は消さない(データフォルダの中の記録も書き戻す)
        self.ledger.lock().unwrap_or_else(|p| p.into_inner()).add(0, false);
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::FakeAsr;

    fn tmp(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("min-cmd-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn state(d: &Path, asr: Option<Box<dyn Asr>>) -> AppState {
        let loader: AsrLoader = Box::new(|p: &Path| Ok(Box::new(FakeAsr { text: format!("loaded {}", p.display()) }) as Box<dyn Asr>));
        AppState::new(d.to_path_buf(), asr, loader, vec![]).unwrap()
    }

    /// 決まった要約を返す偽のエンジン
    struct FakeSummarizer;
    impl crate::summary::Summarizer for FakeSummarizer {
        fn count_tokens(&self, text: &str) -> Result<usize, crate::summary::SummaryError> {
            Ok(text.chars().count())
        }
        fn complete(&self, _s: &str, user: &str, _m: u32, _g: Option<&str>, _t: &mut dyn FnMut(u32)) -> Result<crate::summary::Completion, crate::summary::SummaryError> {
            assert!(user.contains("山田商事の件です"), "文字起こしが渡る");
            Ok(crate::summary::Completion {
                text: r#"{"summary":["山田商事の件"],"decisions":[],"todos":[{"text":"連絡する","owner":"","due":"明日"}]}"#.into(),
                ..Default::default()
            })
        }
    }

    #[test]
    fn 要約は有料版だけ_モデルを取得してから使え_議事録は自動では変わらない() {
        let d = tmp("summary");
        let s = state(&d, Some(Box::new(FakeAsr { text: "山田商事の件です".into() })));
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset/t05_overlap.wav");
        let id = s.import_audio(&src, &ProcessOptions { denoise: false, ..Default::default() }).unwrap();
        s.run_jobs().unwrap();

        // 無料版: 取得も要約もできない(状態は見られる)
        s.set_tier(Tier::Free);
        assert_eq!(s.download_summary_model().unwrap_err(), PRO_ONLY);
        assert_eq!(s.summarize(id).unwrap_err(), PRO_ONLY);
        assert!(!s.summary_status().status.installed);

        // 有料版: モデルが無ければ案内。エンジンが無いビルドでもその旨
        s.set_tier(Tier::Pro);
        assert!(s.summarize(id).unwrap_err().contains("要約のモデルがありません"));
        let dir = d.join("models");
        std::fs::create_dir_all(&dir).unwrap();
        let f = std::fs::File::create(dir.join(crate::summary::SUMMARY_MODEL.file_name)).unwrap();
        f.set_len(crate::summary::SUMMARY_MODEL.size).unwrap();
        std::fs::write(dir.join(format!("{}.sha256", crate::summary::SUMMARY_MODEL.file_name)), crate::summary::SUMMARY_MODEL.sha256).unwrap();
        let st = s.summary_status();
        assert_eq!((st.source, st.status.installed, st.license), ("managed", true, "Apache-2.0"));
        assert!(s.summarize(id).unwrap_err().contains("エンジンが含まれていません"));

        // 偽のエンジンで要約: 下書きを返すだけで、議事録(議題・決定事項・ToDo)は変えない
        s.summary.set_factory(Box::new(|_| Ok(std::sync::Arc::new(FakeSummarizer))));
        let r = s.summarize(id).unwrap();
        assert_eq!(r.draft.summary, vec!["山田商事の件"]);
        assert_eq!(r.draft.todos[0].due, "明日");
        let m = s.detail(id).unwrap().meeting;
        assert_eq!((m.agenda.as_str(), m.decisions.as_str(), m.todos.len()), ("", "", 0));
        assert!(!s.summary_status().running);

        // 文字起こしが終わっていなければ要約しない
        let id2 = s.import_audio(&src, &ProcessOptions { denoise: false, ..Default::default() }).unwrap();
        assert!(s.summarize(id2).unwrap_err().contains("文字起こしが終わってから"));

        // 取り込みは、利用者が確認したあとの update_notes(従来の経路)だけ
        let det = s.update_notes(id, "山田商事の件", "", &[Todo { text: "連絡する(期限: 明日)".into(), ..Default::default() }]).unwrap();
        assert_eq!(det.meeting.todos.len(), 1);
        // 取得済みのモデルの削除(無料版でもできる)
        s.set_tier(Tier::Free);
        assert!(!s.delete_summary_model().unwrap().status.installed);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn モデルが無ければ処理は待ちのまま_取得済みなら初回に読み込む() {
        let d = tmp("model");
        let s = state(&d, None);
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset/t05_overlap.wav");
        let id = s.import_audio(&src, &ProcessOptions { denoise: false, ..Default::default() }).unwrap();
        let st = s.model_status();
        assert_eq!((st.source, st.status.installed, st.status.size), ("none", false, WHISPER_MODEL.size));
        assert!(s.run_jobs().unwrap_err().contains("モデルがありません"));
        assert_eq!(s.progress().unwrap().pending, 1);
        assert_eq!(s.meetings().unwrap()[0].state, "queued");
        // 取得済みの状態を作る(中身は照合の印で判定する)
        let dir = d.join("models");
        std::fs::create_dir_all(&dir).unwrap();
        let f = std::fs::File::create(dir.join(WHISPER_MODEL.file_name)).unwrap();
        f.set_len(WHISPER_MODEL.size).unwrap();
        std::fs::write(dir.join(format!("{}.sha256", WHISPER_MODEL.file_name)), WHISPER_MODEL.sha256).unwrap();
        assert_eq!(s.model_status().source, "managed");
        assert_eq!(s.run_jobs().unwrap(), 1);
        assert!(s.detail(id).unwrap().segments[0].text.starts_with("loaded "));
        assert_eq!(s.settings().model, WHISPER_MODEL.file_name);
        assert!(!s.delete_model().unwrap().status.installed);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 取り込み_処理_修正_確定_書き出し_検索_全削除まで通る() {
        let d = tmp("flow");
        let s = state(&d, Some(Box::new(FakeAsr { text: "やまだ商事の件です".into() })));
        assert!(s.import_audio(Path::new("/nope/x.txt"), &ProcessOptions::default()).is_err());
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset/t05_overlap.wav");
        let id = s.import_audio(&src, &ProcessOptions { denoise: false, ..Default::default() }).unwrap();
        assert!(src.exists());
        assert_eq!(s.meetings().unwrap()[0].state, "queued");
        s.add_glossary("やまだ商事", "山田商事").unwrap();
        assert_eq!(s.run_jobs().unwrap(), 1);
        let det = s.detail(id).unwrap();
        assert_eq!(det.meeting.state, "done");
        assert!(det.segments.iter().all(|x| x.text == "山田商事の件です"));
        assert!(s.audio_path(id).unwrap().is_some()); // 既定は音声を残す

        // 確定前は書き出せない
        let out = d.join("out.md");
        assert!(s.export(id, "md", &out).is_err());
        let sid = det.segments[0].id;
        s.set_speaker(sid, "佐藤", false).unwrap();
        s.edit_text(sid, "山田商事の件です。").unwrap();
        s.update_meta(id, "定例会議", Some("2026-10-01".into()), "佐藤、鈴木").unwrap();
        s.confirm(id).unwrap();
        s.export(id, "md", &out).unwrap();
        let md = std::fs::read_to_string(&out).unwrap();
        assert!(md.starts_with("# 定例会議") && md.contains("**佐藤**"));
        assert_eq!(s.export_name(id, "srt").unwrap(), "定例会議.srt");
        let wf = s.waveform(id, 100).unwrap();
        assert_eq!(wf.len(), 100);
        assert!(wf.iter().all(|v| (0.0..=1.0).contains(v)) && wf.iter().any(|v| *v > 0.9));
        assert_eq!(s.waveform(id, 100).unwrap(), wf, "2回目は保存したものを使う");
        let wav = d.join("dn.wav");
        s.export_denoised(id, &wav).unwrap();
        assert!(std::fs::metadata(&wav).unwrap().len() > 1_000_000);
        assert!(!s.search("山田商事").unwrap().is_empty());

        std::fs::remove_file(&out).ok();
        std::fs::remove_file(&wav).ok();
        s.delete_all().unwrap();
        assert!(s.meetings().unwrap().is_empty());
        assert!(s.glossary().unwrap().is_empty());
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 同梱のモデルがあればそれを使う() {
        let d = tmp("bundled");
        let res = d.join("res");
        std::fs::create_dir_all(&res).unwrap();
        let bundled = res.join(WHISPER_MODEL.file_name);
        let loader: AsrLoader = Box::new(|p: &Path| Ok(Box::new(FakeAsr { text: format!("loaded {}", p.display()) }) as Box<dyn Asr>));
        let s = AppState::new(d.join("data"), None, loader, vec![(d.join("none.bin"), "env"), (bundled.clone(), "bundled")]).unwrap();
        assert_eq!(s.model_status().source, "none");
        std::fs::write(&bundled, b"x").unwrap();
        let st = s.model_status();
        assert_eq!((st.source, st.status.installed), ("bundled", true));
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset/t05_overlap.wav");
        let id = s.import_audio(&src, &ProcessOptions { denoise: false, ..Default::default() }).unwrap();
        s.run_jobs().unwrap();
        assert!(s.detail(id).unwrap().segments[0].text.contains("res"));
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 無料版は書き出しと有料機能が制限され_使った量は全削除でも戻らない() {
        let d = tmp("free");
        let s = state(&d, Some(Box::new(FakeAsr { text: "テスト".into() })));
        s.set_tier(Tier::Free);
        // 無料版は小さなモデル(標準)を使う。ここでは仮のファイルを置き、読み込みはテスト用の loader が行う
        std::fs::create_dir_all(&d).unwrap();
        let small = d.join("small.bin");
        std::fs::write(&small, b"x").unwrap();
        s.set_live_models(vec![small.clone()]);
        assert_eq!(s.settings().model, "small.bin", "無料版の表示は小さなモデル");
        assert!(s.add_glossary("a", "b").unwrap_err().contains("有料版"));
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset/t05_overlap.wav");
        let id = s.import_audio(&src, &ProcessOptions::default()).unwrap();
        let m = s.detail(id).unwrap().meeting;
        assert!(!m.denoise && !m.diarize, "無料版では外れる");
        // 無料版は小さなモデルを使う(ここでは候補が無いので正確なモデルの読み込みに落ちる → テストでは渡した FakeAsr)
        s.run_jobs().unwrap();
        let used = s.plan().usage.used_ms;
        assert!(used > 50_000 && s.plan().remaining_ms == Some(crate::plan::FREE_TOTAL_MS - used));
        s.confirm(id).unwrap();
        let out = d.join("a.txt");
        s.export(id, "txt", &out).unwrap();
        assert!(std::fs::read_to_string(&out).unwrap().contains("無料版で作成"));
        assert!(s.export(id, "docx", &d.join("a.docx")).unwrap_err().contains("有料版"));
        assert!(s.rediarize(id, Some(2)).unwrap_err().contains("有料版"));
        std::fs::remove_file(&out).ok();
        s.delete_all().unwrap();
        assert_eq!(s.plan().usage.used_ms, used, "全削除でも戻らない");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 無料版は小さなモデルが無くても正確なモデルに代えない() {
        let d = tmp("nofallback");
        let s = state(&d, None); // 文字起こしのエンジンは、モデルの場所から読み込む
        // 正確なモデルは「ある」状態にする
        let dir = d.join("models");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::File::create(dir.join(WHISPER_MODEL.file_name)).unwrap().set_len(WHISPER_MODEL.size).unwrap();
        std::fs::write(dir.join(format!("{}.sha256", WHISPER_MODEL.file_name)), WHISPER_MODEL.sha256).unwrap();
        s.set_tier(Tier::Free);
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset/t05_overlap.wav");
        s.import_audio(&src, &ProcessOptions::default()).unwrap();
        assert!(s.run_jobs().unwrap_err().contains("標準"));
        // 有料版なら正確なモデルで動く
        s.set_tier(Tier::Pro);
        assert_eq!(s.run_jobs().unwrap(), 1);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 設定のフラグを保存できる() {
        let d = tmp("set");
        let s = state(&d, Some(Box::new(FakeAsr { text: String::new() })));
        assert!(s.settings().keep_audio && !s.settings().consent_shown);
        assert!(!s.set_flag("keep_audio", false).unwrap().keep_audio);
        assert!(s.set_flag("consent_shown", true).unwrap().consent_shown);
        let st = s.set_flag("update_check", false).unwrap();
        assert!(st.network.iter().any(|e| e.stoppable && !e.enabled));
        assert!(s.set_flag("x", true).is_err());
        std::fs::remove_dir_all(&d).ok();
    }

    /// 音声から特徴を求める間は遅い(テスト用)
    struct SlowEmb(u64);
    impl Embedder for SlowEmb {
        fn embed(&self, pcm: &[f32]) -> Result<Vec<f32>, String> {
            std::thread::sleep(std::time::Duration::from_millis(self.0));
            let r = (pcm.iter().map(|x| x * x).sum::<f32>() / pcm.len().max(1) as f32).sqrt();
            Ok(crate::diarize::normalize(vec![r, 0.05]))
        }
    }

    fn processed_without_windows(name: &str, ms: u64) -> (PathBuf, AppState, i64) {
        let d = tmp(name);
        let s = state(&d, Some(Box::new(FakeAsr { text: "テスト".into() })));
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset/t05_overlap.wav");
        let id = s.import_audio(&src, &ProcessOptions { denoise: false, ..Default::default() }).unwrap();
        s.run_jobs().unwrap(); // 特徴のモデルを設定していないので、窓は無い
        assert!(s.store().windows(id).unwrap().is_empty());
        s.set_embedder_loader(Box::new(move || Ok(Box::new(SlowEmb(ms)) as Box<dyn Embedder>)));
        (d, s, id)
    }

    #[test]
    fn 話者の判別し直しは他の操作を止めず_進み具合が見え_二重に始められない() {
        let (d, s, id) = processed_without_windows("rediar-nb", 40);
        std::thread::scope(|sc| {
            let h = sc.spawn(|| s.rediarize(id, Some(2)));
            // 始まるまで待つ
            let t0 = std::time::Instant::now();
            while s.rediarize_status().map(|p| p.total).unwrap_or(0) == 0 {
                assert!(t0.elapsed().as_secs() < 20 && !h.is_finished(), "始まらない");
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            let t = std::time::Instant::now();
            s.detail(id).unwrap();
            s.progress().unwrap();
            s.meetings().unwrap();
            assert!(t.elapsed().as_millis() < 300, "他の操作が待たされた: {:?}", t.elapsed());
            assert!(s.rediarize(id, None).unwrap_err().contains("途中"));
            assert!(!h.is_finished(), "テストが速すぎる");
            let r = h.join().unwrap();
            assert!(r.is_ok(), "{r:?}");
        });
        assert!(s.rediarize_status().is_none());
        assert!(!s.store().windows(id).unwrap().is_empty());
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 話者の判別し直しを中断すると_ラベルも窓もそのまま() {
        let (d, s, id) = processed_without_windows("rediar-cancel", 40);
        let before = s.detail(id).unwrap().segments.iter().map(|x| x.speaker.clone()).collect::<Vec<_>>();
        std::thread::scope(|sc| {
            let h = sc.spawn(|| s.rediarize(id, Some(2)));
            let t0 = std::time::Instant::now();
            while s.rediarize_status().map(|p| p.done).unwrap_or(0) < 2 {
                assert!(t0.elapsed().as_secs() < 20 && !h.is_finished(), "進まない");
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            s.cancel_rediarize();
            let e = h.join().unwrap().unwrap_err();
            assert!(e.contains("中断"), "{e}");
        });
        assert!(s.rediarize_status().is_none());
        assert!(s.store().windows(id).unwrap().is_empty());
        let after = s.detail(id).unwrap().segments.iter().map(|x| x.speaker.clone()).collect::<Vec<_>>();
        assert_eq!(before, after);
        std::fs::remove_dir_all(&d).ok();
    }

    /// 時間内に終わらなければ(デッドロックの疑い)テストを失敗させる
    fn within<T: Send>(secs: u64, what: &str, f: impl FnOnce() -> T + Send) -> T {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::scope(|sc| {
            sc.spawn(move || {
                let _ = tx.send(f());
            });
            rx.recv_timeout(std::time::Duration::from_secs(secs)).unwrap_or_else(|_| panic!("{what}: {secs}秒で終わらない(デッドロックの疑い)"))
        })
    }

    #[test]
    fn 判別し直しの最中に文の編集_分割_結合_元に戻すを重ねても止まらず_結果が壊れない() {
        let (d, s, id) = processed_without_windows("rediar-edit", 25);
        let n0 = s.detail(id).unwrap().segments.len();
        assert!(n0 >= 2, "文が少なすぎる: {n0}");
        let r = within(120, "判別し直しと編集", || {
            std::thread::scope(|sc| {
                let h = sc.spawn(|| s.rediarize(id, Some(2)));
                let t0 = std::time::Instant::now();
                while s.rediarize_status().map(|p| p.total).unwrap_or(0) == 0 {
                    assert!(t0.elapsed().as_secs() < 20 && !h.is_finished(), "始まらない");
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                // 判別している間、画面からの操作を次々に送る(共有の接続は握られていない)
                let mut ops = 0;
                while !h.is_finished() {
                    let segs = s.detail(id).unwrap().segments;
                    let first = segs[0].id;
                    s.edit_text(first, &format!("編集{ops}")).unwrap();
                    s.set_speaker(first, "手で付けた名前", false).unwrap();
                    if segs.len() >= 2 {
                        s.split(segs[1].id, 1).ok();
                        let segs = s.detail(id).unwrap().segments;
                        s.merge_next(segs[1].id).ok();
                    }
                    s.undo(id).unwrap();
                    // 確定は、判別が終わるまで断られる
                    assert!(s.confirm(id).unwrap_err().contains("途中"));
                    ops += 1;
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                assert!(ops >= 2, "重ねた操作が少ない: {ops}");
                h.join().unwrap()
            })
        });
        assert!(r.is_ok(), "{r:?}");
        let det = s.detail(id).unwrap();
        // 文の id が重ならず、時刻が前後せず、文字が残っている
        let mut ids: Vec<i64> = det.segments.iter().map(|x| x.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), det.segments.len());
        assert!(det.segments.windows(2).all(|w| w[0].start_ms <= w[1].start_ms));
        assert!(det.segments.iter().all(|x| !x.text.is_empty()));
        assert!(s.rediarize_status().is_none());
        // 終われば確定でき、確定のあとは書き出せる
        let c = s.confirm(id).unwrap();
        assert_eq!(c.meeting.status, "confirmed");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 判別し直しを中断したあとは_すぐやり直せて_確定もできる() {
        let (d, s, id) = processed_without_windows("rediar-cancel2", 25);
        // 何も動いていないときの中断は、次の判別を止めない
        s.cancel_rediarize();
        std::thread::scope(|sc| {
            let h = sc.spawn(|| s.rediarize(id, Some(2)));
            let t0 = std::time::Instant::now();
            while s.rediarize_status().map(|p| p.done).unwrap_or(0) < 2 {
                assert!(t0.elapsed().as_secs() < 20 && !h.is_finished(), "進まない");
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            s.cancel_rediarize();
            s.cancel_rediarize(); // 続けて押しても同じ
            assert!(h.join().unwrap().unwrap_err().contains("中断"));
        });
        assert!(s.rediarize_status().is_none());
        assert_eq!(s.confirm(id).unwrap().meeting.status, "confirmed");
        // 中断のあとにやり直すと最後まで終わる
        let r = within(60, "中断後のやり直し", || s.rediarize(id, Some(2)));
        assert!(r.is_ok(), "{r:?}");
        assert!(!s.store().windows(id).unwrap().is_empty());
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 保存と確定が同時に来ても止まらず_最後は確定できる() {
        let d = tmp("save-confirm");
        let s = state(&d, Some(Box::new(FakeAsr { text: "テスト".into() })));
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset/t05_overlap.wav");
        let id = s.import_audio(&src, &ProcessOptions { denoise: false, ..Default::default() }).unwrap();
        s.run_jobs().unwrap();
        let first = s.detail(id).unwrap().segments[0].id;
        within(60, "保存と確定", || {
            std::thread::scope(|sc| {
                let a = sc.spawn(|| (0..60).for_each(|i| { s.edit_text(first, &format!("直し{i}")).unwrap(); }));
                let b = sc.spawn(|| (0..60).for_each(|_| { s.confirm(id).unwrap(); s.unconfirm(id).unwrap(); }));
                a.join().unwrap();
                b.join().unwrap();
            })
        });
        let det = s.detail(id).unwrap();
        assert_eq!(det.segments[0].text, "直し59");
        // 直したあとに確定すれば、確定のまま残る(編集のたびに下書きに戻るのは、確定したあとに直したときだけ)
        assert_eq!(s.confirm(id).unwrap().meeting.status, "confirmed");
        assert_eq!(s.detail(id).unwrap().meeting.status, "confirmed");
        std::fs::remove_dir_all(&d).ok();
    }

    /// 無料版の状態で、小さなモデル(仮の loader)を設定した state
    fn free_state(name: &str) -> (PathBuf, AppState) {
        let d = tmp(name);
        let s = state(&d, Some(Box::new(FakeAsr { text: "テスト".into() })));
        s.set_tier(Tier::Free);
        std::fs::create_dir_all(&d).unwrap();
        let small = d.join("small.bin");
        std::fs::write(&small, b"x").unwrap();
        s.set_live_models(vec![small]);
        (d, s)
    }

    fn silence_b64(ms: usize) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(vec![0u8; ms * 16 * 2])
    }

    #[test]
    fn 無料版の録音は_1件の上限と累計の残りのうち短いほうで止まる() {
        // 累計の残りが 20 秒しかないとき
        let (d, s) = free_state("free-rec-rem");
        s.ledger.lock().unwrap().add(crate::plan::FREE_TOTAL_MS - 20_000, true);
        assert_eq!(s.plan().remaining_ms, Some(20_000));
        s.record_start(&ProcessOptions::default()).unwrap();
        let mut stopped_at = 0;
        for i in 1..=40 {
            match s.record_push(&silence_b64(1_000)) {
                Ok(_) => {}
                Err(e) => {
                    assert!(e.contains("残り時間"), "{e}");
                    stopped_at = i;
                    break;
                }
            }
        }
        assert!((20..=22).contains(&stopped_at), "残りの 20 秒で止まるはず: {stopped_at}");
        s.record_discard().unwrap();
        std::fs::remove_dir_all(&d).ok();

        // 残りが十分あれば、1件 15 分で止まる
        let (d, s) = free_state("free-rec-15");
        s.record_start(&ProcessOptions::default()).unwrap();
        let mut n = 0;
        let chunk = silence_b64(10_000);
        let e = loop {
            match s.record_push(&chunk) {
                Ok(_) => n += 1,
                Err(e) => break e,
            }
            assert!(n <= 100, "止まらない");
        };
        assert!(e.contains("15 分"), "{e}");
        assert!((90..=91).contains(&n), "15 分(10 秒 x 90)で止まるはず: {n}");
        s.record_discard().unwrap();
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 無料版の記録が書き換えられていたら_使い切った扱いで取り込みも録音も断る() {
        let (d, s) = free_state("free-tamper");
        s.ledger.lock().unwrap().add(5 * 60_000, true);
        // 記録の数値だけ書き換える(署名が合わなくなる)
        let f = d.join("usage.dat");
        let v = std::fs::read_to_string(&f).unwrap();
        std::fs::write(&f, v.replace("300000", "0")).unwrap();
        s.set_ledger(crate::plan::Ledger::new(vec![Box::new(crate::plan::FileSlot(f.clone()))]));
        assert!(s.plan().usage.tampered);
        assert_eq!(s.plan().remaining_ms, Some(0));
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset/t05_overlap.wav");
        assert_eq!(s.import_audio(&src, &ProcessOptions::default()).unwrap_err(), crate::pipeline::LIMIT_REACHED);
        assert_eq!(s.record_start(&ProcessOptions::default()).unwrap_err(), crate::pipeline::LIMIT_REACHED);
        // 全削除をしても、書き換えた記録は直らない
        s.delete_all().unwrap();
        assert_eq!(s.plan().remaining_ms, Some(0));
        std::fs::remove_dir_all(&d).ok();
    }
}
