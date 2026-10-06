//! Process-level crash and recovery test. Uses a test recognizer, not a claim
//! about microphone hardware or speech-recognition accuracy.
use base64::Engine;
use minutes::{asr::FakeAsr, commands::AppState, plan::Tier, store::ProcessOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn state(dir: &Path) -> AppState {
    let state = AppState::new(dir.into(), Some(Box::new(FakeAsr { text: "test".into() })),
        Box::new(|_| Ok(Box::new(FakeAsr { text: "test".into() }))), vec![]).unwrap();
    state.set_tier(Tier::Pro);
    state
}

#[test]
#[ignore = "child process used by crash_recovery; never run standalone"]
fn crash_writer_child() {
    let dir = PathBuf::from(std::env::var_os("MINUTES_CRASH_TEST_DIR").expect("child-only directory"));
    let state = state(&dir);
    let id = state.record_start(&ProcessOptions::default()).unwrap();
    let samples: Vec<u8> = (0..32_000).flat_map(|i| (((i as f32 * 0.07).sin() * 10_000.0) as i16).to_le_bytes()).collect();
    state.record_push(&base64::engine::general_purpose::STANDARD.encode(samples)).unwrap();
    std::fs::write(dir.join("ready"), id.to_string()).unwrap();
    loop { std::thread::sleep(Duration::from_millis(100)); }
}

#[test]
fn crash_recovery() {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("minutes-crash-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "crash_writer_child", "--ignored", "--nocapture"])
        .env("MINUTES_CRASH_TEST_DIR", &dir).spawn().unwrap();
    let start = Instant::now();
    while !dir.join("ready").exists() && start.elapsed() < Duration::from_secs(15) {
        if child.try_wait().unwrap().is_some() { break; }
        std::thread::sleep(Duration::from_millis(20));
    }
    let ready = dir.join("ready").exists();
    let _ = child.kill();
    let exit = child.wait().unwrap();
    assert!(ready, "child did not save recording before timeout; {exit}");
    let id: i64 = std::fs::read_to_string(dir.join("ready")).unwrap().parse().unwrap();
    let restarted = state(&dir);
    let detail = restarted.detail(id).unwrap();
    assert_eq!(detail.meeting.state, "failed");
    assert!(!detail.meeting.recording);
    assert!(detail.meeting.has_audio);
    assert_eq!(detail.meeting.duration_ms, Some(2_000));
    let wav = restarted.store.lock().unwrap().paths(id).unwrap().0.unwrap();
    assert_eq!(minutes::audio::decode_to_pcm16k(Path::new(&wav), &dir.join("check.pcm")).unwrap(), 2_000);
    restarted.retry(id).unwrap();
    assert_eq!(restarted.run_jobs().unwrap(), 1);
    assert_eq!(restarted.detail(id).unwrap().meeting.state, "done");
    assert!(restarted.detail(id).unwrap().segments.len() > 0);
    drop(restarted);
    let again = state(&dir);
    assert_eq!(again.detail(id).unwrap().meeting.state, "done");
    drop(again);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn thirty_minute_recording_preserves_duration_and_finalizes() {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("minutes-long-{}-{stamp}", std::process::id()));
    let state = state(&dir);
    let id = state.record_start(&ProcessOptions::default()).unwrap();
    let samples: Vec<u8> = (0..160_000).flat_map(|i| (((i as f32 * 0.07).sin() * 10_000.0) as i16).to_le_bytes()).collect();
    let block = base64::engine::general_purpose::STANDARD.encode(samples);
    for _ in 0..180 { state.record_push(&block).unwrap(); }
    let saved = state.record_stop().unwrap();
    assert_eq!(saved.meeting.duration_ms, Some(1_800_000));
    assert!(!saved.meeting.recording);
    let wav = state.store.lock().unwrap().paths(id).unwrap().0.unwrap();
    assert_eq!(std::fs::metadata(&wav).unwrap().len(), 57_600_044);
    assert_eq!(state.run_jobs().unwrap(), 1);
    let detail = state.detail(id).unwrap();
    assert_eq!(detail.meeting.state, "done");
    assert!(!detail.provisional);
    state.confirm(id).unwrap();
    state.export(id, "txt", &dir.join("transcript.txt")).unwrap();
    assert!(std::fs::metadata(dir.join("transcript.txt")).unwrap().len() > 0);
    drop(state);
    std::fs::remove_dir_all(&dir).unwrap();
}
