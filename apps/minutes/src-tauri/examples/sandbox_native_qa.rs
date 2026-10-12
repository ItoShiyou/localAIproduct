//! Separate ad-hoc sandbox QA host; fabricated bundled PCM only.
use minutes::{asr::{Asr,WhisperAsr},diarize::{Embedder,OnnxEmbedder}};
use std::sync::{Arc,atomic::AtomicBool};
fn main() {
    let exe=std::env::current_exe().unwrap();
    let resources=exe.parent().unwrap().parent().unwrap().join("Resources/resources");
    let bytes=std::fs::read(resources.join("qa-synthetic.pcm")).unwrap();
    let pcm:Vec<f32>=bytes.chunks_exact(2).map(|b|i16::from_le_bytes([b[0],b[1]]) as f32/32768.).collect();
    let asr=WhisperAsr::new(&resources.join("models/ggml-small-q5_1.bin"),6).unwrap();
    let text=asr.transcribe(&pcm,"","ja",&Arc::new(AtomicBool::new(false))).unwrap();
    assert!(!text.is_empty());
    let ort=OnnxEmbedder::new(&resources.join("onnxruntime/libonnxruntime.dylib"),&resources.join("models/voxceleb_resnet34_LM.onnx"),4).unwrap();
    let vector=ort.embed(&pcm).unwrap();assert!(!vector.is_empty());
    println!("Sandbox speech and speaker engines passed; segments={}, embedding_dimensions={}",text.len(),vector.len());
}
