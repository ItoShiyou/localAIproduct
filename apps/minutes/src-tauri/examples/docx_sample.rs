//! 開発用: 架空の議事録で Word ファイルを作る(Word / Pages / textutil で開けるかの確認用)。
//! 使い方: cargo run --example docx_sample --no-default-features -- <出力.docx>
use minutes::export::{render, Format};
use minutes::store::{Meeting, Segment, Todo};

fn main() {
    let out = std::env::args().nth(1).expect("出力先");
    let m = Meeting {
        title: "10月の定例会議(架空)".into(), held_on: Some("2026-10-01".into()), participants_text: "佐藤、鈴木".into(),
        agenda: "見積もり\n展示会".into(), decisions: "パンフレットは五百部".into(),
        todos: vec![Todo { text: "入稿".into(), owner: "鈴木".into(), due: "2026-10-08".into(), done: false }],
        ..Default::default()
    };
    let s = |t: i64, sp: &str, x: &str| Segment { id: 0, chunk_idx: 0, start_ms: t, end_ms: t + 3000, speaker: sp.into(), text: x.into(), raw_text: x.into(), confidence: 1.0, edited: false };
    let segs = vec![s(0, "佐藤", "それでは始めます。"), s(3000, "鈴木", "よろしくお願いします。A&B <社>"), s(6000, "佐藤", "以上です。")];
    std::fs::write(&out, render(Format::Docx, &m, &segs).unwrap()).unwrap();
}
