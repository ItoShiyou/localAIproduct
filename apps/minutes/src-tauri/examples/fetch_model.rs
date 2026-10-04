//! 開発用: 文字起こしのモデルを実際の配布元から取得し、途中で中断 → 再開 → ハッシュ照合 まで確かめる。
//! 使い方: cargo run --release --example fetch_model --no-default-features -- <保存先フォルダ> [中断するMB]
use factory_core::model_manager::ModelManager;
use minutes::commands::WHISPER_MODEL;
use std::sync::atomic::{AtomicBool, Ordering};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args.first().expect("保存先フォルダを指定してください");
    let stop_mb: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let m = ModelManager::new(dir);
    let t0 = std::time::Instant::now();
    if stop_mb > 0 && !m.is_installed(&WHISPER_MODEL) {
        let cancel = AtomicBool::new(false);
        let r = m.download(&WHISPER_MODEL, &cancel, |d, _| if d >= stop_mb << 20 { cancel.store(true, Ordering::SeqCst) });
        println!("1回目: {r:?}、途中まで {} バイト", m.status(&WHISPER_MODEL).downloaded);
    }
    let mut last = 0;
    let p = m
        .download(&WHISPER_MODEL, &AtomicBool::new(false), |d, t| {
            if d >= last + (100 << 20) || d == t {
                println!("  {} / {} MB", d >> 20, t >> 20);
                last = d;
            }
        })
        .unwrap();
    println!("取得・照合済み: {}({:.0}秒)、installed={}", p.display(), t0.elapsed().as_secs_f64(), m.is_installed(&WHISPER_MODEL));
}
