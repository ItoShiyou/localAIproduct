//! 架空テストセットでの抽出結果の固定(回帰テスト)。
//! 正解ラベルは手書きの架空データ。これは実OCR出力での精度ではない。

use bizcard_logic::eval::*;

#[test]
fn 想定内の名刺はすべての項目が一致する() {
    let rs = run_testset();
    assert!(rs.len() >= 25, "テストセットが読めていない: {}枚", rs.len());
    for r in rs.iter().filter(|r| !r.hard) {
        let bad: Vec<&str> = FIELDS.iter().enumerate().filter(|(i, _)| !r.ok[*i]).map(|(_, n)| *n).collect();
        assert!(bad.is_empty(), "card {} の不一致: {:?}", r.id, bad);
    }
}

/// 「難しい」名刺で、いま外れている項目を固定する。ルールを直して外れなくなったら、
/// このリストを更新する(改善も悪化も見落とさないため)。
#[test]
fn 難しい名刺の既知の不一致を固定する() {
    let rs = run_testset();
    let mut got: Vec<(String, Vec<&str>)> = rs
        .iter()
        .filter(|r| r.hard)
        .map(|r| (r.id.clone(), FIELDS.iter().enumerate().filter(|(i, _)| !r.ok[*i]).map(|(_, n)| *n).collect::<Vec<_>>()))
        .filter(|(_, b)| !b.is_empty())
        .collect();
    got.sort();
    let want: Vec<(String, Vec<&str>)> = vec![
        ("12".into(), vec!["emails"]),
        ("19".into(), vec!["company"]),
        ("21".into(), vec!["name"]),
        ("22".into(), vec!["phones", "mobiles"]),
        ("24".into(), vec!["company"]),
        ("25".into(), vec!["phones"]),
    ];
    assert_eq!(got, want);
}

#[test]
fn 要確認の印は信頼度の低い名刺の項目に付く() {
    let rs = run_testset();
    let low = rs.iter().find(|r| r.id == "17").unwrap();
    assert!(low.extracted.phones[0].needs_review());
    assert!(low.extracted.address.as_ref().unwrap().needs_review());
    assert!(low.extracted.emails[0].needs_review());
    let high = rs.iter().find(|r| r.id == "01").unwrap();
    assert!(!high.extracted.emails[0].needs_review());
}

#[test]
fn 氏名のない裏面で氏名を拾わない() {
    let rs = run_testset();
    let back = rs.iter().find(|r| r.id == "10").unwrap();
    assert!(back.extracted.name.is_none());
}
