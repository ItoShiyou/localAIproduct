//! Opt-in real ENOSPC test on a dedicated temporary disk image, never a user volume.
#![cfg(not(feature = "mac-app-store"))]
use base64::Engine;
use minutes::{asr::FakeAsr, commands::AppState, plan::Tier, store::ProcessOptions};
use std::path::Path;
fn state(dir: &Path) -> AppState {
    let state = AppState::new(dir.into(), Some(Box::new(FakeAsr { text: "架空のテスト".into() })),
        Box::new(|_| Ok(Box::new(FakeAsr { text: "架空のテスト".into() }))), vec![]).unwrap();
    state.set_tier(Tier::Pro); state
}
#[test]
#[ignore = "requires dedicated 32MB HFS+ QA image mounted under /tmp with marker"]
fn full_disk_reports_error_and_recording_recovers_after_space_is_freed() {
    let root = std::path::PathBuf::from(std::env::var("MINUTES_QA_VOLUME").expect("dedicated image mountpoint required"));
    assert!(root.canonicalize().unwrap().starts_with("/private/tmp/"));
    assert_eq!(std::fs::read_to_string(root.join("minutes-qa-marker")).unwrap(), "minutes-owned-capacity-fixture");
    let filler = root.join("qa-only-filler");
    std::fs::write(&filler, vec![0u8; 20 * 1024 * 1024]).unwrap();
    let dir = root.join("qa-app-data");
    let s = state(&dir);
    let id = s.record_start(&ProcessOptions { denoise:false, diarize:false, ..Default::default() }).unwrap();
    let bytes: Vec<u8> = (0..16_000).flat_map(|i| (((i as f32*0.07).sin()*10_000.) as i16).to_le_bytes()).collect();
    let block = base64::engine::general_purpose::STANDARD.encode(bytes);
    let mut failure = None;
    for _ in 0..2000 {
        if let Err(error) = s.record_push(&block) { failure = Some(error); break; }
    }
    let error = failure.expect("dedicated disk did not fill");
    assert!(error.contains("空き容量") || error.contains("disk is full"), "{error}");
    std::fs::remove_file(&filler).unwrap();
    drop(s);
    let restored = state(&dir);
    let meeting = restored.detail(id).unwrap().meeting;
    assert!(meeting.has_audio && meeting.duration_ms.unwrap_or(0) > 0);
    println!("Actual disk full returned a user-facing error; recorded prefix recovered after freeing QA filler");
    drop(restored);
    std::fs::remove_dir_all(dir).unwrap();
}
