//! 実モデル(whisper.cpp + WeSpeaker)で、架空の4人の会議(spike/make_diarize_testset.py で生成)を処理し、
//! 話者の判別がどれだけ合っているかを出す。通常はignored。明示実行時はモデルや音声の不足を失敗とする。
//! 必要: MINUTES_WHISPER_MODEL、MINUTES_ORT_LIB、MINUTES_SPK_MODEL、testset/generated/diarize4.{wav,json}
#![cfg(all(feature = "whisper", feature = "diarize"))]

use minutes::asr::WhisperAsr;
use minutes::diarize::OnnxEmbedder;
use minutes::pipeline::{enqueue, relabel, run_jobs, Options};
use minutes::store::{ProcessOptions, Store};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// 予測ラベルごとに、最も多い正解の話者を数えた割合(文の数で)と、予測の人数
fn score(st: &Store, mid: i64, truth: &serde_json::Value) -> (f64, usize) {
    let lines = truth.as_array().unwrap();
    let mut table: HashMap<String, HashMap<String, usize>> = HashMap::new();
    let segs = st.segments(mid).unwrap();
    for s in &segs {
        let best = lines
            .iter()
            .max_by_key(|l| {
                let (a, b) = ((l["start"].as_f64().unwrap() * 1000.0) as i64, (l["end"].as_f64().unwrap() * 1000.0) as i64);
                (s.end_ms.min(b) - s.start_ms.max(a)).max(0)
            })
            .unwrap();
        *table.entry(s.speaker.clone()).or_default().entry(best["speaker"].as_str().unwrap().into()).or_default() += 1;
    }
    let ok: usize = table.values().map(|m| m.values().max().copied().unwrap_or(0)).sum();
    (ok as f64 / segs.len() as f64, table.len())
}

#[test]
#[ignore = "requires speech/speaker models, ONNX Runtime and generated recording"]
fn 実モデルで4人の会議の話者を判別する() {
    let (Ok(w), Ok(lib), Ok(spk)) = (std::env::var("MINUTES_WHISPER_MODEL"), std::env::var("MINUTES_ORT_LIB"), std::env::var("MINUTES_SPK_MODEL")) else {
        panic!("set MINUTES_WHISPER_MODEL, MINUTES_ORT_LIB and MINUTES_SPK_MODEL");
    };
    let ts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset/generated");
    if !ts.join("diarize4.wav").exists() {
        panic!("prepare testset/generated/diarize4.wav");
    }
    let truth: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(ts.join("diarize4.json")).unwrap()).unwrap();
    let asr = WhisperAsr::new(Path::new(&w), 6).unwrap();
    let emb = OnnxEmbedder::new(Path::new(&lib), Path::new(&spk), 4).unwrap();
    let d = std::env::temp_dir().join(format!("min-diar-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::copy(ts.join("diarize4.wav"), d.join("a.wav")).unwrap();
    let st = Store::open(&d.join("db.sqlite3")).unwrap();
    let mid = st.add_meeting("4人", "a.wav", &d.join("a.wav").to_string_lossy(), false).unwrap();
    st.set_options(mid, &ProcessOptions { denoise: false, ..Default::default() }).unwrap();
    enqueue(&st, mid).unwrap();
    let t0 = std::time::Instant::now();
    run_jobs(&st, &asr, &d, &Options { embedder: Some(&emb), ..Options::basic(true) }, &Arc::new(AtomicBool::new(false)), |_, _, _| {}).unwrap();
    let (acc, n) = score(&st, mid, &truth);
    println!("{{\"mode\":\"auto\",\"segments\":{},\"speakers\":{n},\"purity\":{acc:.3},\"proc_s\":{:.1}}}", st.segments(mid).unwrap().len(), t0.elapsed().as_secs_f64());
    relabel(&st, mid, Some(4), true).unwrap();
    let (acc4, n4) = score(&st, mid, &truth);
    println!("{{\"mode\":\"4人と指定\",\"speakers\":{n4},\"purity\":{acc4:.3}}}");
    for s in st.segments(mid).unwrap() {
        println!("  {:>6} {} {}", s.start_ms, s.speaker, s.text);
    }
    assert!(acc >= 0.8, "{acc}");
    std::fs::remove_dir_all(&d).ok();
}
