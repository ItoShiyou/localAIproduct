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

// Cross-edition process test. The Python driver creates an isolated fixture;
// this does not touch the user's application data or OS credential store.
#[test]
#[ignore = "run with tools/test_minutes_edition_upgrade.py"]
fn persisted_edition_upgrade() {
    use minutes::{asr::FakeAsr, store::Todo};
    let dir = PathBuf::from(std::env::var("MINUTES_UPGRADE_FIXTURE").expect("isolated fixture required"));
    assert!(dir.file_name().unwrap().to_string_lossy().starts_with("minutes-upgrade-test-"));
    assert_eq!(std::fs::read_to_string(dir.join("fixture-only")).unwrap(), "fictional-test-data");
    let phase = std::env::var("MINUTES_UPGRADE_PHASE").unwrap();
    let audio = dir.join("fictional.wav");
    if phase == "create" {
        assert_eq!(distribution_tier(), Some(Tier::Free));
        let pcm: Vec<f32> = (0..32000).map(|i| (i as f32 * 0.07).sin() * 0.3).collect();
        minutes::audio::write_wav16(&audio, &pcm).unwrap();
    }
    let state = AppState::new(dir.join("data"), Some(Box::new(FakeAsr { text:"架空の会議です".into() })),
        Box::new(|_| Ok(Box::new(FakeAsr { text:"架空の会議です".into() }))), vec![]).unwrap();
    state.set_live_models(vec![audio.clone()]);
    if phase == "create" {
        let id = state.import_audio(&audio, &ProcessOptions { denoise:false, diarize:false, ..Default::default() }).unwrap();
        assert_eq!(state.run_jobs().unwrap(), 1);
        state.update_meta(id, "引き継ぎ試験", Some("2026-10-07".into()), "架空の参加者").unwrap();
        let segment = state.detail(id).unwrap().segments[0].id;
        state.edit_text(segment, "修正済みの架空の発言").unwrap();
        state.update_notes(id, "確認する議題", "架空の決定事項", &[Todo {
            text:"架空の作業".into(), owner:"担当A".into(), due:"2026-10-08".into(), done:false,
        }]).unwrap();
        state.set_tags(id, &["引き継ぎ".into()]).unwrap();
        state.confirm(id).unwrap();
        state.export(id, "txt", &dir.join("free.txt")).unwrap();
        assert!(state.export(id, "docx", &dir.join("blocked.docx")).is_err());
        std::fs::write(dir.join("meeting-id"), id.to_string()).unwrap();
        std::fs::write(dir.join("usage"), serde_json::to_vec(&state.plan().usage).unwrap()).unwrap();
    } else {
        assert!(phase == "pro" || phase == "return-free");
        let expected = if phase == "pro" { Tier::Pro } else { Tier::Free };
        assert_eq!(state.ent().tier, expected);
        let id = std::fs::read_to_string(dir.join("meeting-id")).unwrap().parse().unwrap();
        let d = state.detail(id).unwrap();
        assert_eq!(d.meeting.title, "引き継ぎ試験");
        assert_eq!(d.meeting.participants_text, "架空の参加者");
        assert_eq!(d.meeting.agenda, "確認する議題");
        assert_eq!(d.meeting.decisions, "架空の決定事項");
        assert_eq!(d.meeting.todos[0].owner, "担当A");
        assert_eq!(d.segments[0].text, "修正済みの架空の発言");
        assert_eq!(d.meeting.status, "confirmed");
        assert!(std::path::Path::new(&state.audio_path(id).unwrap().unwrap()).is_file());
        assert!(state.search("修正済み").unwrap().iter().any(|hit| hit.meeting_id == id));
        assert!(state.all_tags().unwrap().iter().any(|(tag, _)| tag == "引き継ぎ"));
        assert_eq!(serde_json::to_vec(&state.plan().usage).unwrap(), std::fs::read(dir.join("usage")).unwrap());
        if phase == "pro" {
            assert_eq!(state.plan().remaining_ms, None);
            state.export(id, "docx", &dir.join("pro.docx")).unwrap();
            state.export(id, "txt", &dir.join("pro.txt")).unwrap();
            assert!(!std::fs::read_to_string(dir.join("pro.txt")).unwrap().contains(minutes::plan::FREE_FOOTER));
        } else {
            assert!(state.plan().remaining_ms.unwrap() < minutes::plan::FREE_TOTAL_MS);
            assert!(state.export(id, "docx", &dir.join("blocked-again.docx")).is_err());
        }
    }
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
