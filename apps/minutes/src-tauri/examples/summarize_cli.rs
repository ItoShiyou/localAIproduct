//! 要約の動作確認用(開発用)。サイドカーとモデルを指定して、テキストファイル(1行=1段落)を要約する。
//!   cargo run --release --no-default-features --example summarize_cli -- <サイドカー> <モデル.gguf> <文字起こし.txt> [スレッド数]
//! 結果の JSON と、所要時間・トークン数を標準出力に出す。
use minutes::summary::{summarize, SidecarSummarizer};
use std::path::Path;
use std::sync::atomic::AtomicBool;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 {
        eprintln!("使い方: summarize_cli <サイドカー> <モデル.gguf> <文字起こし.txt> [スレッド数]");
        std::process::exit(2);
    }
    let threads = a.get(4).and_then(|s| s.parse().ok()).unwrap_or_else(minutes::summary::default_threads);
    let paras: Vec<String> = std::fs::read_to_string(&a[3]).expect("読めません").lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    let t0 = std::time::Instant::now();
    let s = SidecarSummarizer::spawn(Path::new(&a[1]), Path::new(&a[2]), threads).expect("起動できません");
    let load = t0.elapsed().as_secs_f64();
    let mut last = (0, 0);
    let r = summarize(&s, "定例会議", &paras, &AtomicBool::new(false), &mut |p| {
        if (p.step, p.total) != last {
            eprintln!("[{}/{}] {}", p.step, p.total, p.phase);
            last = (p.step, p.total);
        }
    });
    match r {
        Ok(r) => {
            println!("{}", serde_json::to_string_pretty(&r.draft).unwrap());
            let st = &r.stats;
            println!(
                "--- 起動・モデル読み込み {load:.1}秒 / 要約 {:.1}秒 / 区切り {} / 入力 {} トークン({:.1}秒、{:.1} トークン/秒) / 出力 {} トークン({:.1}秒、{:.1} トークン/秒)",
                st.seconds, st.chunks, st.prompt_tokens, st.prefill_seconds, st.prompt_tokens as f64 / st.prefill_seconds.max(1e-9), st.gen_tokens, st.gen_seconds, st.gen_tokens as f64 / st.gen_seconds.max(1e-9)
            );
        }
        Err(e) => {
            eprintln!("失敗: {e}");
            std::process::exit(1);
        }
    }
}
