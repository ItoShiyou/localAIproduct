//! 1つの議事録の処理: 読み込み(16kHz へ変換)→ 区間に分割 → 区間ごとに(ノイズ除去 →)文字起こし → 保存。
//! 区間ごとに保存するので、途中で閉じても続きの区間から再開できる(`core::jobs` のジョブ1件 = 議事録1件)。

use crate::asr::Asr;
use crate::audio;
use crate::diarize::{self, Embedder};
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

pub struct Options<'a> {
    /// 処理が終わったら音声のコピーを消す(文字だけ残す設定)
    pub keep_audio: bool,
    /// 話者の判別に使う(無ければ判別しない)
    pub embedder: Option<&'a dyn Embedder>,
}

/// 区間を、文字起こしする範囲に合わせる(範囲の外は無音扱いにして飛ばす。範囲の境目で区間を切る)。
pub fn clip_chunks(chunks: Vec<audio::Chunk>, start: Option<i64>, end: Option<i64>) -> Vec<audio::Chunk> {
    let (s, e) = (start.unwrap_or(0).max(0) as u64, end.map(|v| v.max(0) as u64).unwrap_or(u64::MAX));
    chunks
        .into_iter()
        .filter_map(|c| {
            let (a, b) = (c.start_ms.max(s), c.end_ms.min(e));
            if a < b {
                Some(audio::Chunk { start_ms: a, end_ms: b, silent: c.silent || b - a < 300 })
            } else {
                None
            }
        })
        .collect()
}

/// 話者の判別: 文字起こしした範囲の声のある所を窓に分け、窓ごとに声の特徴を求めて保存し、文にラベルを付ける。
pub fn diarize_meeting(store: &Store, emb: &dyn Embedder, pcm: &Path, meeting_id: i64, num_speakers: Option<i64>, overwrite: bool) -> Result<usize, String> {
    let m = store.meeting(meeting_id).map_err(|e| e.to_string())?.ok_or("議事録が見つかりません")?;
    let rms = audio::frame_rms_of(pcm)?;
    let (s, e) = (m.range_start_ms.unwrap_or(0), m.range_end_ms.unwrap_or(i64::MAX));
    let wins: Vec<diarize::Window> = diarize::windows_from_rms(&rms, 0).into_iter().filter(|w| w.end_ms > s && w.start_ms < e && w.end_ms - w.start_ms >= diarize::WIN_MIN_MS).collect();
    let mut out = Vec::with_capacity(wins.len());
    for w in wins {
        let x = audio::read_pcm16k(pcm, w.start_ms as u64, w.end_ms as u64)?;
        if let Ok(v) = emb.embed(&x) {
            out.push((w, v));
        }
    }
    store.set_windows(meeting_id, &out).map_err(|e| e.to_string())?;
    relabel(store, meeting_id, num_speakers, overwrite)
}

/// 保存してある窓の特徴から、文のラベルを付け直す(人数を変えてやり直すとき。音声は要らない)。
pub fn relabel(store: &Store, meeting_id: i64, num_speakers: Option<i64>, overwrite: bool) -> Result<usize, String> {
    let wins = store.windows(meeting_id).map_err(|e| e.to_string())?;
    if wins.is_empty() {
        return Ok(0);
    }
    let embs: Vec<Vec<f32>> = wins.iter().map(|(_, e)| e.clone()).collect();
    let weights: Vec<i64> = wins.iter().map(|(w, _)| w.end_ms - w.start_ms).collect();
    let lab = diarize::cluster(&embs, &weights, num_speakers.map(|n| n as usize), diarize::DEFAULT_THRESHOLD);
    let ws: Vec<diarize::Window> = wins.iter().map(|(w, _)| *w).collect();
    let segs = store.segments(meeting_id).map_err(|e| e.to_string())?;
    let spk = diarize::assign_segments(&ws, &lab, &segs.iter().map(|s| (s.start_ms, s.end_ms)).collect::<Vec<_>>());
    let labels: Vec<(i64, String)> = segs.iter().zip(spk).map(|(s, l)| (s.id, diarize::label_name(l))).collect();
    store.apply_speakers(meeting_id, &labels, overwrite).map_err(|e| e.to_string())
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
                let chunks = clip_chunks(audio::split_chunks(&rms), m.range_start_ms, m.range_end_ms);
                if chunks.is_empty() {
                    return Err("文字起こしする範囲に音声がありません(範囲を確認してください)".into());
                }
                store.set_chunks(meeting_id, &chunks).map_err(|e| e.to_string())?;
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
                asr.transcribe(&x, &hint, &m.language, cancel)?
            };
            store.save_chunk(meeting_id, idx, start, end, &segs).map_err(|e| e.to_string())?;
            let (done, total) = store.chunk_progress(meeting_id).map_err(|e| e.to_string())?;
            on_chunk(done, total);
        }
        // 録音中の仮の文字で、正確な文字起こしと重ならずに残った分を消す
        store.clear_provisional(meeting_id).map_err(|e| e.to_string())?;
        // 3) 話者の判別(作業用の音声があるうちに)
        if m.diarize {
            if let Some(emb) = opts.embedder {
                if cancel.load(Ordering::SeqCst) {
                    return Err(CANCELLED.into());
                }
                diarize_meeting(store, emb, &pcm, meeting_id, m.num_speakers, false)?;
            }
        }
        // 4) 後片付け
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
        fn transcribe(&self, pcm: &[f32], _: &str, _: &str, cancel: &Arc<AtomicBool>) -> Result<Vec<AsrSegment>, String> {
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
        run_jobs(&st, &asr, &d, &Options { keep_audio: true, embedder: None }, &cancel, |_, _, _| {}).unwrap();
        let m = st.meeting(mid).unwrap().unwrap();
        assert_eq!(m.state, "queued");
        let (done, total) = st.chunk_progress(mid).unwrap();
        assert!(total >= 2 && done == 1, "{done}/{total}");
        assert_eq!(Jobs::new(&st.db).summary(Some(JOB_KIND)).unwrap().pending, 1);

        // 再開: 残りの区間だけ処理する
        cancel.store(false, Ordering::SeqCst);
        let mut calls = Vec::new();
        run_jobs(&st, &FakeAsr { text: "続き".into() }, &d, &Options { keep_audio: false, embedder: None }, &cancel, |_, dn, t| calls.push((dn, t))).unwrap();
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
    fn 範囲に合わせて区間を切る() {
        use crate::audio::Chunk;
        let c = |a, b| Chunk { start_ms: a, end_ms: b, silent: false };
        let r = clip_chunks(vec![c(0, 30_000), c(30_000, 60_000), c(60_000, 90_000)], Some(40_000), Some(60_100));
        assert_eq!(r, vec![c(40_000, 60_000), Chunk { start_ms: 60_000, end_ms: 60_100, silent: true }]);
        assert_eq!(clip_chunks(vec![c(0, 10)], None, None), vec![Chunk { start_ms: 0, end_ms: 10, silent: true }]);
    }

    /// 音の大きさで声を分ける、テスト用の特徴(t05 の主話者と、小さい声の話者)
    struct ByLoudness;
    impl crate::diarize::Embedder for ByLoudness {
        fn embed(&self, pcm: &[f32]) -> Result<Vec<f32>, String> {
            let r = (pcm.iter().map(|x| x * x).sum::<f32>() / pcm.len() as f32).sqrt();
            Ok(crate::diarize::normalize(vec![r, 0.05]))
        }
    }

    /// 区間ごとに長さの違う2文を返す ASR
    struct TwoLines;
    impl Asr for TwoLines {
        fn transcribe(&self, _: &[f32], _: &str, _: &str, _: &Arc<AtomicBool>) -> Result<Vec<AsrSegment>, String> {
            Ok(vec![
                AsrSegment { start_ms: 0, end_ms: 4_000, text: "長い文".into(), confidence: 0.9 },
                AsrSegment { start_ms: 4_000, end_ms: 6_000, text: "短い文".into(), confidence: 0.9 },
                AsrSegment { start_ms: 6_000, end_ms: 6_300, text: "相づち".into(), confidence: 0.9 },
            ])
        }
        fn name(&self) -> String {
            "two".into()
        }
    }

    #[test]
    fn 話者を判別し_人数を変えて付け直せる() {
        let d = tmp("diar");
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset/t05_overlap.wav");
        let audio = d.join("copy.wav");
        std::fs::copy(&src, &audio).unwrap();
        let st = Store::open(&d.join("db.sqlite3")).unwrap();
        let mid = st.add_meeting("t", "t05.wav", &audio.to_string_lossy(), false).unwrap();
        enqueue(&st, mid).unwrap();
        run_jobs(&st, &TwoLines, &d, &Options { keep_audio: true, embedder: Some(&ByLoudness) }, &Arc::new(AtomicBool::new(false)), |_, _, _| {}).unwrap();
        assert!(!st.windows(mid).unwrap().is_empty());
        assert!(st.segments(mid).unwrap().iter().all(|s| s.speaker.starts_with("話者")));
        assert_eq!(relabel(&st, mid, Some(1), true).unwrap(), st.segments(mid).unwrap().len());
        assert!(st.segments(mid).unwrap().iter().all(|s| s.speaker == "話者1"));
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
        run_jobs(&st, &FakeAsr { text: "".into() }, &d, &Options { keep_audio: true, embedder: None }, &Arc::new(AtomicBool::new(false)), |_, _, _| {}).unwrap();
        let m = st.meeting(mid).unwrap().unwrap();
        assert_eq!(m.state, "failed");
        assert!(m.error.unwrap().contains("形式"));
        std::fs::remove_dir_all(&d).ok();
    }
}
