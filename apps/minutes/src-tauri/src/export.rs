//! 書き出し(Markdown・テキスト・SRT)。形式ごとに関数を分け、あとで LaTeX・Word などを足せるようにしておく。
//! 呼び出し側(commands)で、確定した議事録だけを渡すこと。

use crate::store::{Meeting, Segment};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    Markdown,
    Text,
    Srt,
}

impl Format {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "md" => Some(Self::Markdown),
            "txt" => Some(Self::Text),
            "srt" => Some(Self::Srt),
            _ => None,
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Text => "txt",
            Self::Srt => "srt",
        }
    }
}

pub fn render(f: Format, m: &Meeting, segs: &[Segment]) -> String {
    match f {
        Format::Markdown => markdown(m, segs),
        Format::Text => text(m, segs),
        Format::Srt => srt(segs),
    }
}

fn hms(ms: i64) -> String {
    let s = ms.max(0) / 1000;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// 同じ話者が続く文を1つの段落にまとめる: (開始時刻, 話者, 文)
fn paragraphs(segs: &[Segment]) -> Vec<(i64, String, String)> {
    let mut out: Vec<(i64, String, String)> = Vec::new();
    for s in segs.iter().filter(|s| !s.text.trim().is_empty()) {
        match out.last_mut() {
            Some(last) if last.1 == s.speaker => last.2.push_str(s.text.trim()),
            _ => out.push((s.start_ms, s.speaker.clone(), s.text.trim().to_string())),
        }
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
    o += "\n";
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

#[cfg(test)]
mod tests {
    use super::*;

    fn s(start: i64, end: i64, sp: &str, t: &str) -> Segment {
        Segment { id: 0, chunk_idx: 0, start_ms: start, end_ms: end, speaker: sp.into(), text: t.into(), raw_text: t.into(), confidence: 1.0, edited: false }
    }

    fn m() -> Meeting {
        Meeting {
            id: 1, title: "定例".into(), held_on: Some("2026-10-01".into()), participants_text: "佐藤、鈴木".into(), source_name: "a.m4a".into(),
            has_audio: true, duration_ms: Some(10_000), denoise: true, state: "done".into(), error: None, status: "confirmed".into(), created_at: 0,
        }
    }

    #[test]
    fn 形式ごとに書き出せる() {
        let segs = [s(0, 1_500, "佐藤", "はじめます。"), s(1_500, 3_000, "佐藤", "議題は二つです。"), s(3_725, 5_000, "", "了解です。"), s(5_000, 5_000, "", " ")];
        let md = render(Format::Markdown, &m(), &segs);
        assert!(md.starts_with("# 定例\n\n- 日付: 2026-10-01\n- 参加者: 佐藤、鈴木\n"));
        assert!(md.contains("**佐藤** [00:00:00] はじめます。議題は二つです。\n\n[00:00:03] 了解です。"));
        let txt = render(Format::Text, &m(), &segs);
        assert!(txt.contains("[00:00:00] 佐藤: はじめます。議題は二つです。\n[00:00:03] 了解です。\n"));
        let srt = render(Format::Srt, &m(), &segs);
        assert_eq!(srt.matches(" --> ").count(), 3);
        assert!(srt.contains("3\n00:00:03,725 --> 00:00:05,000\n了解です。\n"));
        assert_eq!(Format::parse("srt"), Some(Format::Srt));
    }
}
