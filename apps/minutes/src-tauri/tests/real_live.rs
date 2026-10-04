//! 実モデルで、リアルタイム録音が追いつくかを確かめる(音声を実際の速さで 0.5 秒ずつ渡す)。
//! 必要: MINUTES_WHISPER_MODEL(録音中に使うモデル。小さいモデルを想定)、testset/generated/diarize4.wav
#![cfg(feature = "whisper")]

use minutes::asr::{Asr, WhisperAsr};
use minutes::recorder::LiveRecorder;
use minutes::store::{ProcessOptions, Store};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
fn 実際の速さで録音しても文字起こしが追いつく() {
    let Ok(w) = std::env::var("MINUTES_WHISPER_MODEL") else { return };
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset/generated/diarize4.wav");
    if !src.exists() {
        return;
    }
    let d = std::env::temp_dir().join(format!("min-live-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let pcm = d.join("src.pcm");
    let ms = minutes::audio::decode_to_pcm16k(&src, &pcm).unwrap();
    let audio = minutes::audio::read_pcm16k(&pcm, 0, ms).unwrap();
    let db = d.join("db.sqlite3");
    let st = Store::open(&db).unwrap();
    let id = st.add_meeting("live", "mic", "", false).unwrap();
    st.set_options(id, &ProcessOptions { denoise: false, ..Default::default() }).unwrap();
    st.set_state(id, "processing", None).unwrap();
    st.set_recording(id, true).unwrap();
    let asr: Arc<dyn Asr> = Arc::new(WhisperAsr::new(Path::new(&w), 6).unwrap());
    let mut r = LiveRecorder::start(&db, &d.join("work"), id, asr, String::new(), "ja".into(), false).unwrap();
    let t0 = Instant::now();
    let mut max_pending = 0;
    for (i, c) in audio.chunks(8_000).enumerate() {
        // 実際の時刻に合わせて渡す
        let due = Duration::from_millis(i as u64 * 500);
        if let Some(wait) = due.checked_sub(t0.elapsed()) {
            std::thread::sleep(wait);
        }
        max_pending = max_pending.max(r.push(&st, c).unwrap().pending_chunks);
    }
    let t_stop = Instant::now();
    r.stop(&st, &d.join("audio")).unwrap();
    let after = t_stop.elapsed().as_secs_f64();
    let segs = st.segments(id).unwrap();
    println!("{{\"audio_s\":{:.1},\"max_pending_chunks\":{max_pending},\"stop_wait_s\":{after:.1},\"provisional_segments\":{}}}", ms as f64 / 1000.0, segs.len());
    for s in &segs {
        println!("  {:>6} {} {}", s.start_ms, s.speaker, s.text);
    }
    assert!(max_pending <= 2, "追いついていない: {max_pending}");
    std::fs::remove_dir_all(&d).ok();
}
