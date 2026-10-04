//! 文字起こしエンジン。区間(最長 30 秒、16kHz モノラル f32)を受け取り、時刻つきの文を返す。
//! 本番は whisper.cpp(whisper-rs、feature `whisper`)。テストは `FakeAsr`。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct AsrSegment {
    /// 区間の先頭からの時刻(ミリ秒)
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    /// 0〜1。低いものは確認画面で印を付ける
    pub confidence: f32,
}

pub trait Asr: Send + Sync {
    /// `hint` は用語辞書の正しい表記など、認識のヒント(空でもよい)。`language` は "ja" | "en" | "auto"。
    /// `cancel` が立ったら途中で止めて Err を返す。
    fn transcribe(&self, pcm: &[f32], hint: &str, language: &str, cancel: &Arc<AtomicBool>) -> Result<Vec<AsrSegment>, String>;
    fn name(&self) -> String;
}

/// テスト用。区間ごとに決まった文を返す。
pub struct FakeAsr {
    pub text: String,
}

impl Asr for FakeAsr {
    fn transcribe(&self, pcm: &[f32], _hint: &str, _language: &str, cancel: &Arc<AtomicBool>) -> Result<Vec<AsrSegment>, String> {
        if cancel.load(Ordering::SeqCst) {
            return Err("中断しました".into());
        }
        let ms = pcm.len() as u64 * 1000 / crate::audio::RATE as u64;
        Ok(vec![AsrSegment { start_ms: 0, end_ms: ms, text: self.text.clone(), confidence: 0.9 }])
    }
    fn name(&self) -> String {
        "fake".into()
    }
}

/// モデルが無いときに使う。文字起こしを頼まれたら、理由を返して失敗する。
pub struct MissingAsr {
    pub reason: String,
}

impl Asr for MissingAsr {
    fn transcribe(&self, _: &[f32], _: &str, _: &str, _: &Arc<AtomicBool>) -> Result<Vec<AsrSegment>, String> {
        Err(self.reason.clone())
    }
    fn name(&self) -> String {
        "none".into()
    }
}

#[cfg(feature = "whisper")]
pub use whisper::WhisperAsr;

#[cfg(feature = "whisper")]
mod whisper {
    use super::*;
    use std::sync::Mutex;
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

    /// whisper.cpp の ggml 形式のモデル(例: ggml-large-v3-turbo-q5_0.bin)を読む。
    pub struct WhisperAsr {
        // WhisperState は内部にバッファを持つので、1つを使い回す(同時に1区間だけ処理する)
        state: Mutex<WhisperState>,
        threads: i32,
        model_name: String,
    }

    impl WhisperAsr {
        pub fn new(model: &std::path::Path, threads: i32) -> Result<Self, String> {
            let mut p = WhisperContextParameters::default();
            // GPU は使わない(最低ラインの機械に合わせ、結果を揃える)
            p.use_gpu(false);
            let path = model.to_str().ok_or("モデルの場所に使えない文字が含まれています")?;
            let ctx = WhisperContext::new_with_params(path, p).map_err(|e| format!("文字起こしのモデルを読めません({e})"))?;
            let state = ctx.create_state().map_err(|e| format!("文字起こしの準備に失敗しました({e})"))?;
            let model_name = model.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            Ok(Self { state: Mutex::new(state), threads, model_name })
        }
    }

    impl Asr for WhisperAsr {
        fn transcribe(&self, pcm: &[f32], hint: &str, language: &str, cancel: &Arc<AtomicBool>) -> Result<Vec<AsrSegment>, String> {
            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            // "auto" は区間ごとに言語を推定する(英語と日本語が混ざる会議向け)
            params.set_language(Some(match language {
                "en" => "en",
                "auto" => "auto",
                _ => "ja",
            }));
            params.set_n_threads(self.threads);
            params.set_no_context(true);
            params.set_suppress_blank(true);
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            if !hint.is_empty() {
                params.set_initial_prompt(hint);
            }
            // 中断: whisper-rs 0.15.1 の set_abort_callback_safe は、クロージャのポインタの型を取り違えていて
            // 常に「中断」と読まれる(実測: エラーコード -6)。そのため生の関数と AtomicBool のポインタを渡す。
            // `cancel` は state.full() が戻るまで生きている(この関数の引数)。
            unsafe extern "C" fn should_abort(data: *mut std::ffi::c_void) -> bool {
                (*(data as *const AtomicBool)).load(Ordering::SeqCst)
            }
            unsafe {
                params.set_abort_callback(Some(should_abort));
                params.set_abort_callback_user_data(Arc::as_ptr(cancel) as *mut std::ffi::c_void);
            }
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.full(params, pcm).map_err(|e| {
                if cancel.load(Ordering::SeqCst) { "中断しました".to_string() } else { format!("文字起こしに失敗しました({e})") }
            })?;
            let mut out = Vec::new();
            for seg in state.as_iter() {
                let text = seg.to_str_lossy().map(|s| s.trim().to_string()).unwrap_or_default();
                // 無音を文字にしてしまう(幻聴)のを抑える
                if text.is_empty() || seg.no_speech_probability() > 0.6 {
                    continue;
                }
                let n = seg.n_tokens();
                let mut sum = 0.0f32;
                let mut k = 0;
                for i in 0..n {
                    if let Some(t) = seg.get_token(i) {
                        // 特殊トークン([_BEG_] など)は除く
                        if t.to_str().map(|s| s.starts_with("[_")).unwrap_or(true) {
                            continue;
                        }
                        sum += t.token_probability();
                        k += 1;
                    }
                }
                out.push(AsrSegment {
                    start_ms: (seg.start_timestamp().max(0) as u64) * 10,
                    end_ms: (seg.end_timestamp().max(0) as u64) * 10,
                    text,
                    confidence: if k > 0 { sum / k as f32 } else { 0.0 },
                });
            }
            Ok(out)
        }
        fn name(&self) -> String {
            self.model_name.clone()
        }
    }
}
