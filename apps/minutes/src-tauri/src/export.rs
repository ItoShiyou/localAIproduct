//! 書き出し(Markdown・テキスト・SRT・Word)。形式ごとに関数を分け、あとで形式を足せるようにしておく。
//! PDF は画面の印刷(OS の「PDF として保存」)で作る(日本語のフォントを同梱しないため)。
//! 呼び出し側(commands)で、確定した議事録だけを渡すこと。

use crate::store::{Meeting, Segment};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    Markdown,
    Text,
    Srt,
    Docx,
}

impl Format {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "md" => Some(Self::Markdown),
            "txt" => Some(Self::Text),
            "srt" => Some(Self::Srt),
            "docx" => Some(Self::Docx),
            _ => None,
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Text => "txt",
            Self::Srt => "srt",
            Self::Docx => "docx",
        }
    }
}

pub fn render(f: Format, m: &Meeting, segs: &[Segment]) -> Result<Vec<u8>, String> {
    Ok(match f {
        Format::Markdown => markdown(m, segs).into_bytes(),
        Format::Text => text(m, segs).into_bytes(),
        Format::Srt => srt(segs).into_bytes(),
        Format::Docx => docx(m, segs)?,
    })
}

pub fn hms(ms: i64) -> String {
    let s = ms.max(0) / 1000;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// 同じ話者が続く文を1つの段落にまとめる: (開始時刻, 話者, 文)
pub fn paragraphs(segs: &[Segment]) -> Vec<(i64, String, String)> {
    let mut out: Vec<(i64, String, String)> = Vec::new();
    for s in segs.iter().filter(|s| !s.text.trim().is_empty()) {
        match out.last_mut() {
            Some(last) if last.1 == s.speaker => last.2.push_str(s.text.trim()),
            _ => out.push((s.start_ms, s.speaker.clone(), s.text.trim().to_string())),
        }
    }
    out
}

/// 定型欄(議題・決定事項・ToDo)を、見出しと行の組にする。空の欄は出さない。
fn notes(m: &Meeting) -> Vec<(&'static str, Vec<String>)> {
    let lines = |s: &str| s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect::<Vec<_>>();
    let mut out = Vec::new();
    if !m.agenda.trim().is_empty() {
        out.push(("議題", lines(&m.agenda)));
    }
    if !m.decisions.trim().is_empty() {
        out.push(("決定事項", lines(&m.decisions)));
    }
    if !m.todos.is_empty() {
        let v = m
            .todos
            .iter()
            .map(|t| {
                let mut s = format!("{}{}", if t.done { "[済] " } else { "" }, t.text.trim());
                if !t.owner.trim().is_empty() {
                    s += &format!("(担当: {})", t.owner.trim());
                }
                if !t.due.is_empty() {
                    s += &format!("(期限: {})", t.due);
                }
                s
            })
            .collect();
        out.push(("ToDo", v));
    }
    out
}

fn markdown(m: &Meeting, segs: &[Segment]) -> String {
    let mut o = format!("# {}\n\n", m.title);
    if let Some(d) = &m.held_on {
        o += &format!("- 日付: {d}\n");
    }
    if !m.participants_text.trim().is_empty() {
        o += &format!("- 参加者: {}\n", m.participants_text.trim());
    }
    for (h, items) in notes(m) {
        o += &format!("\n## {h}\n\n");
        for i in items {
            o += &format!("- {i}\n");
        }
    }
    o += "\n## 内容\n\n";
    for (t, sp, tx) in paragraphs(segs) {
        if sp.is_empty() {
            o += &format!("[{}] {}\n\n", hms(t), tx);
        } else {
            o += &format!("**{}** [{}] {}\n\n", sp, hms(t), tx);
        }
    }
    o
}

fn text(m: &Meeting, segs: &[Segment]) -> String {
    let mut o = format!("{}\n", m.title);
    if let Some(d) = &m.held_on {
        o += &format!("日付: {d}\n");
    }
    if !m.participants_text.trim().is_empty() {
        o += &format!("参加者: {}\n", m.participants_text.trim());
    }
    for (h, items) in notes(m) {
        o += &format!("\n■{h}\n");
        for i in items {
            o += &format!("・{i}\n");
        }
    }
    o += "\n■内容\n";
    for (t, sp, tx) in paragraphs(segs) {
        if sp.is_empty() {
            o += &format!("[{}] {}\n", hms(t), tx);
        } else {
            o += &format!("[{}] {}: {}\n", hms(t), sp, tx);
        }
    }
    o
}

fn srt(segs: &[Segment]) -> String {
    let ts = |ms: i64| {
        let ms = ms.max(0);
        format!("{},{:03}", hms(ms), ms % 1000)
    };
    let mut o = String::new();
    for (i, s) in segs.iter().filter(|s| !s.text.trim().is_empty()).enumerate() {
        let body = if s.speaker.is_empty() { s.text.trim().to_string() } else { format!("{}: {}", s.speaker, s.text.trim()) };
        o += &format!("{}\n{} --> {}\n{}\n\n", i + 1, ts(s.start_ms), ts(s.end_ms.max(s.start_ms + 1)), body);
    }
    o
}

// ---------------- Word(.docx、WordprocessingML を直接書く) ----------------

fn xml(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(*c as u32, 0..=8 | 11 | 12 | 14..=31)) // XML に入れられない制御文字を除く
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn run(text: &str, bold: bool, color: Option<&str>) -> String {
    let mut pr = String::new();
    if bold {
        pr += "<w:b/>";
    }
    if let Some(c) = color {
        pr += &format!("<w:color w:val=\"{c}\"/>");
    }
    format!("<w:r><w:rPr>{pr}</w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r>", xml(text))
}

fn para(style: Option<&str>, runs: &str) -> String {
    let st = style.map(|s| format!("<w:pPr><w:pStyle w:val=\"{s}\"/></w:pPr>")).unwrap_or_default();
    format!("<w:p>{st}{runs}</w:p>")
}

fn docx(m: &Meeting, segs: &[Segment]) -> Result<Vec<u8>, String> {
    use std::io::Write;
    let mut body = para(Some("Title"), &run(&m.title, false, None));
    let mut meta = Vec::new();
    if let Some(d) = &m.held_on {
        meta.push(format!("日付: {d}"));
    }
    if !m.participants_text.trim().is_empty() {
        meta.push(format!("参加者: {}", m.participants_text.trim()));
    }
    for l in meta {
        body += &para(None, &run(&l, false, Some("555555")));
    }
    for (h, items) in notes(m) {
        body += &para(Some("Heading1"), &run(h, false, None));
        for i in items {
            body += &para(Some("ListBullet"), &run(&format!("・{i}"), false, None));
        }
    }
    body += &para(Some("Heading1"), &run("内容", false, None));
    for (t, sp, tx) in paragraphs(segs) {
        let mut r = String::new();
        if !sp.is_empty() {
            r += &run(&format!("{sp} "), true, None);
        }
        r += &run(&format!("[{}] ", hms(t)), false, Some("888888"));
        r += &run(&tx, false, None);
        body += &para(None, &r);
    }
    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}\
<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\" w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/></w:sectPr>\
</w:body></w:document>"
    );
    let font = "<w:rFonts w:ascii=\"Yu Gothic\" w:hAnsi=\"Yu Gothic\" w:eastAsia=\"游ゴシック\" w:cs=\"Yu Gothic\"/>";
    let styles = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w:styles xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
<w:docDefaults><w:rPrDefault><w:rPr>{font}<w:sz w:val=\"21\"/><w:lang w:val=\"ja-JP\" w:eastAsia=\"ja-JP\"/></w:rPr></w:rPrDefault>\
<w:pPrDefault><w:pPr><w:spacing w:after=\"120\" w:line=\"300\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault></w:docDefaults>\
<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Title\"><w:name w:val=\"Title\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:spacing w:after=\"240\"/></w:pPr><w:rPr><w:b/><w:sz w:val=\"36\"/></w:rPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Heading1\"><w:name w:val=\"heading 1\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:keepNext/><w:spacing w:before=\"360\" w:after=\"120\"/><w:outlineLvl w:val=\"0\"/></w:pPr><w:rPr><w:b/><w:sz w:val=\"26\"/></w:rPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"ListBullet\"><w:name w:val=\"List Bullet\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:ind w:left=\"360\"/><w:spacing w:after=\"60\"/></w:pPr></w:style>\
</w:styles>"
    );
    let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
<Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/>\
</Types>";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\
</Relationships>";
    let doc_rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>\
</Relationships>";
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opt = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in [
            ("[Content_Types].xml", content_types.to_string()),
            ("_rels/.rels", rels.to_string()),
            ("word/_rels/document.xml.rels", doc_rels.to_string()),
            ("word/document.xml", document),
            ("word/styles.xml", styles),
        ] {
            z.start_file(name, opt).map_err(|e| e.to_string())?;
            z.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
        }
        z.finish().map_err(|e| e.to_string())?;
    }
    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Todo;

    fn s(start: i64, end: i64, sp: &str, t: &str) -> Segment {
        Segment { id: 0, chunk_idx: 0, start_ms: start, end_ms: end, speaker: sp.into(), text: t.into(), raw_text: t.into(), confidence: 1.0, edited: false }
    }

    fn m() -> Meeting {
        Meeting {
            id: 1, title: "定例".into(), held_on: Some("2026-10-01".into()), participants_text: "佐藤、鈴木".into(), source_name: "a.m4a".into(),
            has_audio: true, duration_ms: Some(10_000), denoise: true, state: "done".into(), status: "confirmed".into(),
            agenda: "予算\n日程".into(), decisions: "五百部にする".into(),
            todos: vec![Todo { text: "入稿".into(), owner: "鈴木".into(), due: "2026-10-08".into(), done: false }],
            ..Default::default()
        }
    }

    fn str(f: Format, segs: &[Segment]) -> String {
        String::from_utf8(render(f, &m(), segs).unwrap()).unwrap()
    }

    #[test]
    fn 形式ごとに書き出せる() {
        let segs = [s(0, 1_500, "佐藤", "はじめます。"), s(1_500, 3_000, "佐藤", "議題は二つです。"), s(3_725, 5_000, "", "了解です。"), s(5_000, 5_000, "", " ")];
        let md = str(Format::Markdown, &segs);
        assert!(md.starts_with("# 定例\n\n- 日付: 2026-10-01\n- 参加者: 佐藤、鈴木\n"));
        assert!(md.contains("## 議題\n\n- 予算\n- 日程\n"));
        assert!(md.contains("## ToDo\n\n- 入稿(担当: 鈴木)(期限: 2026-10-08)\n"));
        assert!(md.contains("**佐藤** [00:00:00] はじめます。議題は二つです。\n\n[00:00:03] 了解です。"));
        let txt = str(Format::Text, &segs);
        assert!(txt.contains("■決定事項\n・五百部にする\n"));
        assert!(txt.contains("[00:00:00] 佐藤: はじめます。議題は二つです。\n[00:00:03] 了解です。\n"));
        let srt = str(Format::Srt, &segs);
        assert_eq!(srt.matches(" --> ").count(), 3);
        assert!(srt.contains("3\n00:00:03,725 --> 00:00:05,000\n了解です。\n"));
        assert_eq!(Format::parse("docx"), Some(Format::Docx));
    }

    #[test]
    fn wordのファイルはzipで_本文と書式を含む() {
        use std::io::Read;
        let segs = [s(0, 1_000, "佐藤", "A&B <テスト>\u{1}")];
        let bytes = render(Format::Docx, &m(), &segs).unwrap();
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut doc = String::new();
        z.by_name("word/document.xml").unwrap().read_to_string(&mut doc).unwrap();
        assert!(doc.contains("A&amp;B &lt;テスト&gt;") && !doc.contains('\u{1}'));
        assert!(doc.contains("<w:pStyle w:val=\"Title\"/>") && doc.contains("定例"));
        assert!(doc.contains("・入稿(担当: 鈴木)(期限: 2026-10-08)"));
        for n in ["[Content_Types].xml", "_rels/.rels", "word/styles.xml", "word/_rels/document.xml.rels"] {
            assert!(z.by_name(n).is_ok(), "{n}");
        }
    }
}
