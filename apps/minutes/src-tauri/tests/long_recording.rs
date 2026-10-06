//! 長い録音(1時間前後)を、アプリと同じ処理で、途中で中断 → 再開して最後まで通す。時間とメモリの確認用。
//! 明示実行時は MINUTES_LONG_AUDIO(録音の場所)と MINUTES_WHISPER_MODEL が必須。不足時は失敗。
//! 実行例: /usr/bin/time -l cargo test --release --test long_recording -- --ignored --nocapture
#![cfg(feature = "whisper")]

use minutes::asr::WhisperAsr;
use minutes::pipeline::{enqueue, run_jobs, Options};
use minutes::store::Store;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[test]
#[ignore = "requires long audio and a real speech model"]
fn 長い録音を中断して再開し_最後まで処理する() {
    let (Ok(audio), Ok(model)) = (std::env::var("MINUTES_LONG_AUDIO"), std::env::var("MINUTES_WHISPER_MODEL")) else {
        panic!("set MINUTES_LONG_AUDIO and MINUTES_WHISPER_MODEL");
    };
    let threads: i32 = std::env::var("MINUTES_THREADS").ok().and_then(|s| s.parse().ok()).unwrap_or(6);
    let asr = WhisperAsr::new(Path::new(&model), threads).unwrap();
    let d = std::env::temp_dir().join(format!("min-long-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let copy = d.join(Path::new(&audio).file_name().unwrap());
    std::fs::copy(&audio, &copy).unwrap();
    let st = Store::open(&d.join("db.sqlite3")).unwrap();
    let mid = st.add_meeting("長い録音", "long", &copy.to_string_lossy(), true).unwrap();
    enqueue(&st, mid).unwrap();

    // 1回目: 10 区間で中断(アプリを閉じた想定)
    let cancel = Arc::new(AtomicBool::new(false));
    let t0 = std::time::Instant::now();
    let c2 = cancel.clone();
    run_jobs(&st, &asr, &d, &Options::basic(true), &cancel, |_, done, _| if done >= 10 { c2.store(true, Ordering::SeqCst) }).unwrap();
    let t1 = t0.elapsed().as_secs_f64();
    let (done1, total) = st.chunk_progress(mid).unwrap();
    assert_eq!(st.meeting(mid).unwrap().unwrap().state, "queued");
    assert!(done1 >= 10 && done1 < total);
    println!("1回目: {done1}/{total} 区間で中断({t1:.0}秒。読み込みと区間分割を含む)");

    // 2回目: 続きから
    cancel.store(false, Ordering::SeqCst);
    let t2 = std::time::Instant::now();
    run_jobs(&st, &asr, &d, &Options::basic(true), &cancel, |_, _, _| {}).unwrap();
    let rest = t2.elapsed().as_secs_f64();
    let m = st.meeting(mid).unwrap().unwrap();
    assert_eq!(m.state, "done", "{:?}", m.error);
    assert_eq!(st.chunk_progress(mid).unwrap(), (total, total));
    let segs = st.segments(mid).unwrap();
    let dur = m.duration_ms.unwrap() as f64 / 1000.0;
    assert!(segs.windows(2).all(|w| w[0].start_ms <= w[1].start_ms));
    assert!(segs.last().unwrap().end_ms as f64 / 1000.0 > dur - 60.0, "最後まで文字になっている");
    let chars: usize = segs.iter().map(|s| s.text.chars().count()).sum();
    println!(
        "{{\"dur_s\":{dur:.0},\"chunks\":{total},\"segments\":{},\"chars\":{chars},\"proc_s\":{:.0},\"rtf\":{:.3}}}",
        segs.len(),
        t1 + rest,
        (t1 + rest) / dur
    );
    std::fs::remove_dir_all(&d).ok();
}
