//! 実OCR(core::ocr の PpOcr、2認識モデル併用)で架空の名刺画像を読み、ルール抽出まで通して採点する。
//! 使い方:
//!   cargo run --release --features onnx --example ocr_eval -- ORT_LIB DET REC_JA REC_ZH [threads] [--dump DIR] [--no-alt]
//! 画像は testset/images/、正解は testset/cards/NN.expected.json(NN-a / NN-b は同じ正解)。
//! モデルと onnxruntime はリポジトリに含めない(docs/licenses.md の取得元を参照)。
#[cfg(not(feature = "onnx"))]
fn main() {
    eprintln!("--features onnx が必要です");
}

#[cfg(feature = "onnx")]
fn main() {
    use bizcard_logic::eval::{compare, Expected, FIELDS};
    use bizcard_logic::extract::extract;
    use factory_core::ocr::ppocr::{PpOcr, PpOcrConfig};
    use factory_core::ocr::{Ocr, OcrImage};
    use std::time::Instant;

    let a: Vec<String> = std::env::args().collect();
    let threads = a.get(5).and_then(|s| s.parse().ok()).unwrap_or(2);
    let no_alt = a.iter().any(|x| x == "--no-alt");
    let scale: f32 = a.iter().position(|x| x == "--scale").and_then(|i| a.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let dump = a.iter().position(|x| x == "--dump").and_then(|i| a.get(i + 1)).cloned();
    let ocr = PpOcr::new(&PpOcrConfig {
        onnxruntime_lib: a[1].clone().into(),
        det_model: a[2].clone().into(),
        rec_model: a[3].clone().into(),
        dict: None,
        alt_rec_model: if no_alt { None } else { Some(a[4].clone().into()) },
        threads,
    })
    .expect("OCRの初期化に失敗");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset");
    let mut files: Vec<_> = std::fs::read_dir(root.join("images")).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map(|x| x == "png" || x == "jpg").unwrap_or(false)).collect();
    files.sort();
    if let Some(d) = &dump {
        std::fs::create_dir_all(d).unwrap();
    }
    let mut ok = [0usize; 12];
    let mut n = 0usize;
    let mut ocr_t = vec![];
    let mut ext_t = vec![];
    let mut all_contact = 0usize;
    let mut fails: Vec<String> = vec![];
    let mut cand_hit = 0usize;
    let mut cand_top1 = 0usize;
    // 初回は読み込みが入るので、時間の集計から除く(暖機)
    let warm = OcrImage::decode(&std::fs::read(&files[0]).unwrap()).unwrap();
    let _ = ocr.recognize(&warm);
    for p in &files {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let card_id = &name[..2];
        let exp: Expected = serde_json::from_str(&std::fs::read_to_string(root.join(format!("cards/{card_id}.expected.json"))).unwrap()).unwrap();
        let mut img = OcrImage::decode(&std::fs::read(p).unwrap()).unwrap();
        if scale != 1.0 {
            let d = image::load_from_memory(&std::fs::read(p).unwrap()).unwrap();
            img = OcrImage::from_dynamic(d.resize_exact((d.width() as f32 * scale) as u32, (d.height() as f32 * scale) as u32, image::imageops::FilterType::Lanczos3));
        }
        let t = Instant::now();
        let page = ocr.recognize(&img).unwrap();
        ocr_t.push(t.elapsed().as_secs_f64());
        let t = Instant::now();
        let x = extract(&page);
        ext_t.push(t.elapsed().as_secs_f64());
        let r = compare(&x, &exp);
        for i in 0..12 {
            ok[i] += r[i] as usize;
        }
        if r[5] && r[6] && r[7] && r[8] {
            all_contact += 1;
        }
        n += 1;
        {
            use bizcard_logic::extract::squash;
            let want = exp.company.as_deref().map(squash).unwrap_or_default();
            if !want.is_empty() && x.company_candidates.iter().any(|f| squash(&f.value) == want) {
                cand_hit += 1;
            }
            if !want.is_empty() && x.company_candidates.first().map(|f| squash(&f.value)) == Some(want.clone()) {
                cand_top1 += 1;
            }
        }
        let bad: Vec<&str> = FIELDS.iter().enumerate().filter(|(i, _)| !r[*i]).map(|(_, f)| *f).collect();
        if !bad.is_empty() {
            fails.push(format!("{name}: {}", bad.join(",")));
        }
        if let Some(d) = &dump {
            std::fs::write(format!("{d}/{name}.json"), serde_json::to_string_pretty(&serde_json::json!({"ocr": page, "extraction": x})).unwrap()).unwrap();
        }
    }
    println!("engine=PpOcr alt={} threads={threads} images={n}", !no_alt);
    println!("| 項目 | 一致 / {n} |\n|---|---|");
    for i in 0..12 {
        println!("| {} | {} |", FIELDS[i], ok[i]);
    }
    println!("メール・電話・携帯・FAXすべて一致(1枚単位): {all_contact}/{n}");
    let with_company = files.iter().filter(|p| { let id = &p.file_name().unwrap().to_string_lossy()[..2]; let e: Expected = serde_json::from_str(&std::fs::read_to_string(root.join(format!("cards/{id}.expected.json"))).unwrap()).unwrap(); e.company.is_some() }).count();
    println!("会社名の候補に正解がある(上位5件): {cand_hit}/{with_company}、先頭が正解: {cand_top1}/{with_company}(会社名が正解にある画像のみ)");
    ocr_t.sort_by(|a, b| a.total_cmp(b));
    ext_t.sort_by(|a, b| a.total_cmp(b));
    let pct = |v: &Vec<f64>, q: f64| v[((v.len() as f64 - 1.0) * q).round() as usize];
    println!("OCR時間: 中央値 {:.2}s / 90%点 {:.2}s / 最大 {:.2}s", pct(&ocr_t, 0.5), pct(&ocr_t, 0.9), pct(&ocr_t, 1.0));
    println!("抽出時間: 中央値 {:.4}s / 最大 {:.4}s", pct(&ext_t, 0.5), pct(&ext_t, 1.0));
    if let Ok(st) = std::fs::read_to_string("/proc/self/status") {
        for l in st.lines().filter(|l| l.starts_with("VmHWM")) {
            println!("{l}");
        }
    }
    println!("不一致:");
    for f in fails {
        println!("- {f}");
    }
}
