//! 実モデル(whisper.cpp)で、テストセットを アプリと同じ処理(pipeline)に通し、文字誤り率(CER)と時間を出す。
//! MINUTES_WHISPER_MODEL(ggml 形式のモデルの場所)が無ければ何もせずに通る。任意で MINUTES_THREADS、MINUTES_FILES(カンマ区切り)。
//! 実行例: MINUTES_WHISPER_MODEL=.../ggml-large-v3-turbo-q5_0.bin cargo test --release --test real_asr -- --nocapture
#![cfg(feature = "whisper")]

use minutes::asr::WhisperAsr;
use minutes::pipeline::{enqueue, run_jobs, Options};
use minutes::store::Store;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn norm(s: &str) -> Vec<char> {
    s.chars().filter(|c| !c.is_whitespace() && !"、。,.!?！？「」".contains(*c)).collect()
}

fn cer(r: &[char], h: &[char]) -> f64 {
    let mut prev: Vec<usize> = (0..=h.len()).collect();
    for (i, rc) in r.iter().enumerate() {
        let mut cur = vec![i + 1; h.len() + 1];
        for (j, hc) in h.iter().enumerate() {
            cur[j + 1] = (prev[j] + (rc != hc) as usize).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[h.len()] as f64 / r.len().max(1) as f64
}

#[test]
fn 実モデルでテストセットを文字起こしする() {
    let Ok(model) = std::env::var("MINUTES_WHISPER_MODEL") else {
        eprintln!("MINUTES_WHISPER_MODEL が無いため省略");
        return;
    };
    let threads: i32 = std::env::var("MINUTES_THREADS").ok().and_then(|s| s.parse().ok()).unwrap_or(6);
    let files = std::env::var("MINUTES_FILES").unwrap_or_else(|_| "t01_clean,t02_aircon,t03_keyboard,t04_farmic,t05_overlap".into());
    let asr = WhisperAsr::new(Path::new(&model), threads).unwrap();
    let ts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset");
    let d = std::env::temp_dir().join(format!("min-real-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    for f in files.split(',') {
        for denoise in [false, true] {
            let st = Store::open_in_memory().unwrap();
            let copy = d.join(format!("{f}.wav"));
            std::fs::copy(ts.join(format!("{f}.wav")), &copy).unwrap();
            let mid = st.add_meeting(f, f, &copy.to_string_lossy(), denoise).unwrap();
            enqueue(&st, mid).unwrap();
            let t0 = std::time::Instant::now();
            run_jobs(&st, &asr, &d, &Options::basic(true), &Arc::new(AtomicBool::new(false)), |_, _, _| {}).unwrap();
            let secs = t0.elapsed().as_secs_f64();
            let m = st.meeting(mid).unwrap().unwrap();
            assert_eq!(m.state, "done", "{:?}", m.error);
            let hyp: String = st.segments(mid).unwrap().iter().map(|s| s.text.clone()).collect();
            let r = norm(&std::fs::read_to_string(ts.join(format!("{f}.txt"))).unwrap());
            let dur = m.duration_ms.unwrap() as f64 / 1000.0;
            println!(
                "{{\"file\":\"{f}\",\"denoise\":{denoise},\"threads\":{threads},\"dur_s\":{dur:.1},\"proc_s\":{secs:.1},\"rtf\":{:.3},\"cer\":{:.4}}}",
                secs / dur,
                cer(&r, &norm(&hyp))
            );
        }
    }
    std::fs::remove_dir_all(&d).ok();
}
