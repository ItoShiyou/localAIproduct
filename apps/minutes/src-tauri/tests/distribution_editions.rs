#![cfg(any(feature = "edition-free", feature = "edition-pro"))]
use minutes::{commands::AppState, plan::{distribution_tier, Tier}, store::ProcessOptions};
use std::path::PathBuf;

fn temp() -> PathBuf {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    std::env::temp_dir().join(format!("minutes-edition-{}-{stamp}", std::process::id()))
}

#[test]
fn edition_entitlements_cannot_be_changed_by_a_license_or_runtime_tier() {
    let dir = temp();
    let s = AppState::new(dir.clone(), None, minutes::whisper_loader(), vec![]).unwrap();
    let expected = distribution_tier().unwrap();
    s.set_tier(if expected == Tier::Free { Tier::Pro } else { Tier::Free });
    assert_eq!(s.ent().tier, expected);
    assert!(s.install_license("MNT1-INVALID").is_err());
    assert!(!dir.join(minutes::commands::LICENSE_FILE).exists());
    if expected == Tier::Free {
        assert!(!s.ent().summary && !s.ent().glossary && !s.ent().denoise && !s.ent().diarize);
        assert!(s.download_summary_model().is_err());
        assert!(s.download_model().is_err());
        assert_eq!(s.ent().exports, vec!["txt"]);
        assert_eq!(s.model_status().status.size, 190_085_487);
    } else {
        assert!(s.ent().summary && s.ent().glossary && s.ent().denoise && s.ent().diarize);
        assert!(s.ent().can_export("docx"));
        assert_eq!(s.plan().remaining_ms, None);
    }
    drop(s);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "explicit local model / audio test; sends nothing and never downloads"]
fn edition_bundled_audio_and_summary() {
    let dir = temp();
    let resources = PathBuf::from(std::env::var("MINUTES_EDITION_RESOURCES").expect("set bundle resources directory"));
    let audio = PathBuf::from(std::env::var("MINUTES_EDITION_AUDIO").expect("set fictional test audio"));
    let s = AppState::new(dir.clone(), None, minutes::whisper_loader(),
        vec![(resources.join("models/ggml-large-v3-turbo-q5_0.bin"), "bundled")]).unwrap();
    s.set_live_models(vec![resources.join("models/ggml-small-q5_1.bin")]);
    s.set_flag("offline_mode", true).unwrap();
    let id = s.import_audio(&audio, &ProcessOptions { denoise: false, diarize: false, ..Default::default() }).unwrap();
    assert_eq!(s.run_jobs().unwrap(), 1);
    let detail = s.detail(id).unwrap();
    assert_eq!(detail.meeting.state, "done");
    assert!(detail.segments.iter().any(|x| !x.text.trim().is_empty()));
    assert_eq!(s.ent().tier, distribution_tier().unwrap());
    if distribution_tier() == Some(Tier::Pro) {
        s.summary.set_bundled_model(resources.join("models").join(minutes::summary::SUMMARY_MODEL.file_name));
        let dirs = vec![PathBuf::from(std::env::var("MINUTES_EDITION_SIDECAR_DIR").expect("set installed sidecar directory"))];
        s.summary.set_sidecar_dirs(dirs.clone());
        s.summary.set_factory(minutes::summary::sidecar_factory(dirs));
        assert_eq!(s.summary_status().source, "bundled");
        assert!(s.summary_status().engine);
        let result = s.summarize(id).unwrap();
        assert!(!result.draft.summary.is_empty() || !result.draft.todos.is_empty() || !result.draft.decisions.is_empty());
        assert_eq!(s.detail(id).unwrap().meeting.status, "draft");
    } else {
        assert!(!resources.join("models/ggml-large-v3-turbo-q5_0.bin").exists());
        assert!(!resources.join("models").join(minutes::summary::SUMMARY_MODEL.file_name).exists());
        assert_eq!(s.model_status().source, "bundled");
        assert!(s.summarize(id).is_err());
    }
    drop(s);
    std::fs::remove_dir_all(dir).unwrap();
}
