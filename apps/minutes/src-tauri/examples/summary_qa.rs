//! Component QA on fabricated text; no meeting database or user audio involved.
use std::{path::PathBuf};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3, "summary_qa <engine directory> <Qwen GGUF path>");
    let dir = std::env::temp_dir().join(format!("minutes-summary-qa-{}",std::process::id()));
    let runtime = minutes::summary::SummaryRuntime::new(dir.clone());
    let engines = vec![PathBuf::from(&args[1])];
    runtime.set_sidecar_dirs(engines.clone());
    runtime.set_factory(minutes::summary::sidecar_factory(engines));
    runtime.set_bundled_model(PathBuf::from(&args[2]));
    let paragraphs = vec!["架空の会議です。試作品の展示会を11月5日に行うことに決めました。".into(),
        "佐藤さんが10月30日までに会場の見積書を作成して共有します。".into(),
        "人手不足のため検査が遅れています。応援の手配はまだ決定していません。".into()];
    let result = runtime.run(1,"架空の展示会打ち合わせ",&paragraphs).unwrap();
    println!("{}",serde_json::to_string_pretty(&result).unwrap());
    assert!(!result.draft.summary.is_empty());
    assert!(result.draft.decisions.iter().any(|line|line.contains("展示会")));
    assert!(result.draft.todos.iter().any(|todo|todo.text.contains("見積")));
    std::fs::remove_dir_all(dir).ok();
}
