//! 音声の読み込み(symphonia)・16kHz モノラルへの変換・ノイズ除去(RNNoise の Rust 版 nnnoiseless)・区間の分割。
//!
//! - 長い録音(1〜2時間)でもメモリを使いすぎないよう、読み込みと変換は少しずつ(ストリーム)行い、
//!   16kHz モノラルの i16 でディスクに書く(2時間で約 230MB)。
//! - 区間は「最長 30 秒、その手前 10 秒のうち一番静かな所で切る」。ほぼ無音の区間は飛ばす(文字起こししない)。

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub const RATE: u32 = 16_000;

/// 区間化のための窓付き sinc による、少しずつ入力できるリサンプラ。
pub struct Resampler {
    ratio: f64, // 入力 / 出力
    cutoff: f64,
    buf: Vec<f32>,
    pos: f64, // 次の出力の位置(buf 上の添字)
}

const HALF_TAPS: isize = 16;

impl Resampler {
    pub fn new(from: u32, to: u32) -> Self {
        let ratio = from as f64 / to as f64;
        // 間引きのときは折り返しを防ぐため遮断周波数を下げる
        let cutoff = if ratio > 1.0 { 0.95 / ratio } else { 0.95 };
        Self { ratio, cutoff, buf: vec![0.0; HALF_TAPS as usize], pos: HALF_TAPS as f64 }
    }

    fn kernel(&self, x: f64) -> f64 {
        let w = x / (HALF_TAPS as f64 + 1.0);
        if w.abs() >= 1.0 {
            return 0.0;
        }
        let win = 0.5 * (1.0 + (std::f64::consts::PI * w).cos()); // Hann
        let a = std::f64::consts::PI * self.cutoff * x;
        let sinc = if a.abs() < 1e-9 { 1.0 } else { a.sin() / a };
        self.cutoff * sinc * win
    }

    /// 入力を足し、出せるだけ出力する。
    pub fn push(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if (self.ratio - 1.0).abs() < 1e-12 {
            out.extend_from_slice(input);
            return;
        }
        self.buf.extend_from_slice(input);
        let limit = self.buf.len() as f64 - HALF_TAPS as f64 - 1.0;
        while self.pos < limit {
            let c = self.pos.floor() as isize;
            let frac = self.pos - c as f64;
            let mut acc = 0.0f64;
            for k in -HALF_TAPS..=HALF_TAPS {
                let i = c + k;
                acc += self.buf[i as usize] as f64 * self.kernel(k as f64 - frac);
            }
            out.push(acc as f32);
            self.pos += self.ratio;
        }
        // 使い終わった入力を捨てる(履歴は残す)
        let drop = (self.pos.floor() as isize - HALF_TAPS).max(0) as usize;
        if drop > 0 {
            self.buf.drain(..drop);
            self.pos -= drop as f64;
        }
    }

    /// 入力の終わり。残りを出す。
    pub fn finish(&mut self, out: &mut Vec<f32>) {
        if (self.ratio - 1.0).abs() < 1e-12 {
            return;
        }
        let pad = vec![0.0; HALF_TAPS as usize + 2];
        self.push(&pad, out);
    }
}

/// 音声ファイルを読み、16kHz モノラル i16 の生データとして `dest` に書く。再生時間(ミリ秒)を返す。
/// 動画(mp4 など)は、最初の音声トラックだけを読む。
pub fn decode_to_pcm16k(src: &Path, dest: &Path) -> Result<u64, String> {
    let file = File::open(src).map_err(|_| "音声ファイルを開けません".to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = src.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|_| "対応していない形式か、壊れたファイルです(m4a / mp3 / wav / mp4 など)".to_string())?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.sample_rate.is_some() && t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or("音声のトラックが見つかりません")?;
    let track_id = track.id;
    let rate = track.codec_params.sample_rate.unwrap_or(RATE);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|_| "この音声の形式(コーデック)には対応していません".to_string())?;

    let mut rs = Resampler::new(rate, RATE);
    let mut w = BufWriter::new(File::create(dest).map_err(|_| "作業用のファイルを作れません".to_string())?);
    let mut mono = Vec::new();
    let mut out = Vec::new();
    let mut written: u64 = 0;
    let mut write = |out: &mut Vec<f32>, w: &mut BufWriter<File>| -> Result<(), String> {
        for s in out.drain(..) {
            let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
            w.write_all(&v.to_le_bytes()).map_err(|_| "作業用のファイルに書けません(空き容量を確認してください)".to_string())?;
            written += 1;
        }
        Ok(())
    };
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(_) => break,
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymError::DecodeError(_)) => continue, // 壊れたパケットは飛ばす
            Err(_) => break,
        };
        let spec = *decoded.spec();
        let ch = spec.channels.count().max(1);
        let mut sb = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        sb.copy_interleaved_ref(decoded);
        mono.clear();
        mono.extend(sb.samples().chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32));
        rs.push(&mono, &mut out);
        write(&mut out, &mut w)?;
    }
    rs.finish(&mut out);
    write(&mut out, &mut w)?;
    w.flush().map_err(|e| e.to_string())?;
    if written == 0 {
        return Err("音声を読み取れませんでした(無音か、対応していない形式です)".into());
    }
    Ok(written * 1000 / RATE as u64)
}

/// 16kHz モノラル i16 の生データから、[start_ms, end_ms) を f32(-1〜1)で読む。
pub fn read_pcm16k(path: &Path, start_ms: u64, end_ms: u64) -> Result<Vec<f32>, String> {
    use std::io::{Seek, SeekFrom};
    let mut f = BufReader::new(File::open(path).map_err(|_| "作業用の音声を読めません".to_string())?);
    let s = start_ms * RATE as u64 / 1000;
    let e = end_ms * RATE as u64 / 1000;
    f.seek(SeekFrom::Start(s * 2)).map_err(|e| e.to_string())?;
    let mut bytes = vec![0u8; ((e - s) * 2) as usize];
    let n = read_full(&mut f, &mut bytes)?;
    Ok(bytes[..n - n % 2].chunks(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect())
}

fn read_full(r: &mut impl Read, buf: &mut [u8]) -> Result<usize, String> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(n)
}

/// ノイズ除去(RNNoise / nnnoiseless)。16kHz を 48kHz に上げて処理し、16kHz に戻す。
pub fn denoise_16k(pcm: &[f32]) -> Vec<f32> {
    use nnnoiseless::DenoiseState;
    let fs = DenoiseState::FRAME_SIZE;
    let mut up = Vec::with_capacity(pcm.len() * 3 + 64);
    let mut r = Resampler::new(RATE, 48_000);
    r.push(pcm, &mut up);
    r.finish(&mut up);
    // 完全な無音が続くと処理が不安定になる例があったため(DFN3 のスパイク)、ごく小さな揺らぎを足す
    let mut state = DenoiseState::new();
    let mut out48 = Vec::with_capacity(up.len() + fs);
    let mut inb = vec![0.0f32; fs];
    let mut outb = vec![0.0f32; fs];
    // 末尾に 1 フレーム足して流し切る(RNNoise の出力は 1 フレーム(480 サンプル)遅れるため)
    let total = up.len() + fs;
    let mut n = 0;
    while n < total {
        for (i, x) in inb.iter_mut().enumerate() {
            let dither = if (n + i) % 2 == 0 { 0.5 } else { -0.5 };
            *x = up.get(n + i).copied().unwrap_or(0.0) * 32768.0 + dither;
        }
        state.process_frame(&mut outb, &inb);
        out48.extend(outb.iter().map(|x| x / 32768.0));
        n += fs;
    }
    // 遅れの分を捨てて、入力と時刻をそろえる
    out48.drain(..fs.min(out48.len()));
    out48.truncate(up.len());
    let mut down = Vec::with_capacity(pcm.len() + 64);
    let mut r = Resampler::new(48_000, RATE);
    r.push(&out48, &mut down);
    r.finish(&mut down);
    down.truncate(pcm.len());
    down
}

/// 16kHz モノラル i16 の生データを WAV として書き出す(ノイズ除去後の音声の書き出し用)。
pub fn write_wav16(dest: &Path, pcm: &[f32]) -> Result<(), String> {
    let mut w = BufWriter::new(File::create(dest).map_err(|_| "書き出し先に保存できません".to_string())?);
    let data_len = (pcm.len() * 2) as u32;
    let mut h = Vec::with_capacity(44);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + data_len).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes()); // PCM
    h.extend_from_slice(&1u16.to_le_bytes()); // mono
    h.extend_from_slice(&RATE.to_le_bytes());
    h.extend_from_slice(&(RATE * 2).to_le_bytes());
    h.extend_from_slice(&2u16.to_le_bytes());
    h.extend_from_slice(&16u16.to_le_bytes());
    h.extend_from_slice(b"data");
    h.extend_from_slice(&data_len.to_le_bytes());
    w.write_all(&h).map_err(|e| e.to_string())?;
    for s in pcm {
        w.write_all(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()).map_err(|e| e.to_string())?;
    }
    w.flush().map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Chunk {
    pub start_ms: u64,
    pub end_ms: u64,
    /// ほぼ無音(文字起こししない)
    pub silent: bool,
}

pub const MAX_CHUNK_MS: u64 = 30_000;
const SEARCH_MS: u64 = 10_000;
const FRAME_MS: u64 = 30;
/// これより小さい RMS(-1〜1 のスケール)の区間は無音とみなす(約 -50 dBFS)
const SILENCE_RMS: f32 = 0.003;

/// 区間に分ける。`frame_rms` は 30ms ごとの RMS(`frame_rms_of` で作る)。
pub fn split_chunks(frame_rms: &[f32]) -> Vec<Chunk> {
    let total_ms = frame_rms.len() as u64 * FRAME_MS;
    let mut out = Vec::new();
    let mut start = 0u64;
    while start < total_ms {
        let mut end = (start + MAX_CHUNK_MS).min(total_ms);
        if end < total_ms {
            // 最後の 10 秒のうち、一番静かな所(300ms の移動平均が最小)で切る
            let a = ((end - SEARCH_MS) / FRAME_MS) as usize;
            let b = (end / FRAME_MS) as usize;
            let mut best = (f32::MAX, b);
            for i in a..b {
                let lo = i.saturating_sub(5);
                let hi = (i + 5).min(frame_rms.len());
                let m = frame_rms[lo..hi].iter().sum::<f32>() / (hi - lo) as f32;
                if m < best.0 {
                    best = (m, i);
                }
            }
            end = best.1 as u64 * FRAME_MS;
        }
        let a = (start / FRAME_MS) as usize;
        let b = ((end / FRAME_MS) as usize).min(frame_rms.len());
        let voiced = frame_rms[a..b].iter().filter(|r| **r >= SILENCE_RMS).count();
        // 声らしいフレームが 0.3 秒分も無ければ無音とみなす
        out.push(Chunk { start_ms: start, end_ms: end, silent: (voiced as u64) * FRAME_MS < 300 });
        start = end;
    }
    out
}

/// 16kHz モノラル i16 の生データ全体から、30ms ごとの RMS を少しずつ計算する。
pub fn frame_rms_of(path: &Path) -> Result<Vec<f32>, String> {
    let mut f = BufReader::new(File::open(path).map_err(|_| "作業用の音声を読めません".to_string())?);
    let n = (RATE as u64 * FRAME_MS / 1000) as usize;
    let mut bytes = vec![0u8; n * 2];
    let mut out = Vec::new();
    loop {
        let k = read_full(&mut f, &mut bytes)?;
        if k < 2 {
            break;
        }
        let s: f32 = bytes[..k - k % 2].chunks(2).map(|b| (i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).powi(2)).sum();
        out.push((s / (k / 2) as f32).sqrt());
        if k < bytes.len() {
            break;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(n: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("min-audio-{n}-{}", std::process::id()))
    }

    fn sine(rate: u32, hz: f32, secs: f32) -> Vec<f32> {
        (0..(rate as f32 * secs) as usize).map(|i| (i as f32 / rate as f32 * hz * std::f32::consts::TAU).sin() * 0.5).collect()
    }

    #[test]
    fn リサンプラは長さと周波数を保ち_少しずつ入れても同じ() {
        let x = sine(48_000, 440.0, 1.0);
        let mut a = Vec::new();
        let mut r = Resampler::new(48_000, 16_000);
        r.push(&x, &mut a);
        r.finish(&mut a);
        let mut b = Vec::new();
        let mut r = Resampler::new(48_000, 16_000);
        for c in x.chunks(777) {
            r.push(c, &mut b);
        }
        r.finish(&mut b);
        assert!((a.len() as i64 - 16_000).abs() < 20, "{}", a.len());
        assert_eq!(a.len(), b.len());
        assert!(a.iter().zip(&b).all(|(p, q)| (p - q).abs() < 1e-5));
        // 440Hz のゼロ交差は 1 秒に約 880 回
        let zc = a.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
        assert!((870..=890).contains(&zc), "{zc}");
        // 間引きで 12kHz の音(16kHz では表せない)は十分に弱まる
        let hi = sine(48_000, 12_000.0, 0.5);
        let mut o = Vec::new();
        let mut r = Resampler::new(48_000, 16_000);
        r.push(&hi, &mut o);
        let rms = (o[100..o.len() - 100].iter().map(|v| v * v).sum::<f32>() / (o.len() - 200) as f32).sqrt();
        assert!(rms < 0.02, "{rms}");
    }

    #[test]
    fn wavを読み_16kのpcmにして_区間と無音を判定できる() {
        // 前半 5 秒は無音、後半 40 秒は音のある 44.1kHz ステレオ WAV
        let rate = 44_100;
        let mut mono = vec![0.0f32; rate as usize * 5];
        mono.extend(sine(rate, 300.0, 40.0));
        let src = tmp("in.wav");
        {
            // 2ch で書く
            let mut w = BufWriter::new(File::create(&src).unwrap());
            let data_len = (mono.len() * 4) as u32;
            let mut h = Vec::new();
            h.extend_from_slice(b"RIFF");
            h.extend_from_slice(&(36 + data_len).to_le_bytes());
            h.extend_from_slice(b"WAVEfmt ");
            h.extend_from_slice(&16u32.to_le_bytes());
            h.extend_from_slice(&1u16.to_le_bytes());
            h.extend_from_slice(&2u16.to_le_bytes());
            h.extend_from_slice(&rate.to_le_bytes());
            h.extend_from_slice(&(rate * 4).to_le_bytes());
            h.extend_from_slice(&4u16.to_le_bytes());
            h.extend_from_slice(&16u16.to_le_bytes());
            h.extend_from_slice(b"data");
            h.extend_from_slice(&data_len.to_le_bytes());
            w.write_all(&h).unwrap();
            for s in &mono {
                let v = ((s * 32767.0) as i16).to_le_bytes();
                w.write_all(&v).unwrap();
                w.write_all(&v).unwrap();
            }
        }
        let pcm = tmp("out.pcm");
        let ms = decode_to_pcm16k(&src, &pcm).unwrap();
        assert!((ms as i64 - 45_000).abs() < 50, "{ms}");
        let rms = frame_rms_of(&pcm).unwrap();
        let chunks = split_chunks(&rms);
        assert_eq!(chunks.first().unwrap().start_ms, 0);
        assert!((chunks.last().unwrap().end_ms as i64 - 45_000).abs() < 50);
        assert!(chunks.windows(2).all(|w| w[0].end_ms == w[1].start_ms));
        assert!(chunks.iter().all(|c| c.end_ms - c.start_ms <= MAX_CHUNK_MS));
        assert!(chunks.iter().any(|c| !c.silent));
        let head = read_pcm16k(&pcm, 0, 4_000).unwrap();
        assert_eq!(head.len(), 64_000);
        assert!(head.iter().all(|v| v.abs() < 1e-3));
        std::fs::remove_file(&src).ok();
        std::fs::remove_file(&pcm).ok();
    }

    #[test]
    fn 無音だけの区間は無音と判定する() {
        let rms = vec![0.0001f32; (70_000 / FRAME_MS) as usize];
        let c = split_chunks(&rms);
        assert!(c.len() >= 3 && c.iter().all(|c| c.silent));
    }

    fn si_sdr(est: &[f32], refr: &[f32]) -> f64 {
        let n = est.len().min(refr.len());
        let (e, r) = (&est[..n], &refr[..n]);
        let dot: f64 = e.iter().zip(r).map(|(a, b)| *a as f64 * *b as f64).sum();
        let rr: f64 = r.iter().map(|b| (*b as f64).powi(2)).sum();
        let a = dot / rr;
        let (mut s, mut d) = (0.0, 0.0);
        for (x, y) in e.iter().zip(r) {
            let t = a * *y as f64;
            s += t * t;
            d += (*x as f64 - t).powi(2);
        }
        10.0 * (s / d).log10()
    }

    #[test]
    fn ノイズ除去は長さを保ち_テストセットの雑音入り音声を基準に近づける() {
        let ts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testset");
        let (noisy, clean) = (tmp("n.pcm"), tmp("c.pcm"));
        decode_to_pcm16k(&ts.join("t02_aircon.wav"), &noisy).unwrap();
        decode_to_pcm16k(&ts.join("t01_clean_ref.wav"), &clean).unwrap();
        let x = read_pcm16k(&noisy, 0, 60_000).unwrap();
        let r = read_pcm16k(&clean, 0, 60_000).unwrap();
        let y = denoise_16k(&x);
        assert_eq!(y.len(), x.len());
        let (before, after) = (si_sdr(&x, &r), si_sdr(&y, &r));
        eprintln!("SI-SDR(t02 エアコン、先頭60秒): {before:.1} dB → {after:.1} dB");
        assert!(after > before + 3.0);
        std::fs::remove_file(&noisy).ok();
        std::fs::remove_file(&clean).ok();
    }

    #[test]
    fn wavの書き出しは読み直せる() {
        let p = tmp("w.wav");
        let x = sine(RATE, 200.0, 0.5);
        write_wav16(&p, &x).unwrap();
        let q = tmp("w.pcm");
        assert_eq!(decode_to_pcm16k(&p, &q).unwrap(), 500);
        std::fs::remove_file(&p).ok();
        std::fs::remove_file(&q).ok();
    }
}
