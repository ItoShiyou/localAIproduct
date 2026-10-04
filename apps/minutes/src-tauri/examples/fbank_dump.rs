//! 開発用: fbank を JSON で出す(Python の kaldi-native-fbank と突き合わせるため)。
//! 使い方: cargo run --example fbank_dump --no-default-features -- <16kHz wav> <開始秒> <終了秒>
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let tmp = std::env::temp_dir().join("fbank_dump.pcm");
    minutes::audio::decode_to_pcm16k(std::path::Path::new(&a[0]), &tmp).unwrap();
    let s = (a[1].parse::<f64>().unwrap() * 1000.0) as u64;
    let e = (a[2].parse::<f64>().unwrap() * 1000.0) as u64;
    let pcm = minutes::audio::read_pcm16k(&tmp, s, e).unwrap();
    let f = minutes::diarize::fbank(&pcm);
    println!("{}", serde_json::to_string(&f.iter().map(|r| r.to_vec()).collect::<Vec<_>>()).unwrap());
}
