//! Trust boundary tests. These do not simulate an Apple purchase.
#![cfg(feature = "mac-app-store")]
use minutes::{commands::AppState, plan::Tier};
#[test]
fn mac_store_ignores_legacy_keys_and_internal_tier_override() {
    let dir = std::env::temp_dir().join(format!("minutes-mas-rights-{}", std::process::id()));
    let state = AppState::new(dir.clone(), None, minutes::whisper_loader(), vec![]).unwrap();
    assert!(state.plan().purchase_verification_pending);
    assert!(state.run_jobs().unwrap_err().contains("購入状態"));
    state.set_tier(Tier::Pro);
    assert_eq!(state.ent().tier, Tier::Free);
    assert!(!state.ent().summary);
    assert!(state.install_license("MNT1-invalid").is_err());
    assert!(state.remove_license().is_err());
    assert!(state.machine_code().is_none());
    assert_eq!(minutes::plan::resolve_tier(true, Some("pro"), true), Tier::Free);
    std::fs::remove_dir_all(dir).unwrap();
}
