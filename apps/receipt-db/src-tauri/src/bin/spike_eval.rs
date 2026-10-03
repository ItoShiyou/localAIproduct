//! スパイク用: OCR/PDFテキストの出力ディレクトリと labels.json を突き合わせて、項目ごとの正答率を出す。
//! 使い方: spike_eval <labels.json> <テキストのディレクトリ(ファイル名.txt)>
use receipt_db::rules::extract_by_rules;
use receipt_db::vendor::guess_vendor;
use serde_json::Value;
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let labels: Value = serde_json::from_str(&std::fs::read_to_string(&args[1]).unwrap()).unwrap();
    let dir = &args[2];
    let verbose = args.get(3).map(|s| s == "-v").unwrap_or(false);
    // 区分ごと: (件数, 日付, 金額, 税率, 登録番号)
    let mut stat: BTreeMap<String, [u32; 7]> = BTreeMap::new();
    for (file, l) in labels.as_object().unwrap() {
        let text = std::fs::read_to_string(format!("{dir}/{file}.txt")).unwrap_or_default();
        let r = extract_by_rules(&text);
        let ok_date = r.date.as_deref() == l["date"].as_str();
        let ok_total = r.total == l["total"].as_i64();
        let ok_rate = r.tax_rate.map(|x| x as i64) == l["tax_rate"].as_i64();
        let vend = guess_vendor(&text).map(|v| v.name).unwrap_or_default();
        let want = l["vendor"].as_str().unwrap_or("");
        let ok_vendor = vend.replace(' ', "") == want;
        let near_vendor = lev(&vend.replace(' ', ""), want) <= 1;
        let ok_inv = r.invoice_no.as_deref() == l["invoice_no"].as_str();
        for key in [l["category"].as_str().unwrap(), "全体"] {
            let s = stat.entry(key.to_string()).or_default();
            s[0] += 1;
            s[1] += ok_date as u32;
            s[2] += ok_total as u32;
            s[3] += ok_rate as u32;
            s[4] += ok_inv as u32;
            s[5] += ok_vendor as u32;
            s[6] += near_vendor as u32;
        }
        if verbose && !(ok_date && ok_total && ok_rate && ok_inv) {
            println!("MISS {file}: date {:?}/{} total {:?}/{} rate {:?}/{} inv {:?}/{} vendor {:?}", r.date, l["date"], r.total, l["total"], r.tax_rate, l["tax_rate"], r.invoice_no, l["invoice_no"], vend);
        }
    }
    println!("区分,件数,日付,金額,税率,登録番号,支払先(完全一致),支払先(1文字以内の差)");
    for (k, s) in &stat {
        println!("{k},{},{},{},{},{},{},{}", s[0], s[1], s[2], s[3], s[4], s[5], s[6]);
    }
}

/// 文字単位の編集距離
fn lev(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i];
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur.push((prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost));
        }
        prev = cur;
    }
    prev[b.len()]
}
