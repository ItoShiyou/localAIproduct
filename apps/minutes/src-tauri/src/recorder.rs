//! アプリ内のリアルタイム録音と文字起こし。
//!
//! - 画面がマイクの音(16kHz モノラル)を少しずつ渡す → 作業用の生データに追記する。
//! - 声の切れ目(0.6 秒以上の無音)で区間を切り(4 秒未満は切らない、25 秒を超えたら切る)、
//!   区間ごとに別スレッドで文字起こしして、ふつうの取り込みと同じ「区間」として保存する(画面は議事録を読み直すだけで追える)。
//! - 録音中は速い小さなモデルで「仮の文字」を出す(10〜20 秒ほど遅れる、精度は低め)。仮の文字の区間番号は負の数。
//! - 停止したら WAV に書き出し(再生用)、正確なモデルでの文字起こしを待ちに入れる。仮の文字は、正確な結果ができた区間から
//!   順に置き換わる(待っている間も文字が消えない)。話者の判別も正確な文字起こしのあとに行う。
//! - 録音はこのパソコンの中にだけ保存する(外部に送らない)。

use crate::asr::Asr;
use crate::audio::{self, RATE};
use crate::pipeline;
use crate::store::Store;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

const FRAME: usize = 480; // 30ms
const MIN_CHUNK_MS: u64 = 4_000;
const MAX_CHUNK_MS: u64 = 25_000;
const CUT_SILENCE_MS: u64 = 600;

struct Job {
    idx: i64,
    start_ms: u64,
    end_ms: u64,
    silent: bool,
}

/// 区切りの判定(30ms ごとの音の大きさから)
#[derive(Debug, Default)]
pub struct Segmenter {
    pending: Vec<f32>,
    /// 今の区間の始まり・これまでの長さ(サンプル数)
    chunk_start: u64,
    total: u64,
    silence_run_ms: u64,
    voiced_ms: u64,
    /// 雑音の大きさの推定(ゆっくり追う)
    noise: f32,
    pub level: f32,
}

impl Segmenter {
    /// サンプルを足し、区切れた区間 (開始ms, 終了ms, ほぼ無音か) を返す。
    pub fn push(&mut self, x: &[f32]) -> Vec<(u64, u64, bool)> {
        self.pending.extend_from_slice(x);
        let mut cuts = Vec::new();
        let mut consumed = 0;
        while self.pending.len() - consumed >= FRAME {
            let f = &self.pending[consumed..consumed + FRAME];
            consumed += FRAME;
            let rms = (f.iter().map(|v| v * v).sum::<f32>() / FRAME as f32).sqrt();
            self.level = rms;
            // 雑音の大きさ: 小さい値から始め、下がるときはすぐ追い、上がるのは声の無いときだけ(声で上がらないように)
            if self.noise == 0.0 {
                self.noise = 0.002;
            }
            let voiced = rms > (self.noise * 3.0).max(0.004);
            if rms < self.noise {
                self.noise = rms.max(1e-4);
            } else if !voiced {
                self.noise = self.noise * 0.98 + rms * 0.02;
            } else {
                self.noise = self.noise * 0.9999 + rms * 0.0001;
            }
            self.total += FRAME as u64;
            if voiced {
                self.voiced_ms += 30;
                self.silence_run_ms = 0;
            } else {
                self.silence_run_ms += 30;
            }
            let len_ms = (self.total - self.chunk_start) * 1000 / RATE as u64;
            if (len_ms >= MIN_CHUNK_MS && self.silence_run_ms >= CUT_SILENCE_MS) || len_ms >= MAX_CHUNK_MS {
                cuts.push(self.cut());
            }
        }
        self.pending.drain(..consumed);
        cuts
    }

    fn cut(&mut self) -> (u64, u64, bool) {
        let s = self.chunk_start * 1000 / RATE as u64;
        let e = self.total * 1000 / RATE as u64;
        let silent = self.voiced_ms < 300;
        self.chunk_start = self.total;
        self.voiced_ms = 0;
        (s, e, silent)
    }

    /// 終わり: 残りを1区間にする(無ければ None)
    pub fn finish(&mut self) -> Option<(u64, u64, bool)> {
        self.total += self.pending.len() as u64;
        self.pending.clear();
        (self.total > self.chunk_start).then(|| self.cut())
    }

    pub fn elapsed_ms(&self) -> u64 {
        (self.total + self.pending.len() as u64) * 1000 / RATE as u64
    }
}

pub struct LiveRecorder {
    pub meeting_id: i64,
    pcm_path: PathBuf,
    file: BufWriter<File>,
    seg: Segmenter,
    idx: i64,
    tx: Option<mpsc::Sender<Job>>,
    worker: Option<std::thread::JoinHandle<Result<(), String>>>,
    pending: Arc<AtomicUsize>,
    cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecordStatus {
    pub meeting_id: i64,
    pub elapsed_ms: u64,
    /// 文字起こし待ちの区間の数(多いと追いついていない)
    pub pending_chunks: usize,
    pub level: f32,
}

impl LiveRecorder {
    /// 録音を始める。`db_path` は文字起こしのスレッドが別の接続で開く。
    pub fn start(db_path: &Path, work_dir: &Path, meeting_id: i64, asr: Arc<dyn Asr>, hint: String, language: String, denoise: bool) -> Result<Self, String> {
        std::fs::create_dir_all(work_dir).map_err(|_| "作業フォルダを作れません".to_string())?;
        let pcm_path = work_dir.join(format!("meeting-{meeting_id}.pcm"));
        let file = BufWriter::new(File::create(&pcm_path).map_err(|_| "録音のファイルを作れません".to_string())?);
        let (tx, rx) = mpsc::channel::<Job>();
        let pending = Arc::new(AtomicUsize::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let (db, pcm, p2, c2) = (db_path.to_path_buf(), pcm_path.clone(), pending.clone(), cancel.clone());
        let worker = std::thread::spawn(move || -> Result<(), String> {
            // 開く瞬間は busy_timeout がまだ効かず、録音を始めた側の接続と重なると失敗することがある(Windows の CI で確認)ので数回やり直す
            let mut tries = 0;
            let store = loop {
                match Store::open(&db) {
                    Ok(s) => break s,
                    Err(e) if tries >= 50 => return Err(e.to_string()),
                    Err(_) => {
                        tries += 1;
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            };
            for job in rx {
                let segs = if job.silent || c2.load(Ordering::SeqCst) {
                    vec![]
                } else {
                    let mut x = audio::read_pcm16k(&pcm, job.start_ms, job.end_ms)?;
                    if denoise {
                        x = audio::denoise_16k(&x);
                    }
                    asr.transcribe(&x, &hint, &language, &c2).unwrap_or_default()
                };
                store.save_chunk(meeting_id, job.idx, job.start_ms as i64, job.end_ms as i64, &segs).map_err(|e| e.to_string())?;
                p2.fetch_sub(1, Ordering::SeqCst);
            }
            Ok(())
        });
        Ok(Self { meeting_id, pcm_path, file, seg: Segmenter::default(), idx: 0, tx: Some(tx), worker: Some(worker), pending, cancel })
    }

    fn send(&mut self, store: &Store, (s, e, silent): (u64, u64, bool)) -> Result<(), String> {
        self.file.flush().map_err(|e| e.to_string())?;
        let idx = -(self.idx + 1); // 仮の文字は負の区間番号
        store.add_chunk(self.meeting_id, idx, s as i64, e as i64, silent).map_err(|e| e.to_string())?;
        self.pending.fetch_add(1, Ordering::SeqCst);
        if let Some(tx) = &self.tx {
            tx.send(Job { idx, start_ms: s, end_ms: e, silent }).map_err(|_| "文字起こしが止まっています".to_string())?;
        }
        self.idx += 1;
        Ok(())
    }

    /// 音を足す(16kHz モノラル、-1〜1)。
    pub fn push(&mut self, store: &Store, x: &[f32]) -> Result<RecordStatus, String> {
        for v in x {
            self.file.write_all(&((v.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()).map_err(|_| "録音を保存できません(空き容量を確認してください)".to_string())?;
        }
        for c in self.seg.push(x) {
            self.send(store, c)?;
        }
        Ok(self.status())
    }

    pub fn status(&self) -> RecordStatus {
        RecordStatus { meeting_id: self.meeting_id, elapsed_ms: self.seg.elapsed_ms(), pending_chunks: self.pending.load(Ordering::SeqCst), level: self.seg.level }
    }

    /// 止める: 残りの仮の文字を出し(終わるまで待つ)、WAV に書き出し、正確な文字起こしを待ちに入れる。
    pub fn stop(mut self, store: &Store, audio_dir: &Path) -> Result<(), String> {
        if let Some(c) = self.seg.finish() {
            self.send(store, c)?;
        }
        self.file.flush().map_err(|e| e.to_string())?;
        drop(self.tx.take());
        if let Some(w) = self.worker.take() {
            w.join().map_err(|_| "文字起こしが途中で止まりました".to_string())??;
        }
        let id = self.meeting_id;
        let ms = self.seg.elapsed_ms();
        std::fs::create_dir_all(audio_dir).map_err(|e| e.to_string())?;
        let wav = audio_dir.join(format!("{id}.wav"));
        let all = audio::read_pcm16k(&self.pcm_path, 0, ms)?;
        audio::write_wav16(&wav, &all)?;
        drop(all);
        let _ = std::fs::remove_file(&self.pcm_path);
        store.set_audio_path(id, &wav.to_string_lossy()).map_err(|e| e.to_string())?;
        store.set_pcm(id, None, Some(ms as i64)).map_err(|e| e.to_string())?;
        store.reset_for_final(id).map_err(|e| e.to_string())?;
        pipeline::enqueue(store, id).map(|_| ())
    }

    /// 録音を捨てる(議事録ごと消す)。
    pub fn discard(mut self, store: &Store) -> Result<(), String> {
        self.cancel.store(true, Ordering::SeqCst);
        drop(self.tx.take());
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
        let _ = std::fs::remove_file(&self.pcm_path);
        for p in store.delete_meeting(self.meeting_id).map_err(|e| e.to_string())? {
            let _ = std::fs::remove_file(p);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::FakeAsr;

    fn tone(ms: u64, amp: f32) -> Vec<f32> {
        (0..(ms * 16) as usize).map(|i| (i as f32 * 0.07).sin() * amp).collect()
    }

    #[test]
    fn 声の切れ目で区切り_短すぎる区間は切らない() {
        let mut s = Segmenter::default();
        let mut cuts = Vec::new();
        cuts.extend(s.push(&tone(2_000, 0.3)));
        cuts.extend(s.push(&tone(700, 0.0001))); // 2 秒しかないので切らない
        assert!(cuts.is_empty());
        cuts.extend(s.push(&tone(3_000, 0.3)));
        cuts.extend(s.push(&tone(800, 0.0001))); // 4 秒を超えて 0.6 秒の無音 → 切る
        assert_eq!(cuts.len(), 1);
        assert!(cuts[0].0 == 0 && (6_000..=6_400).contains(&cuts[0].1) && !cuts[0].2);
        // 話し続けても 25 秒で切る
        cuts.extend(s.push(&tone(26_000, 0.3)));
        assert_eq!(cuts.len(), 2);
        assert!(cuts[1].1 - cuts[1].0 <= MAX_CHUNK_MS + 30);
        let last = s.finish().unwrap();
        assert_eq!(last.1, s.elapsed_ms());
        assert!(s.finish().is_none());
    }

    #[test]
    fn 録音して区間ごとに文字にし_止めると再生用のwavができて完了になる() {
        let d = std::env::temp_dir().join(format!("min-rec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let db = d.join("db.sqlite3");
        let st = Store::open(&db).unwrap();
        let id = st.add_meeting("録音", "マイク", "", false).unwrap();
        st.set_state(id, "processing", None).unwrap();
        st.set_recording(id, true).unwrap();
        let asr: Arc<dyn Asr> = Arc::new(FakeAsr { text: "こんにちは".into() });
        let mut r = LiveRecorder::start(&db, &d.join("work"), id, asr, String::new(), "ja".into(), false).unwrap();
        // 0.5 秒ずつ渡す(画面と同じ)
        let mut audio = tone(5_000, 0.3);
        audio.extend(tone(1_000, 0.0001));
        audio.extend(tone(3_000, 0.3));
        for c in audio.chunks(8_000) {
            r.push(&st, c).unwrap();
        }
        assert!(r.status().elapsed_ms >= 8_900);
        r.stop(&st, &d.join("audio")).unwrap();
        let m = st.meeting(id).unwrap().unwrap();
        assert_eq!((m.state.as_str(), m.recording, m.duration_ms.map(|v| v / 1000)), ("queued", false, Some(9)));
        let segs = st.segments(id).unwrap();
        assert_eq!(segs.len(), 2, "{segs:?}"); // 声の切れ目で2区間(仮の文字)
        assert!(segs.iter().all(|s| s.text == "こんにちは" && s.chunk_idx < 0));
        assert!(st.has_provisional(id).unwrap());
        let wav = st.paths(id).unwrap().0.unwrap();
        let pcm = d.join("check.pcm");
        assert_eq!(audio::decode_to_pcm16k(Path::new(&wav), &pcm).unwrap() / 1000, 9);
        assert!(!d.join("work").join(format!("meeting-{id}.pcm")).exists());
        // 正確なモデルで処理すると、仮の文字が置き換わる
        crate::pipeline::run_jobs(&st, &FakeAsr { text: "正確".into() }, &d.join("work"), &crate::pipeline::Options::basic(true), &Arc::new(AtomicBool::new(false)), |_, _, _| {}).unwrap();
        let segs = st.segments(id).unwrap();
        assert!(!segs.is_empty() && segs.iter().all(|s| s.text == "正確" && s.chunk_idx >= 0), "{segs:?}");
        assert_eq!(st.meeting(id).unwrap().unwrap().state, "done");
        std::fs::remove_dir_all(&d).ok();
    }
}
