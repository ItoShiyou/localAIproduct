//! Copy-only migration preserves edited records, media and usage without keys or executables.
#![cfg(not(feature = "mac-app-store"))]
use minutes::{asr::FakeAsr,commands::AppState,plan::Tier,store::ProcessOptions};
use std::path::Path;
fn state(dir:&Path)->AppState {
 let s=AppState::new(dir.into(),Some(Box::new(FakeAsr{text:"架空の文字".into()})),Box::new(|_|Ok(Box::new(FakeAsr{text:"架空の文字".into()}))),vec![]).unwrap();s.set_tier(Tier::Pro);s
}
#[test]
fn copy_migration_preserves_edits_audio_search_and_usage_without_touching_source() {
 let root=std::env::temp_dir().join(format!("minutes-migration-{}",std::process::id()));std::fs::create_dir_all(&root).unwrap();let root=root.canonicalize().unwrap();
 let old=root.join("old");let new=root.join("new");
 let s=state(&old);let wav=root.join("synthetic.wav");minutes::audio::write_wav16(&wav,&vec![0.2;16000]).unwrap();
 let ledger=minutes::plan::Ledger::new(vec![Box::new(minutes::plan::FileSlot(old.join("usage.dat")))]);ledger.add(1234,true);s.set_ledger(ledger);
 let id=s.import_audio(&wav,&ProcessOptions{denoise:false,diarize:false,..Default::default()}).unwrap();s.run_jobs().unwrap();s.set_tier(Tier::Pro);
 let segment=s.detail(id).unwrap().segments[0].id;s.edit_text(segment,"修正した架空の議事録").unwrap();s.set_tags(id,&["架空タグ".into()]).unwrap();s.update_notes(id,"架空の議題","架空の決定",&[]).unwrap();s.confirm(id).unwrap();
 let before=s.detail(id).unwrap();let usage=s.plan().usage.used_ms;assert!(usage>0);drop(s);
 std::fs::write(old.join("license.key"),"do not migrate this key").unwrap();
 let source_db=std::fs::read(old.join("minutes.sqlite3")).unwrap();
 let dest=state(&new);drop(dest);minutes::migration::prepare(&old,&new).unwrap();minutes::migration::apply_pending(&new).unwrap();
 let migrated=state(&new);let after=migrated.detail(id).unwrap();
 assert_eq!(after.meeting.title,before.meeting.title);assert_eq!(after.meeting.agenda,"架空の議題");assert_eq!(after.segments[0].text,"修正した架空の議事録");assert!(after.meeting.has_audio);assert!(migrated.search("架空").unwrap().len()>0);assert_eq!(migrated.plan().usage.used_ms,usage);
 let path=migrated.store.lock().unwrap().paths(id).unwrap().0.unwrap();assert!(Path::new(&path).starts_with(&new)&&Path::new(&path).is_file());
 assert!(!new.join("license.key").exists());assert_eq!(std::fs::read(old.join("minutes.sqlite3")).unwrap(),source_db);
 assert!(minutes::migration::prepare(&old,&new).is_err());drop(migrated);std::fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn migration_rejects_symlink_escape_without_overwriting_new_data() {
 use std::os::unix::fs::symlink;
 let root=std::env::temp_dir().join(format!("minutes-migration-link-{}",std::process::id()));let old=root.join("old");let new=root.join("new");drop(state(&old));drop(state(&new));
 std::fs::create_dir(old.join("audio")).unwrap();symlink(&new,old.join("audio/outside")).unwrap();
 assert!(minutes::migration::prepare(&old,&new).is_err());assert!(!new.join("legacy-migration").exists());assert!(old.join("minutes.sqlite3").exists());std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn installation_resumes_after_a_media_directory_was_moved() {
 let root=std::env::temp_dir().join(format!("minutes-migration-resume-{}",std::process::id()));let old=root.join("old");let new=root.join("new");
 drop(state(&old));drop(state(&new));std::fs::create_dir_all(old.join("audio")).unwrap();std::fs::write(old.join("audio/synthetic.pcm"),b"synthetic fixture").unwrap();
 minutes::migration::prepare(&old,&new).unwrap();let stage=new.join("legacy-migration");
 std::fs::create_dir_all(stage.join("empty-destination-backup")).unwrap();std::fs::write(stage.join("installing"),b"v1").unwrap();
 std::fs::rename(stage.join("audio"),new.join("audio")).unwrap();
 minutes::migration::apply_pending(&new).unwrap();assert_eq!(std::fs::read(new.join("audio/synthetic.pcm")).unwrap(),b"synthetic fixture");
 drop(state(&new));assert!(new.join("legacy-migration-completed").exists());minutes::migration::apply_pending(&new).unwrap();std::fs::remove_dir_all(root).unwrap();
}
