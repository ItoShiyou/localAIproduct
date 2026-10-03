//! `cargo run --example eval` で、架空テストセットの採点結果を表で出す。
use bizcard_logic::eval::*;

fn main() {
    let rs = run_testset();
    let rows = tally(&rs, |_| true);
    println!("| 項目 | 正解 | 枚数 |\n|---|---|---|");
    for (n, ok, t) in &rows {
        println!("| {n} | {ok} | {t} |");
    }
    let (c, t) = contact_tally(&rs);
    println!("\nメール・電話・携帯・FAXがすべて一致: {c}/{t}");
    let (co, na, t) = company_name_tally(&rs);
    println!("会社名一致 {co}/{t}, 氏名一致 {na}/{t}");
    println!("\n不一致の内訳:");
    for r in &rs {
        let bad: Vec<&str> = FIELDS.iter().enumerate().filter(|(i, _)| !r.ok[*i]).map(|(_, n)| *n).collect();
        if !bad.is_empty() {
            println!("- card {} (hard={}): {} {}", r.id, r.hard, bad.join(","), r.note);
        }
    }
    if std::env::args().any(|a| a == "--time") {
        // ルール抽出だけの時間(OCRは含まない)。ビルド種別と環境によって大きく変わるので参考値
        let dir = testset_dir();
        let cards: Vec<bizcard_logic::ocr_input::OcrCard> = rs
            .iter()
            .map(|r| serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{}.ocr.json", r.id))).unwrap()).unwrap())
            .collect();
        let n = 200;
        let t = std::time::Instant::now();
        for _ in 0..n {
            for c in &cards {
                std::hint::black_box(bizcard_logic::extract::extract(c));
            }
        }
        let per = t.elapsed().as_secs_f64() * 1000.0 / (n * cards.len()) as f64;
        println!("\nルール抽出のみ: 1枚あたり平均 {per:.3} ms ({} 枚 x {n} 回)", cards.len());
    }
    if std::env::args().any(|a| a == "--verbose") {
        for r in &rs {
            println!("{} => {:#?}", r.id, r.extracted);
        }
    }
}
