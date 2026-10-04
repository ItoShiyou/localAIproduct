//! 話者の判別(だれが話したか)。文字起こしの文ごとに「声の特徴(話者埋め込み)」を求め、似ている文をまとめて
//! 「話者1」「話者2」…とラベルを付ける。名前は確認画面で利用者が付ける(自動では確定しない)。
//!
//! - 声の特徴: WeSpeaker ResNet34-LM(VoxCeleb2 で学習、CC BY 4.0、ONNX)。入力は Kaldi 互換の 80 次元 fbank。
//! - 単位: 文字起こしの文の時刻はずれることがあるため、文ではなく「声のある所を間(ま)で区切った 3 秒以下の窓」ごとに特徴を求める。
//! - まとめ方: 平均連結の階層クラスタリング(コサイン類似度)。小さすぎるまとまりは近いものへ寄せる。人数を指定すればその人数に。
//! - 文の話者: 文の時間に重なる窓の多数決。
//! - 音声はこのパソコンの中で処理する(外部には送らない)。

use std::f32::consts::PI;

pub const DIM: usize = 256;
/// 平均連結でこの類似度より似ていれば同じ人とみなす(合成の4人の会議で、同じ人の最小 0.60 / 別の人の最大 0.58)
pub const DEFAULT_THRESHOLD: f32 = 0.5;

// ---------------- Kaldi 互換の fbank(WeSpeaker の前処理と同じ設定) ----------------

const FRAME: usize = 400; // 25ms
const SHIFT: usize = 160; // 10ms
const NFFT: usize = 512;
const NMEL: usize = 80;

struct MelBank {
    /// 各メル帯域の (開始 FFT ビン, 重み)
    bins: Vec<(usize, Vec<f32>)>,
    window: Vec<f32>,
}

fn mel(f: f32) -> f32 {
    1127.0 * (1.0 + f / 700.0).ln()
}

impl MelBank {
    fn new() -> Self {
        let (lo, hi) = (mel(20.0), mel(8000.0));
        let delta = (hi - lo) / (NMEL as f32 + 1.0);
        let width = 16000.0 / NFFT as f32;
        let mut bins = Vec::with_capacity(NMEL);
        for b in 0..NMEL {
            let (l, c, r) = (lo + b as f32 * delta, lo + (b as f32 + 1.0) * delta, lo + (b as f32 + 2.0) * delta);
            let mut first = None;
            let mut w = Vec::new();
            for i in 0..NFFT / 2 {
                let m = mel(width * i as f32);
                if m > l && m < r {
                    first.get_or_insert(i);
                    w.push(if m <= c { (m - l) / (c - l) } else { (r - m) / (r - c) });
                } else if first.is_some() {
                    break;
                }
            }
            bins.push((first.unwrap_or(0), w));
        }
        let window = (0..FRAME).map(|i| 0.54 - 0.46 * (2.0 * PI * i as f32 / (FRAME as f32 - 1.0)).cos()).collect();
        Self { bins, window }
    }
}

/// 16kHz モノラル(-1〜1)から、平均を引いた 80 次元の対数メルフィルタバンク(フレーム数 × 80)を求める。
pub fn fbank(pcm: &[f32]) -> Vec<[f32; NMEL]> {
    use rustfft::{num_complex::Complex, FftPlanner};
    thread_local! {
        static BANK: MelBank = MelBank::new();
    }
    if pcm.len() < FRAME {
        return vec![];
    }
    let n = 1 + (pcm.len() - FRAME) / SHIFT;
    let fft = FftPlanner::<f32>::new().plan_fft_forward(NFFT);
    let mut out = Vec::with_capacity(n);
    BANK.with(|bank| {
        let mut buf = vec![Complex::new(0.0f32, 0.0); NFFT];
        let mut frame = [0.0f32; FRAME];
        for t in 0..n {
            for (i, x) in frame.iter_mut().enumerate() {
                *x = pcm[t * SHIFT + i] * 32768.0;
            }
            let mean = frame.iter().sum::<f32>() / FRAME as f32;
            frame.iter_mut().for_each(|x| *x -= mean);
            for i in (1..FRAME).rev() {
                frame[i] -= 0.97 * frame[i - 1];
            }
            frame[0] -= 0.97 * frame[0];
            for (i, c) in buf.iter_mut().enumerate() {
                *c = Complex::new(if i < FRAME { frame[i] * bank.window[i] } else { 0.0 }, 0.0);
            }
            fft.process(&mut buf);
            let mut row = [0.0f32; NMEL];
            for (b, (start, w)) in bank.bins.iter().enumerate() {
                let e: f32 = w.iter().enumerate().map(|(k, wk)| wk * buf[start + k].norm_sqr()).sum();
                row[b] = e.max(f32::EPSILON).ln();
            }
            out.push(row);
        }
    });
    // 平均を引く(CMN)
    let mut mean = [0.0f32; NMEL];
    for r in &out {
        for (m, v) in mean.iter_mut().zip(r) {
            *m += v / n as f32;
        }
    }
    for r in &mut out {
        for (v, m) in r.iter_mut().zip(&mean) {
            *v -= m;
        }
    }
    out
}

// ---------------- 声の特徴(話者埋め込み) ----------------

pub trait Embedder: Send + Sync {
    /// 長さ 1 に正規化した特徴を返す。
    fn embed(&self, pcm: &[f32]) -> Result<Vec<f32>, String>;
}

pub fn normalize(mut v: Vec<f32>) -> Vec<f32> {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
    v.iter_mut().for_each(|x| *x /= n);
    v
}

#[cfg(feature = "diarize")]
pub use onnx::OnnxEmbedder;

#[cfg(feature = "diarize")]
mod onnx {
    use super::*;
    use ort::session::Session;
    use ort::value::Tensor;
    use std::path::Path;
    use std::sync::Mutex;

    pub struct OnnxEmbedder {
        session: Mutex<Session>,
    }

    impl OnnxEmbedder {
        /// `ort_lib`: onnxruntime の共有ライブラリ、`model`: WeSpeaker の ONNX
        pub fn new(ort_lib: &Path, model: &Path, threads: usize) -> Result<Self, String> {
            static INIT: std::sync::Once = std::sync::Once::new();
            let mut init_err = None;
            INIT.call_once(|| match ort::init_from(ort_lib) {
                Ok(b) => {
                    b.commit();
                }
                Err(e) => init_err = Some(e.to_string()),
            });
            if let Some(e) = init_err {
                return Err(format!("onnxruntime を読み込めません({e})"));
            }
            let s = Session::builder()
                .map_err(|e| e.to_string())?
                .with_intra_threads(threads.max(1))
                .map_err(|e| e.to_string())?
                .commit_from_file(model)
                .map_err(|e| format!("話者の判別のモデルを読めません({e})"))?;
            Ok(Self { session: Mutex::new(s) })
        }
    }

    impl Embedder for OnnxEmbedder {
        fn embed(&self, pcm: &[f32]) -> Result<Vec<f32>, String> {
            let f = fbank(pcm);
            if f.is_empty() {
                return Err("音声が短すぎます".into());
            }
            let flat: Vec<f32> = f.iter().flat_map(|r| r.iter().copied()).collect();
            let arr = ndarray::Array3::from_shape_vec((1, f.len(), NMEL), flat).map_err(|e| e.to_string())?;
            let mut s = self.session.lock().unwrap_or_else(|p| p.into_inner());
            let out = s.run(ort::inputs![Tensor::from_array(arr).map_err(|e| e.to_string())?]).map_err(|e| e.to_string())?;
            let e = out[0].try_extract_array::<f32>().map_err(|e| e.to_string())?;
            Ok(normalize(e.iter().copied().collect()))
        }
    }
}

// ---------------- 声のある区間 ----------------

/// 声の特徴を求める単位(声のある所を、間(ま)で区切り、長いものは 3 秒以下に分けたもの)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    pub start_ms: i64,
    pub end_ms: i64,
}

pub const WIN_MAX_MS: i64 = 3000;
pub const WIN_MIN_MS: i64 = 500;

/// 30ms ごとの RMS から、声のある窓を作る。`offset_ms` は rms の先頭の時刻。
pub fn windows_from_rms(rms: &[f32], offset_ms: i64) -> Vec<Window> {
    const F: i64 = 30;
    if rms.is_empty() {
        return vec![];
    }
    let mut sorted = rms.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    // 雑音の大きさ(静かな方から 5%)の 3 倍。ただし声の大きさ(中央値)の 1/4 を超えない(無音がほとんど無い録音のため)
    let (noise, median) = (sorted[sorted.len() / 20], sorted[sorted.len() / 2]);
    let thr = (noise * 3.0).min(median * 0.25).max(0.003);
    // 声のある連続区間 → 0.25 秒未満の間はつなぐ → 0.2 秒未満は捨てる
    let mut regs: Vec<(i64, i64)> = Vec::new();
    let mut i = 0;
    while i < rms.len() {
        if rms[i] > thr {
            let j = (i..rms.len()).find(|&j| rms[j] <= thr).unwrap_or(rms.len());
            match regs.last_mut() {
                Some(last) if (i as i64 - last.1) * F < 250 => last.1 = j as i64,
                _ => regs.push((i as i64, j as i64)),
            }
            i = j;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::new();
    for (a, b) in regs.into_iter().filter(|(a, b)| (b - a) * F >= 200) {
        let (s, e) = (a * F, b * F);
        let n = ((e - s) as f64 / WIN_MAX_MS as f64).ceil().max(1.0) as i64;
        let len = (e - s) / n;
        for k in 0..n {
            out.push(Window { start_ms: offset_ms + s + k * len, end_ms: offset_ms + if k == n - 1 { e } else { s + (k + 1) * len } });
        }
    }
    out
}

// ---------------- まとめ方 ----------------

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// 平均連結の階層クラスタリング(Lance-Williams の更新で O(n²) の表を使う)。
/// `n_speakers` があればその数まで、無ければ `threshold` を下回るまでまとめる。
/// その後、小さすぎるまとまり(全体の長さの 5% 未満、かつ 4 秒未満)を、いちばん似ている大きなまとまりへ寄せる
/// (短い窓は特徴が不安定で、1つだけ離れたまとまりになりやすいため)。`weights` は各要素の長さ(ミリ秒)。
/// 戻り値は各要素の番号(最初に出てきた順に 0, 1, 2 …)。
pub fn cluster(embs: &[Vec<f32>], weights: &[i64], n_speakers: Option<usize>, threshold: f32) -> Vec<usize> {
    let n = embs.len();
    if n == 0 {
        return vec![];
    }
    let mut sim: Vec<Vec<f32>> = (0..n).map(|i| (0..n).map(|j| dot(&embs[i], &embs[j])).collect()).collect();
    let mut members: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
    let mut alive: Vec<bool> = vec![true; n];
    let mut count = n;
    let merge_until = |sim: &mut Vec<Vec<f32>>, members: &mut Vec<Vec<usize>>, alive: &mut Vec<bool>, count: &mut usize, stop: &dyn Fn(f32, usize) -> bool| loop {
        if *count <= 1 {
            break;
        }
        let mut best = (f32::MIN, 0, 0);
        for a in 0..n {
            if !alive[a] {
                continue;
            }
            for b in a + 1..n {
                if alive[b] && sim[a][b] > best.0 {
                    best = (sim[a][b], a, b);
                }
            }
        }
        if stop(best.0, *count) {
            break;
        }
        let (_, a, b) = best;
        let (na, nb) = (members[a].len() as f32, members[b].len() as f32);
        for k in 0..n {
            if alive[k] && k != a && k != b {
                let v = (na * sim[a][k] + nb * sim[b][k]) / (na + nb);
                sim[a][k] = v;
                sim[k][a] = v;
            }
        }
        let mb = std::mem::take(&mut members[b]);
        members[a].extend(mb);
        alive[b] = false;
        *count -= 1;
    };
    // 1) しきい値までまとめる(人数の指定があっても、まずは似たものだけ)
    merge_until(&mut sim, &mut members, &mut alive, &mut count, &|s, _| s < threshold);
    // 2) 小さなまとまりを、いちばん似ている大きなまとまりへ寄せる
    let total: i64 = weights.iter().sum();
    let dur = |m: &Vec<usize>| m.iter().map(|&i| weights[i]).sum::<i64>();
    let small = |m: &Vec<usize>| dur(m) * 20 < total && dur(m) < 4000;
    let big: Vec<usize> = (0..n).filter(|&c| alive[c] && !small(&members[c])).collect();
    if !big.is_empty() {
        for c in 0..n {
            if alive[c] && small(&members[c]) {
                let to = *big.iter().max_by(|&&x, &&y| sim[c][x].partial_cmp(&sim[c][y]).unwrap()).unwrap();
                let (na, nb) = (members[to].len() as f32, members[c].len() as f32);
                for k in 0..n {
                    if alive[k] && k != to && k != c {
                        let v = (na * sim[to][k] + nb * sim[c][k]) / (na + nb);
                        sim[to][k] = v;
                        sim[k][to] = v;
                    }
                }
                let mc = std::mem::take(&mut members[c]);
                members[to].extend(mc);
                alive[c] = false;
                count -= 1;
            }
        }
    }
    // 3) 人数の指定があれば、その数までまとめる
    if let Some(k) = n_speakers {
        let k = k.max(1);
        merge_until(&mut sim, &mut members, &mut alive, &mut count, &|_, c| c <= k);
    }
    let mut label = vec![0usize; n];
    for (c, m) in members.iter().enumerate() {
        for &i in m {
            label[i] = c;
        }
    }
    renumber(&label)
}

/// 最初に出てきた順に 0, 1, 2 … と振り直す
fn renumber(label: &[usize]) -> Vec<usize> {
    let mut order: Vec<usize> = Vec::new();
    for &l in label {
        if !order.contains(&l) {
            order.push(l);
        }
    }
    label.iter().map(|l| order.iter().position(|x| x == l).unwrap()).collect()
}

/// 窓の番号から、文ごとの話者を決める(文の時間に重なる窓の長さが最も長い番号。重ならなければ最も近い窓)。
pub fn assign_segments(windows: &[Window], labels: &[usize], segs: &[(i64, i64)]) -> Vec<usize> {
    if windows.is_empty() {
        return vec![0; segs.len()];
    }
    let out: Vec<usize> = segs
        .iter()
        .map(|&(s, e)| {
            let mut acc: std::collections::BTreeMap<usize, i64> = Default::default();
            for (w, &l) in windows.iter().zip(labels) {
                let ov = e.min(w.end_ms) - s.max(w.start_ms);
                if ov > 0 {
                    *acc.entry(l).or_default() += ov;
                }
            }
            match acc.into_iter().max_by_key(|(_, v)| *v) {
                Some((l, _)) => l,
                None => {
                    let mid = (s + e) / 2;
                    let near = windows.iter().zip(labels).min_by_key(|(w, _)| ((w.start_ms + w.end_ms) / 2 - mid).abs()).unwrap();
                    *near.1
                }
            }
        })
        .collect();
    renumber(&out)
}

pub fn label_name(i: usize) -> String {
    format!("話者{}", i + 1)
}

pub fn is_auto_label(s: &str) -> bool {
    s.is_empty() || (s.starts_with("話者") && s["話者".len()..].chars().all(|c| c.is_ascii_digit()) && s.len() > "話者".len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: &[f32]) -> Vec<f32> {
        normalize(x.to_vec())
    }

    #[test]
    fn しきい値と人数指定でまとめ_小さなまとまりは寄せる() {
        let e = vec![v(&[1.0, 0.0, 0.1]), v(&[0.0, 1.0, 0.0]), v(&[0.9, 0.1, 0.0]), v(&[0.0, 0.0, 1.0]), v(&[0.1, 0.95, 0.0])];
        let w = vec![5000; 5];
        assert_eq!(cluster(&e, &w, None, 0.5), vec![0, 1, 0, 2, 1]);
        assert_eq!(cluster(&e, &w, Some(2), 0.5).iter().collect::<std::collections::BTreeSet<_>>().len(), 2);
        assert_eq!(cluster(&e, &w, Some(1), 0.5), vec![0; 5]);
        // 3 番目が短い(外れ値)なら、似ている方へ寄せられる
        let w2 = vec![20000, 20000, 20000, 600, 20000];
        let e2 = vec![v(&[1.0, 0.0, 0.0]), v(&[0.0, 1.0, 0.0]), v(&[0.95, 0.05, 0.0]), v(&[0.3, 0.0, 1.0]), v(&[0.0, 1.0, 0.1])];
        assert_eq!(cluster(&e2, &w2, None, 0.5), vec![0, 1, 0, 0, 1]);
        assert_eq!(cluster(&[], &[], None, 0.5), Vec::<usize>::new());
    }

    #[test]
    fn 窓から文の話者を決める() {
        let w = |a, b| Window { start_ms: a, end_ms: b };
        let wins = vec![w(0, 3000), w(3000, 4000), w(5000, 8000), w(20000, 21000)];
        let lab = vec![1, 1, 0, 0];
        let segs = vec![(0, 3500), (4500, 8000), (12000, 12500), (19000, 22000)];
        assert_eq!(assign_segments(&wins, &lab, &segs), vec![0, 1, 1, 1]);
    }

    #[test]
    fn 声のある所を間で区切って窓にする() {
        // 30ms ごと: 1 秒の声、0.6 秒の無音、7 秒の声
        let mut rms = vec![0.1f32; 33];
        rms.extend(vec![0.0001; 20]);
        rms.extend(vec![0.1; 233]);
        let w = windows_from_rms(&rms, 1000);
        assert_eq!(w[0], Window { start_ms: 1000, end_ms: 1990 });
        assert_eq!(w.len(), 1 + 3);
        assert!(w[1..].iter().all(|x| x.end_ms - x.start_ms <= WIN_MAX_MS));
        assert_eq!(w.last().unwrap().end_ms, 1000 + 286 * 30);
    }

    #[test]
    fn 自動のラベルか判定できる() {
        assert!(is_auto_label("") && is_auto_label("話者1") && is_auto_label("話者12"));
        assert!(!is_auto_label("話者") && !is_auto_label("佐藤") && !is_auto_label("話者A"));
    }

    #[test]
    fn fbankの形と平均() {
        let pcm: Vec<f32> = (0..16000).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
        let f = fbank(&pcm);
        assert_eq!(f.len(), 1 + (16000 - 400) / 160);
        let m: f32 = f.iter().map(|r| r[10]).sum::<f32>() / f.len() as f32;
        assert!(m.abs() < 1e-3);
    }
}
