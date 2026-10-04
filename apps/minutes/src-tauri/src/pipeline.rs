//! 1つの議事録の処理: 読み込み(16kHz へ変換)→ 区間に分割 → 区間ごとに(ノイズ除去 →)文字起こし → 保存。
//! 区間ごとに保存するので、途中で閉じても続きの区間から再開できる(`core::jobs` のジョブ1件 = 議事録1件)。

use crate::asr::Asr;
use crate::audio;
use crate::store::Store;
use factory_core::jobs::{CancelToken, Jobs};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const JOB_KIND: &str = "transcribe";
pub const CANCELLED: &str = "中断しました";

pub fn enqueue(store: &Store, meeting_id: i64) -> Result<i64, String> {
    Jobs::new(&store.db).enqueue(JOB_KIND, &format!("{{\"meeting_id\":{meeting_id}}}")).map_err(|e| e.to_string())
}

pub struct Options {
    /// 処理が終わったら音声のコピーを消す(文字だけ残す設定)
    pub keep_audio: bool,
}

/// 議事録1件を処理する。`on_chunk(処理済み, 全体)` は区間が1つ終わるごとに呼ばれる。
pub fn process_meeting(
    store: &Store,
    asr: &dyn Asr,
    work_dir: &Path,
    meeting_id: i64,
    opts: &Options,
    cancel: &Arc<AtomicBool>,
    mut on_chunk: impl FnMut(i64, i64),
) -> Result<(), String> {
    let m = store.meeting(meeting_id).map_err(|e| e.to_string())?.ok_or("議事録が見つかりません")?;
    store.set_state(meeting_id, "processing", None).map_err(|e| e.to_string())?;
    let res = (|| -> Result<(), String> {
        let (audio_path, pcm_path) = store.paths(meeting_id).map_err(|e| e.to_string())?;
        // 1) 16kHz モノラルの作業ファイル(前回の途中から再開するときは作り直さない)
        let pcm = match pcm_path.filter(|p| Path::new(p).exists()) {
            Some(p) if store.has_chunks(meeting_id).map_err(|e| e.to_string())? => p.into(),
            _ => {
                let src = audio_path.ok_or("音声のコピーがありません")?;
                std::fs::create_dir_all(work_dir).map_err(|_| "作業フォルダを作れません".to_string())?;
                let p = work_dir.join(format!("meeting-{meeting_id}.pcm"));
                let ms = audio::decode_to_pcm16k(Path::new(&src), &p)?;
                store.set_pcm(meeting_id, Some(&p.to_string_lossy()), Some(ms as i64)).map_err(|e| e.to_string())?;
                let rms = audio::frame_rms_of(&p)?;
                store.set_chunks(meeting_id, &audio::split_chunks(&rms)).map_err(|e| e.to_string())?;
                p
            }
        };
        // 2) 区間ごとに
        let hint = store.glossary_hint().map_err(|e| e.to_string())?;
        for (idx, start, end, silent) in store.pending_chunks(meeting_id).map_err(|e| e.to_string())? {
            if cancel.load(Ordering::SeqCst) {
                return Err(CANCELLED.into());
            }
            let segs = if silent {
                vec![]
            } else {
                let mut x = audio::read_pcm16k(&pcm, start as u64, end as u64)?;
                if m.denoise {
                    x = audio::denoise_16k(&x);
                }
                asr.transcribe(&x, &hint, cancel)?
            };
            store.save_chunk(meeting_id, idx, start, end, &segs).map_err(|e| e.to_string())?;
            let (done, total) = store.chunk_progress(meeting_id).map_err(|e| e.to_string())?;
            on_chunk(done, total);
        }
        // 3) 後片付け
        let _ = std::fs::remove_file(&pcm);
        store.set_pcm(meeting_id, None, None).map_err(|e| e.to_string())?;
        if !opts.keep_audio {
            if let (Some(a), _) = store.paths(meeting_id).map_err(|e| e.to_string())? {
                let _ = std::fs::remove_file(a);
            }
            store.clear_audio_path(meeting_id).map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    match &res {
        Ok(()) => store.set_state(meeting_id, "done", None),
        Err(msg) if msg == CANCELLED => store.set_state(meeting_id, "queued", None),
        Err(msg) => store.set_state(meeting_id, "failed", Some(msg)),
    }
    .map_err(|e| e.to_string())?;
    res
}

/// 待ちのジョブを順に処理する。中断されたジョブは「待ち」に戻す(次回、続きの区間から)。
pub fn run_jobs(
    store: &Store,
    asr: &dyn Asr,
    work_dir: &Path,
    opts: &Options,
    cancel: &Arc<AtomicBool>,
    mut on_chunk: impl FnMut(i64, i64, i64),
) -> Result<usize, String> {
    let token = CancelToken::new();
    let jobs = Jobs::new(&store.db);
    let n = jobs
        .run_pending(
            &token,
            |job| {
                if cancel.load(Ordering::SeqCst) {
                    token.cancel();
                    return Err(CANCELLED.into());
                }
                let v: serde_json::Value = serde_json::from_str(&job.payload_json).map_err(|e| e.to_string())?;
                let mid = v["meeting_id"].as_i64().ok_or("ジョブの内容が不正です")?;
                let r = process_meeting(store, asr, work_dir, mid, opts, cancel, |d, t| {
                    let _ = jobs.set_progress(job.id, if t > 0 { d as f64 / t as f64 } else { 0.0 });
                    on_chunk(mid, d, t);
                });
                if cancel.load(Ordering::SeqCst) {
                    token.cancel();
                }
                r
            },
            |_| {},
        )
        .map_err(|e| e.to_string())?;
    // 中断で failed になったジョブを待ちに戻す
    store
        .db
        .conn
        .execute("UPDATE jobs SET state='pending', error=NULL WHERE kind=?1 AND state='failed' AND error=?2", (JOB_KIND, CANCELLED))
        .map_err(|e| e.to_string())?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::{AsrSegment, FakeAsr};

    fn tmp(n: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("min-pipe-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// 2区間目の途中で中断させる ASR
    struct StopAfter {
        n: std::sync::Mutex<usize>,
        stop_at: usize,
    }
    impl Asr for StopAfter {
        fn transcribe(&self, pcm: &[f32], _: &str, cancel: &Arc<AtomicBool>) -> Result<Vec<AsrSegment>, String> {
            let mut n = self.n.lock().unwrap();
            *n += 1;
            if *n == self.stop_at {
                cancel.store(true, Ordering::SeqCst);
                return Err(CANCELLED.into());
            }
            Ok(vec![AsrSegment { start_ms: 0, end_ms: (pcm.len() / 16) as u64, text: format!("区間{}", *n), confidence: 0.9 }])
        }
        fn name(&self) -> String {
            "stop".into()
        }
    }

    #[test]
    fn テストセットの音声を処理し_中断しても続きの区間から再開できる() {
        let d = tmp("resume");
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset/t05_overlap.wav"); // 56 秒 → 2 区間
        let audio = d.join("copy.wav");
        std::fs::copy(&src, &audio).unwrap();
        let st = Store::open(&d.join("db.sqlite3")).unwrap();
        let mid = st.add_meeting("テスト", "t05.wav", &audio.to_string_lossy(), true).unwrap();
        enqueue(&st, mid).unwrap();

        let cancel = Arc::new(AtomicBool::new(false));
        let asr = StopAfter { n: Default::default(), stop_at: 2 };
        run_jobs(&st, &asr, &d, &Options { keep_audio: true }, &cancel, |_, _, _| {}).unwrap();
        let m = st.meeting(mid).unwrap().unwrap();
        assert_eq!(m.state, "queued");
        let (done, total) = st.chunk_progress(mid).unwrap();
        assert!(total >= 2 && done == 1, "{done}/{total}");
        assert_eq!(Jobs::new(&st.db).summary(Some(JOB_KIND)).unwrap().pending, 1);

        // 再開: 残りの区間だけ処理する
        cancel.store(false, Ordering::SeqCst);
        let mut calls = Vec::new();
        run_jobs(&st, &FakeAsr { text: "続き".into() }, &d, &Options { keep_audio: false }, &cancel, |_, dn, t| calls.push((dn, t))).unwrap();
        let m = st.meeting(mid).unwrap().unwrap();
        assert_eq!(m.state, "done");
        assert!(!m.has_audio, "音声を残さない設定なのでコピーは消える");
        assert!(!audio.exists());
        let segs = st.segments(mid).unwrap();
        assert_eq!(segs[0].text, "区間1");
        assert!(segs[1..].iter().all(|s| s.text == "続き"));
        assert_eq!(calls.last().unwrap().0, total);
        assert!(st.paths(mid).unwrap().1.is_none(), "作業ファイルは片付く");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn 読めない音声は理由つきで失敗にする() {
        let d = tmp("bad");
        let bad = d.join("bad.m4a");
        std::fs::write(&bad, b"not audio").unwrap();
        let st = Store::open_in_memory().unwrap();
        let mid = st.add_meeting("x", "bad.m4a", &bad.to_string_lossy(), false).unwrap();
        enqueue(&st, mid).unwrap();
        run_jobs(&st, &FakeAsr { text: "".into() }, &d, &Options { keep_audio: true }, &Arc::new(AtomicBool::new(false)), |_, _, _| {}).unwrap();
        let m = st.meeting(mid).unwrap().unwrap();
        assert_eq!(m.state, "failed");
        assert!(m.error.unwrap().contains("形式"));
        std::fs::remove_dir_all(&d).ok();
    }
}
