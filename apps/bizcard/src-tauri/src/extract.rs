//! ルールによる項目抽出(生成モデルは使わない)。
//!
//! 入力はOCRの「行+位置情報」。出力は項目ごとの値と、ルール上の確からしさ(0〜1)。
//! この確からしさは**確率ではなく、ルールの強さとOCR信頼度から作った目安**で、
//! 確認画面で「要確認」の印を付ける閾値に使う。抽出結果は必ず利用者が確認して確定する。
//!
//! 既知の限界(テストセットでも確認できる範囲のみ):
//! - 会社名は「株式会社」「Inc.」などの語を含む行だけを拾う。語を含まない屋号は拾えない
//! - 氏名は「漢字2〜4字(+スペース区切り)」「英字2〜3語」の形で、文字の大きさと位置で選ぶ。
//!   読みがな・ローマ字は隣接する行から拾う。名乗りに記号が入る名前などは拾えない
//! - OCRの誤認識(「.」が「,」になる等)は直さない
//! - 同じ面に複数人の名前がある名刺は、最も大きい1人だけを候補の先頭にする

use crate::ocr_input::{BBox, OcrCard, OcrLine};
use regex::Regex;
use serde::Serialize;
use std::sync::OnceLock;

/// 確認画面で「要確認」の印を付ける目安の閾値
pub const REVIEW_THRESHOLD: f32 = 0.7;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Field {
    pub value: String,
    /// ルールとOCR信頼度から作った目安(確率ではない)
    pub confidence: f32,
    /// 元の行の番号(入力 `lines` の添字)
    pub line: usize,
}

impl Field {
    pub fn needs_review(&self) -> bool {
        self.confidence < REVIEW_THRESHOLD
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Extraction {
    pub name: Option<Field>,
    /// 氏名の候補(スコアの高い順。先頭が `name`)
    pub name_candidates: Vec<Field>,
    pub name_kana: Option<Field>,
    pub company: Option<Field>,
    pub department: Option<Field>,
    pub title: Option<Field>,
    pub emails: Vec<Field>,
    pub phones: Vec<Field>,
    pub mobiles: Vec<Field>,
    pub faxes: Vec<Field>,
    pub postal_code: Option<Field>,
    pub address: Option<Field>,
    pub urls: Vec<Field>,
}

// ---------- 正規化 ----------

/// 全角英数記号を半角に、全角スペースを半角に、各種ハイフンを `-` にそろえる。
pub fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let n = match c {
            '\u{3000}' => ' ',
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            '\u{2010}'..='\u{2015}' | '\u{2212}' => '-',
            '\u{00A0}' => ' ',
            _ => c,
        };
        out.push(n);
    }
    // 数字にはさまれた長音記号はハイフンの誤認識として扱う
    let re = re_cached(&LONG_BAR, r"(\d)ー(\d)");
    let mut prev;
    let mut cur = out;
    loop {
        prev = cur.clone();
        cur = re.replace_all(&prev, "$1-$2").into_owned();
        if cur == prev {
            break;
        }
    }
    cur
}

/// 比較用: 空白を除き、半角にそろえる(評価と重複検知で使う)
pub fn squash(s: &str) -> String {
    normalize(s).chars().filter(|c| !c.is_whitespace()).collect()
}

static LONG_BAR: OnceLock<Regex> = OnceLock::new();
fn re_cached(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).expect("regex"))
}

macro_rules! re {
    ($pat:expr) => {{
        static CELL: OnceLock<Regex> = OnceLock::new();
        re_cached(&CELL, $pat)
    }};
}

// ---------- 語彙 ----------

/// 長い順に並べる(末尾一致で最長のものを採る)
const TITLE_JA: &[&str] = &[
    "代表取締役社長", "代表取締役会長", "代表取締役", "専務取締役", "常務取締役", "取締役", "執行役員", "代表社員",
    "代表理事", "理事長", "事務局長", "副社長", "副部長", "副課長", "社会保険労務士", "公認会計士", "司法書士",
    "行政書士", "コンサルタント", "プロデューサー", "ディレクター", "マネージャー", "マネジャー", "アドバイザー",
    "エンジニア", "デザイナー", "リーダー", "支店長", "本部長", "工場長", "社長", "会長", "専務", "常務", "部長",
    "次長", "課長", "係長", "主任", "主査", "主幹", "参事", "室長", "所長", "店長", "院長", "校長", "代表", "顧問",
    "相談役", "監査役", "弁護士", "税理士", "担当",
];

/// 「営業部長」のように部署名と役職が1語になっている場合の、役職側に残す語と、部署に戻す接尾辞
const TITLE_SPLIT: &[(&str, &str)] = &[
    ("本部長", "本部"), ("支店長", "支店"), ("部長", "部"), ("課長", "課"), ("室長", "室"), ("所長", "所"),
];

const DEPT_SUFFIX_JA: &[&str] = &["事業部", "本部", "グループ", "チーム", "センター", "営業所", "支店", "部", "課", "室", "係", "局", "科"];

const COMPANY_JA: &[&str] = &[
    "株式会社", "有限会社", "合同会社", "合資会社", "合名会社", "一般社団法人", "一般財団法人", "公益社団法人",
    "公益財団法人", "NPO法人", "医療法人", "社会福祉法人", "学校法人", "(株)", "(有)", "㈱", "㈲",
];

fn company_en() -> &'static Regex {
    re!(r"(?i)(\b(Inc|Incorporated|Corp|Corporation|LLC|L\.L\.C|GmbH|PLC|Pty|Limited)\b\.?|\bCo\.,?\s*Ltd\.?|\bLtd\.?(\s|$)|\bCompany\b)")
}
fn title_en() -> &'static Regex {
    re!(r"(?i)\b(CEO|CTO|COO|CFO|CIO|President|Vice President|VP|Director|Manager|Engineer|Developer|Designer|Consultant|Founder|Co-?founder|Partner|Principal|Officer|Lead|Head|Chairman|Representative|Associate|Analyst|Architect|Specialist|Producer|Editor|Attorney|Accountant)\b")
}
fn dept_en() -> &'static Regex {
    re!(r"(?i)\b(Department|Dept\.?|Division|Section|Team|Group|Office)\b")
}

fn has_cjk(s: &str) -> bool {
    s.chars().any(|c| matches!(c, '\u{3040}'..='\u{30FF}' | '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}'))
}

// ---------- 内部の作業用構造 ----------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Role {
    None,
    Contact,
    Address,
    Company,
    DeptTitle,
}

struct Work<'a> {
    idx: usize,
    line: &'a OcrLine,
    norm: String,
    rest: String,
    role: Role,
}

fn boundary_ok(s: &str, start: usize, end: usize) -> bool {
    let prev = s[..start].chars().next_back();
    let next = s[end..].chars().next();
    let bad = |c: char| c.is_ascii_digit() || c == '-' || c == '+';
    !prev.map(bad).unwrap_or(false) && !next.map(|c| c.is_ascii_digit() || c == '-').unwrap_or(false)
}

fn mask(s: &mut String, start: usize, end: usize) {
    s.replace_range(start..end, &" ".repeat(end - start));
}

fn strip_labels(s: &str) -> String {
    let re = re!(r"(?i)(e-?mail|mail|tel|電話|携帯|fax|ファックス|url|web|website|hp|ホームページ|main|sub|mobile|cell|phone|直通|代表|住所|所在地|〒)");
    let t = re.replace_all(s, " ");
    t.chars().filter(|c| c.is_alphanumeric() || (!c.is_whitespace() && !":/・|()[]-,.;".contains(*c))).collect::<String>()
}

fn reading_order(card: &OcrCard) -> Vec<usize> {
    let mut v: Vec<usize> = (0..card.lines.len()).collect();
    let all_vertical = !card.lines.is_empty() && card.lines.iter().all(|l| l.vertical);
    v.sort_by(|&a, &b| {
        let (la, lb) = (&card.lines[a].bbox, &card.lines[b].bbox);
        let key = |bb: &BBox| if all_vertical { (-bb.cx(), bb.y) } else { (bb.y, bb.x) };
        let (ka, kb) = (key(la), key(lb));
        ka.partial_cmp(&kb).unwrap_or(std::cmp::Ordering::Equal)
    });
    v
}

// ---------- 抽出本体 ----------

pub fn extract(card: &OcrCard) -> Extraction {
    let order = reading_order(card);
    let mut works: Vec<Work> = order
        .iter()
        .map(|&i| {
            let l = &card.lines[i];
            let norm = normalize(&l.text);
            Work { idx: i, line: l, rest: norm.clone(), norm, role: Role::None }
        })
        .collect();
    let mut ex = Extraction::default();

    // 1) メール
    for w in works.iter_mut() {
        let re = re!(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9\-]+(?:\.[A-Za-z0-9\-]+)*\.[A-Za-z]{2,}");
        let spans: Vec<(usize, usize)> = re.find_iter(&w.rest).map(|m| (m.start(), m.end())).collect();
        for (s, e) in spans {
            let v = w.rest[s..e].trim_end_matches('.').to_string();
            let lv = v.to_lowercase();
            if !ex.emails.iter().any(|f| f.value.to_lowercase() == lv) {
                ex.emails.push(Field { value: v, confidence: w.line.confidence * 0.98, line: w.idx });
            }
            mask(&mut w.rest, s, e);
        }
    }

    // 2) URL
    for w in works.iter_mut() {
        let re = re!(r"(?i)(https?://[^\s]+|www\.[^\s]+)");
        let mut spans: Vec<(usize, usize)> = re.find_iter(&w.rest).map(|m| (m.start(), m.end())).collect();
        if spans.is_empty() {
            let lab = re!(r"(?i)(?:\burl|\bweb(?:site)?|\bhp|ホームページ)\s*[:：]?\s*([a-z0-9][a-z0-9\-]*(?:\.[a-z0-9\-]+)+(?:/[^\s]*)?)");
            if let Some(c) = lab.captures(&w.rest) {
                let m = c.get(1).unwrap();
                spans.push((m.start(), m.end()));
            }
        }
        for (s, e) in spans {
            let v = w.rest[s..e].trim_end_matches(|c| ",.;)".contains(c)).to_string();
            ex.urls.push(Field { value: v, confidence: w.line.confidence * 0.9, line: w.idx });
            mask(&mut w.rest, s, e);
        }
    }

    // 3) 電話・携帯・FAX
    for w in works.iter_mut() {
        let re = re!(
            r"\+\d{1,3}[-\s.]?\(?\d{1,4}\)?(?:[-\s.]\d{2,4}){1,3}|\(\d{2,5}\)\s?\d{1,4}[-.]\d{3,4}|0\d{1,4}-\d{1,4}-\d{3,4}|\d{3}[-.]\d{3}[-.]\d{4}"
        );
        let spans: Vec<(usize, usize)> = re
            .find_iter(&w.rest)
            .filter(|m| boundary_ok(&w.rest, m.start(), m.end()))
            .map(|m| (m.start(), m.end()))
            .collect();
        let mut prev_end = 0;
        for (s, e) in &spans {
            let ctx = w.rest[prev_end..*s].to_lowercase();
            prev_end = *e;
            let num = w.rest[*s..*e].trim().to_string();
            let digits: String = num.chars().filter(|c| c.is_ascii_digit()).collect();
            let jp_mobile = digits.starts_with("070") || digits.starts_with("080") || digits.starts_with("090")
                || digits.starts_with("8170") || digits.starts_with("8180") || digits.starts_with("8190");
            let is_fax = ctx.contains("fax") || ctx.contains("ファックス") || ctx.contains("ファクス");
            let is_mobile = !is_fax
                && (ctx.contains("携帯") || ctx.contains("mobile") || ctx.contains("cell") || jp_mobile);
            let strength = if ctx.trim().is_empty() && !jp_mobile { 0.9 } else { 0.95 };
            let f = Field { value: num, confidence: w.line.confidence * strength, line: w.idx };
            if is_fax {
                ex.faxes.push(f)
            } else if is_mobile {
                ex.mobiles.push(f)
            } else {
                ex.phones.push(f)
            }
        }
        for (s, e) in spans {
            mask(&mut w.rest, s, e);
        }
    }

    // 4) 郵便番号(〒付き、または行頭の 000-0000)
    for w in works.iter_mut() {
        if ex.postal_code.is_some() {
            break;
        }
        let re = re!(r"^\s*(?:〒\s*)?(\d{3})-?(\d{4})(?:\s|$)|〒\s*(\d{3})-?(\d{4})");
        let found = re.captures(&w.rest).map(|c| {
            let (a, b) = if let Some(a) = c.get(1) { (a.as_str(), c.get(2).unwrap().as_str()) } else { (c.get(3).unwrap().as_str(), c.get(4).unwrap().as_str()) };
            let m = c.get(0).unwrap();
            (format!("{a}-{b}"), m.start(), m.end())
        });
        if let Some((v, s, e)) = found {
            ex.postal_code = Some(Field { value: v, confidence: w.line.confidence * 0.95, line: w.idx });
            mask(&mut w.rest, s, e);
        }
    }

    // 5) 住所
    let n = works.len();
    for i in 0..n {
        if ex.address.is_some() {
            break;
        }
        if works[i].role != Role::None {
            continue;
        }
        let t = works[i].rest.trim().to_string();
        let ja = re!(r"^(?:住所|所在地)?\s*:?\s*(北海道|東京都|京都府|大阪府|[^\s\d]{2,3}県)\S*?[市区町村郡]");
        let en = re!(r"(?i)^\d+[\w\s.,#&\-]*\b(Street|St|Avenue|Ave|Road|Rd|Boulevard|Blvd|Drive|Dr|Lane|Ln|Suite|Floor|Way|Building|Bldg)\b");
        if ja.is_match(&t) || (!has_cjk(&t) && en.is_match(&t)) {
            let mut text = t.trim_start_matches(|c: char| "住所所在地: ".contains(c)).to_string();
            works[i].role = Role::Address;
            // 次の行が建物名・州と郵便番号なら連結する
            if let Some(j) = (i + 1 < n).then_some(i + 1) {
                let nx = works[j].rest.trim().to_string();
                let same_dir = works[j].line.vertical == works[i].line.vertical;
                let gap_ok = works[i].line.bbox.gap(&works[j].line.bbox) <= works[i].line.char_size() * 1.5;
                let ja_cont = re!(r"^\S*(ビル|タワー|会館|階|\d+F|号室|棟)\S*$");
                let en_cont = re!(r"^[A-Za-z .]+,?\s+[A-Z]{2}\s+\d{5}(?:-\d{4})?$");
                if works[j].role == Role::None && same_dir && gap_ok && (ja_cont.is_match(&nx) || en_cont.is_match(&nx)) {
                    text = format!("{text} {nx}");
                    works[j].role = Role::Address;
                    works[j].rest = String::new();
                }
            }
            ex.address = Some(Field { value: text, confidence: works[i].line.confidence * 0.85, line: works[i].idx });
            works[i].rest = String::new();
        }
    }

    // 連絡先の行(ラベルしか残らない行)を確定
    for w in works.iter_mut() {
        if w.role == Role::None && w.rest != w.norm && strip_labels(&w.rest).is_empty() {
            w.role = Role::Contact;
        }
    }

    // 6) 会社
    let mut dt_tokens: Vec<(usize, String)> = Vec::new(); // 会社行から切り出した部署・役職候補
    for w in works.iter_mut() {
        if ex.company.is_some() {
            break;
        }
        if w.role != Role::None {
            continue;
        }
        let t = w.rest.trim().to_string();
        let is_co = COMPANY_JA.iter().any(|k| t.contains(k)) || company_en().is_match(&t);
        if !is_co {
            continue;
        }
        let mut company = t.clone();
        if has_cjk(&t) {
            let toks: Vec<&str> = t.split_whitespace().collect();
            if let Some(p) = toks.iter().skip(1).position(|k| ja_dept_or_title(k).is_some()) {
                let p = p + 1;
                company = toks[..p].join(" ");
                dt_tokens.push((w.idx, toks[p..].join(" ")));
            }
        }
        ex.company = Some(Field { value: company, confidence: w.line.confidence * 0.85, line: w.idx });
        w.role = Role::Company;
        w.rest = String::new();
    }

    // 7) 部署・役職の候補を集める
    struct Cand {
        line: usize,
        bbox: BBox,
        conf: f32,
        dept: Option<String>,
        title: Option<String>,
    }
    let mut cands: Vec<Cand> = Vec::new();
    let mut name_texts: Vec<(usize, String, &OcrLine)> = Vec::new(); // 氏名候補になりうる文字列
    for w in works.iter_mut() {
        let text = if w.role == Role::None {
            w.rest.trim().to_string()
        } else if let Some((_, extra)) = dt_tokens.iter().find(|(i, _)| *i == w.idx) {
            extra.clone()
        } else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        let (dept, title, remainder) = classify_dept_title(&text);
        if dept.is_some() || title.is_some() {
            if w.role == Role::None {
                w.role = Role::DeptTitle;
            }
            cands.push(Cand { line: w.idx, bbox: w.line.bbox, conf: w.line.confidence, dept, title });
            if !remainder.is_empty() && w.role == Role::DeptTitle {
                name_texts.push((w.idx, remainder, w.line));
            }
        } else if w.role == Role::None {
            name_texts.push((w.idx, text, w.line));
        }
    }

    // 8) 氏名
    let max_size = name_texts
        .iter()
        .filter(|(_, t, _)| name_shape(t).is_some())
        .map(|(_, _, l)| l.char_size())
        .fold(0.0_f32, f32::max);
    let mut names: Vec<(f32, Field, BBox, bool)> = Vec::new();
    for (idx, t, l) in &name_texts {
        let Some(kind) = name_shape(t) else { continue };
        let size_ratio = if max_size > 0.0 { l.char_size() / max_size } else { 1.0 };
        let pos = if l.vertical { 1.0 } else if l.bbox.cy() / card.height < 0.7 { 1.0 } else { 0.0 };
        let kind_w = match kind {
            NameKind::Ja => 1.0,
            NameKind::Latin => 0.7,
        };
        let score = 0.5 * size_ratio + 0.15 * pos + 0.25 * kind_w + 0.1 * l.confidence;
        names.push((
            score,
            Field { value: t.trim().to_string(), confidence: 0.0, line: *idx },
            l.bbox,
            l.vertical,
        ));
    }
    // 読みがな: ひらがな/カタカナだけの行で、氏名候補に隣接するもの
    let kana_re = re!(r"^[ぁ-んァ-ヶー・ ]+$");
    let kana_lines: Vec<(usize, String, &OcrLine)> = works
        .iter()
        .filter(|w| w.role == Role::None && kana_re.is_match(w.rest.trim()) && w.rest.trim().chars().count() >= 3)
        .map(|w| (w.idx, w.rest.trim().to_string(), w.line))
        .collect();
    let mut kana_for: Vec<Option<(f32, String, usize, f32)>> = vec![None; names.len()];
    for (ni, (_, nf, bb, vert)) in names.iter().enumerate() {
        let size = if *vert { bb.w } else { bb.h };
        let mut best: Option<(f32, String, usize, f32)> = None;
        for (ki, kt, kl) in &kana_lines {
            if kl.vertical != *vert || *ki == nf.line {
                continue;
            }
            let g = bb.gap(&kl.bbox);
            if g <= size * 1.5 && best.as_ref().map(|b| g < b.0).unwrap_or(true) {
                best = Some((g, kt.clone(), *ki, kl.confidence));
            }
        }
        kana_for[ni] = best;
    }
    for (ni, n) in names.iter_mut().enumerate() {
        if kana_for[ni].is_some() && matches!(name_shape(&n.1.value), Some(NameKind::Ja)) {
            n.0 += 0.1;
        }
    }
    let mut order_n: Vec<usize> = (0..names.len()).collect();
    order_n.sort_by(|&a, &b| names[b].0.partial_cmp(&names[a].0).unwrap_or(std::cmp::Ordering::Equal));
    let min_score = 0.45;
    for &ni in &order_n {
        let (score, f, _, _) = &names[ni];
        if *score < min_score {
            continue;
        }
        let line_conf = card.lines[f.line].confidence;
        ex.name_candidates.push(Field { confidence: (score.min(1.0) * 0.9).min(line_conf), ..f.clone() });
    }
    let top_ni = order_n.iter().copied().find(|&ni| names[ni].0 >= min_score);
    ex.name = ex.name_candidates.first().cloned();
    if let Some(ni) = top_ni {
        if matches!(name_shape(&names[ni].1.value), Some(NameKind::Ja)) {
            if let Some((_, kt, ki, kc)) = &kana_for[ni] {
                ex.name_kana = Some(Field { value: kt.clone(), confidence: kc * 0.8, line: *ki });
            }
        }
    }

    // 部署・役職は、氏名に近いものを採る(氏名が無ければ先頭)
    let near = |c: &Cand| -> bool {
        match &ex.name {
            Some(n) => {
                let nb = &card.lines[n.line].bbox;
                let size = card.lines[n.line].char_size();
                nb.gap(&c.bbox) <= size * 4.0
            }
            None => true,
        }
    };
    let dist = |c: &Cand| -> f32 {
        ex.name.as_ref().map(|n| card.lines[n.line].bbox.gap(&c.bbox)).unwrap_or(0.0)
    };
    let pick = |want_title: bool| -> Option<Field> {
        cands
            .iter()
            .filter(|c| if want_title { c.title.is_some() } else { c.dept.is_some() })
            .filter(|c| near(c))
            .min_by(|a, b| dist(a).partial_cmp(&dist(b)).unwrap_or(std::cmp::Ordering::Equal))
            .map(|c| Field {
                value: if want_title { c.title.clone().unwrap() } else { c.dept.clone().unwrap() },
                confidence: c.conf * if want_title { 0.85 } else { 0.8 },
                line: c.line,
            })
    };
    ex.title = pick(true);
    ex.department = pick(false);
    ex
}

#[derive(Clone, Copy, PartialEq)]
enum NameKind {
    Ja,
    Latin,
}

fn name_shape(t: &str) -> Option<NameKind> {
    let t = t.trim();
    let ja_spaced = re!(r"^[\p{Han}々ぁ-んァ-ヶー]{1,4} [\p{Han}々ぁ-んァ-ヶー]{1,4}$");
    let ja_unspaced = re!(r"^[\p{Han}々]{2,4}$");
    let has_han = t.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c) || c == '々');
    if has_han && (ja_spaced.is_match(t) || ja_unspaced.is_match(t)) {
        return Some(NameKind::Ja);
    }
    let latin = re!(r"^[A-Z][a-z]+(?:[ .\-][A-Z][a-z]*\.?){1,3}$|^[A-Z]{2,}(?: [A-Z]{2,}){1,2}$");
    if latin.is_match(t) {
        return Some(NameKind::Latin);
    }
    None
}

/// 日本語の1語が部署・役職か。(部署, 役職) を返す
fn ja_dept_or_title(tok: &str) -> Option<(Option<String>, Option<String>)> {
    for t in TITLE_JA {
        if tok.ends_with(t) {
            let prefix = &tok[..tok.len() - t.len()];
            if prefix.is_empty() {
                return Some((None, Some(tok.to_string())));
            }
            if let Some((_, suffix)) = TITLE_SPLIT.iter().find(|(w, _)| w == t) {
                return Some((Some(format!("{prefix}{suffix}")), Some(t.to_string())));
            }
            return Some((None, Some(tok.to_string())));
        }
    }
    // 2文字以下は人名(阿部・服部など)と区別できないので部署にしない
    if tok.chars().count() >= 3 && DEPT_SUFFIX_JA.iter().any(|s| tok.ends_with(s)) {
        return Some((Some(tok.to_string()), None));
    }
    None
}

/// 1行の文字列を (部署, 役職, 残り) に分ける
fn classify_dept_title(text: &str) -> (Option<String>, Option<String>, String) {
    if !has_cjk(text) {
        // 英語: 行全体で判定。カンマ・縦線・スラッシュで区切って個別に見る
        let parts: Vec<&str> = text.split(|c| c == ',' || c == '|' || c == '/').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        let mut dept = None;
        let mut title = None;
        for p in &parts {
            let (is_t, is_d) = (title_en().is_match(p), dept_en().is_match(p));
            if is_d && !is_t {
                dept.get_or_insert(p.to_string());
            } else if is_t {
                title.get_or_insert(p.to_string());
            }
        }
        return (dept, title, String::new());
    }
    let mut dept: Vec<String> = Vec::new();
    let mut title: Vec<String> = Vec::new();
    let mut rest: Vec<&str> = Vec::new();
    for tok in text.split_whitespace() {
        if let Some((d, t)) = ja_dept_or_title(tok) {
            if let Some(d) = d {
                dept.push(d);
            }
            if let Some(t) = t {
                title.push(t);
            }
        } else if !has_cjk(tok) && title_en().is_match(tok) {
            title.push(tok.to_string());
        } else {
            rest.push(tok);
        }
    }
    let j = |v: Vec<String>| if v.is_empty() { None } else { Some(v.join(" ")) };
    (j(dept), j(title), rest.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, x: f32, y: f32, size: f32) -> OcrLine {
        let w = text.chars().count() as f32 * size * 0.8;
        OcrLine { text: text.into(), bbox: BBox { x, y, w, h: size }, vertical: false, confidence: 0.97 }
    }
    fn card(lines: Vec<OcrLine>) -> OcrCard {
        OcrCard { id: "t".into(), side: "front".into(), width: 1050.0, height: 600.0, lines }
    }
    fn vals(v: &[Field]) -> Vec<&str> {
        v.iter().map(|f| f.value.as_str()).collect()
    }

    #[test]
    fn 全角を半角にそろえる() {
        assert_eq!(normalize("ＴＥＬ：０３−０００−１２３４"), "TEL:03-000-1234");
        assert_eq!(normalize("０３ー０００ー１２３４"), "03-000-1234");
        assert_eq!(normalize("a＠b.example.com"), "a@b.example.com");
    }

    #[test]
    fn メールはラベルや大文字小文字が違っても拾い_重複を除く() {
        let c = card(vec![line("E-mail: Taro.Y@Example.COM", 0.0, 0.0, 18.0), line("taro.y@example.com", 0.0, 30.0, 18.0)]);
        assert_eq!(vals(&extract(&c).emails), vec!["Taro.Y@Example.COM"]);
    }

    #[test]
    fn 電話_携帯_faxを区別する() {
        let c = card(vec![line("TEL 03-0000-0001 FAX 03-0000-0002", 0.0, 0.0, 18.0), line("090-0000-0003", 0.0, 30.0, 18.0)]);
        let x = extract(&c);
        assert_eq!(vals(&x.phones), vec!["03-0000-0001"]);
        assert_eq!(vals(&x.faxes), vec!["03-0000-0002"]);
        assert_eq!(vals(&x.mobiles), vec!["090-0000-0003"]);
    }

    #[test]
    fn 郵便番号は電話や住所の番地と取り違えない() {
        let c = card(vec![line("〒100-0001 東京都千代田区1-10-1", 0.0, 0.0, 18.0), line("電話 03-0000-0004", 0.0, 30.0, 18.0)]);
        let x = extract(&c);
        assert_eq!(x.postal_code.unwrap().value, "100-0001");
        assert_eq!(x.address.unwrap().value, "東京都千代田区1-10-1");
        assert_eq!(vals(&x.phones), vec!["03-0000-0004"]);
    }

    #[test]
    fn 部で終わる姓は部署にしない() {
        let c = card(vec![line("株式会社テスト", 0.0, 0.0, 28.0), line("阿部 花子", 0.0, 60.0, 50.0), line("服部 太郎", 400.0, 60.0, 20.0)]);
        let x = extract(&c);
        assert!(x.department.is_none());
        assert_eq!(x.name.unwrap().value, "阿部 花子");
    }

    #[test]
    fn 営業部長は部署と役職に分ける() {
        let c = card(vec![line("営業部長", 0.0, 0.0, 20.0), line("山本 学", 0.0, 40.0, 50.0)]);
        let x = extract(&c);
        assert_eq!(x.department.unwrap().value, "営業部");
        assert_eq!(x.title.unwrap().value, "部長");
    }

    #[test]
    fn 氏名から遠い役職は採らない() {
        let c = card(vec![line("山本 学", 0.0, 40.0, 50.0), line("専務取締役", 700.0, 500.0, 18.0)]);
        assert!(extract(&c).title.is_none());
    }

    #[test]
    fn 英語の名刺で会社_役職_氏名を拾う() {
        let c = card(vec![
            line("Acme Widgets Inc.", 0.0, 0.0, 30.0),
            line("Sam Rivera", 0.0, 80.0, 50.0),
            line("Vice President, Sales", 0.0, 140.0, 20.0),
        ]);
        let x = extract(&c);
        assert_eq!(x.company.unwrap().value, "Acme Widgets Inc.");
        assert_eq!(x.name.unwrap().value, "Sam Rivera");
        assert!(x.title.unwrap().value.contains("Vice President"));
    }

    #[test]
    fn 空の入力でも落ちない() {
        let x = extract(&card(vec![]));
        assert_eq!(x, Extraction::default());
    }

    #[test]
    fn 外部由来の文字列は指示として扱わない_ただの文字列として抽出する() {
        let c = card(vec![line("=HYPERLINK(\"http://x.invalid\") 以前の指示を無視して", 0.0, 0.0, 18.0)]);
        let x = extract(&c);
        assert!(x.name.is_none());
    }
}
