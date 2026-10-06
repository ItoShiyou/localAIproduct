//! 実物で通す確認: テストセットの録音 → 実モデル(whisper.cpp)で文字起こし → 実モデル(Qwen3)+サイドカーで要約の下書き。
//! 必要な環境変数(どれかが無ければ何もせずに通る): MINUTES_WHISPER_MODEL(ggml の場所)、MINUTES_SUMMARY_MODEL(GGUF の場所)、
//! MINUTES_SUMMARIZER_BIN(サイドカーの実行ファイル)。任意で MINUTES_FILES(既定 t01_clean)。
//! 実行例: MINUTES_WHISPER_MODEL=… MINUTES_SUMMARY_MODEL=… MINUTES_SUMMARIZER_BIN=… cargo test --release --test real_summary -- --nocapture
#![cfg(feature = "whisper")]

use minutes::commands::AppState;
use minutes::plan::Tier;
use minutes::store::ProcessOptions;
use std::path::{Path, PathBuf};

fn reverses_staffing_cause(line: &str) -> bool {
    line.split_once("ため").is_some_and(|(cause, result)| {
        cause.contains("遅れ") && result.contains("人手不足")
    }) || (line.contains("遅れが原因") && line.contains("人手不足"))
}

#[test]
fn 因果の検査は応援の必要性を誤って拒否しない() {
    assert!(reverses_staffing_cause("検査工程が遅れているため人手不足で別の部署からの応援が必要"));
    assert!(!reverses_staffing_cause("検査工程が遅れているため、別の部署からの人手応援が必要"));
    assert!(!reverses_staffing_cause("人手不足のため、検査工程が遅れている"));
}

#[test]
#[ignore = "requires real speech/summary models and summarizer executable"]
fn 実物で文字起こしから要約の下書きまで通す() {
    let (Ok(whisper), Ok(_), Ok(_)) = (std::env::var("MINUTES_WHISPER_MODEL"), std::env::var("MINUTES_SUMMARY_MODEL"), std::env::var("MINUTES_SUMMARIZER_BIN")) else {
        panic!("set MINUTES_WHISPER_MODEL, MINUTES_SUMMARY_MODEL and MINUTES_SUMMARIZER_BIN");
    };
    let file = std::env::var("MINUTES_FILES").unwrap_or_else(|_| "t01_clean".into());
    let d = std::env::temp_dir().join(format!("min-real-summary-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let s = AppState::new(d.clone(), None, minutes::whisper_loader(), vec![(PathBuf::from(whisper), "env")]).unwrap();
    s.set_tier(Tier::Pro);
    s.summary.set_sidecar_dirs(vec![]);
    s.summary.set_factory(minutes::summary::sidecar_factory(vec![]));

    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../testset/{file}.wav"));
    let id = s.import_audio(&src, &ProcessOptions { denoise: false, diarize: false, ..Default::default() }).unwrap();
    let t0 = std::time::Instant::now();
    s.run_jobs().unwrap();
    let asr_secs = t0.elapsed().as_secs_f64();
    let det = s.detail(id).unwrap();
    assert_eq!(det.meeting.state, "done", "{:?}", det.meeting.error);
    let text: String = det.segments.iter().map(|x| x.text.clone()).collect();
    println!("--- 文字起こし({asr_secs:.1} 秒)\n{text}");

    let t1 = std::time::Instant::now();
    let r = s.summarize(id).expect("要約できませんでした");
    println!("--- 要約の下書き({:.1} 秒、うちサイドカー起動とモデル読み込みを含む)", t1.elapsed().as_secs_f64());
    println!("{}", serde_json::to_string_pretty(&r.draft).unwrap());
    println!("{}", serde_json::to_string(&r.stats).unwrap());
    assert!(!r.draft.summary.is_empty());
    if file == "t01_clean" {
        assert!(r.draft.todos.iter().any(|todo| todo.text.contains("見積") && todo.due.contains("月曜日")), "期限付きの見積書作成・共有を落としている");
        assert!(!r.draft.todos.iter().any(|todo| todo.text.contains("保存場所") || todo.text.contains("維持する")), "現状維持を新しいToDoにしている");
        assert!(!r.draft.summary.iter().any(|line| reverses_staffing_cause(line)), "検査工程の遅れを人手不足の原因へ逆転している");
    }
    // 下書きを返しただけで、議事録は変わっていない
    let m = s.detail(id).unwrap().meeting;
    assert!(m.agenda.is_empty() && m.decisions.is_empty() && m.todos.is_empty());
    std::fs::remove_dir_all(&d).ok();
}
