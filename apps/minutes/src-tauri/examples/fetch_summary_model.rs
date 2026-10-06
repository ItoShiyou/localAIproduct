//! Download and verify the exact optional summary model; no private audio sent.
use factory_core::model_manager::ModelManager;
use minutes::summary::SUMMARY_MODEL;
use std::sync::atomic::AtomicBool;

fn main() {
    let dir = std::env::args().nth(1).expect("specify a model directory");
    let manager = ModelManager::new(dir);
    let mut last = 0;
    let path = manager.download(&SUMMARY_MODEL, &AtomicBool::new(false), |done, total| {
        if done >= last + (100 << 20) || done == total {
            println!("{} / {} MiB", done >> 20, total >> 20);
            last = done;
        }
    }).expect("summary model download/hash verification failed");
    println!("verified {}", path.display());
}
