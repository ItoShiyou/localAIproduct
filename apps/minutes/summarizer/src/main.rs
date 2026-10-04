//! 要約用の推論サイドカー(llama.cpp、CPU)。議事録アプリの本体から子プロセスとして起動される。
//!
//! - 通信しない。ソケットも開かない。本体とは標準入力・標準出力の JSON 行だけでやり取りする(ログは標準エラー)。
//! - 本体は whisper.cpp(別の ggml)を抱えているため、同じ実行ファイルに llama.cpp を入れず、別の実行ファイルにしている。
//! - モデルは起動時に1回だけ読み込み、標準入力が閉じるまで何度でも依頼を受ける。中断は本体がプロセスを終了させる。
//!
//! 使い方: `minutes-summarizer --model <GGUF> [--threads N] [--ctx N]`
//!
//! 入力(1行1依頼):
//!   {"id":1,"op":"generate","system":"…","user":"…","max_tokens":1024,"grammar":"<GBNF。省略可>"}
//!   {"id":2,"op":"count","text":"…"}
//! 出力(1行1メッセージ):
//!   {"type":"ready","n_ctx":8192,"load_secs":1.2}
//!   {"type":"progress","id":1,"generated":16}
//!   {"type":"done","id":1,"text":"…","prompt_tokens":900,"gen_tokens":210,"prefill_secs":8.1,"gen_secs":14.0}
//!   {"type":"count","id":2,"tokens":123}
//!   {"type":"error","id":1,"message":"…"}

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{LlamaChatMessage, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use serde::Deserialize;
use serde_json::json;
use std::io::{BufRead, Write};
use std::num::NonZeroU32;
use std::time::Instant;

#[derive(Deserialize)]
struct Request {
    #[serde(default)]
    id: u64,
    op: String,
    #[serde(default)]
    system: String,
    #[serde(default)]
    user: String,
    #[serde(default)]
    text: String,
    #[serde(default = "default_max")]
    max_tokens: u32,
    #[serde(default)]
    grammar: Option<String>,
}

fn default_max() -> u32 {
    1024
}

fn emit(v: serde_json::Value) {
    let mut o = std::io::stdout().lock();
    let _ = writeln!(o, "{v}");
    let _ = o.flush();
}

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

/// モデルの会話テンプレートで質問文を組み立てる(テンプレートが無い・適用できないときは素朴な形)。
fn build_prompt(model: &LlamaModel, system: &str, user: &str) -> String {
    let msgs = (|| {
        let mut v = Vec::new();
        if !system.is_empty() {
            v.push(LlamaChatMessage::new("system".into(), system.into()).ok()?);
        }
        v.push(LlamaChatMessage::new("user".into(), user.into()).ok()?);
        Some(v)
    })();
    if let (Some(msgs), Ok(tmpl)) = (msgs, model.chat_template(None)) {
        if let Ok(p) = model.apply_chat_template(&tmpl, &msgs, true) {
            return p;
        }
    }
    format!("{system}\n\n{user}\n\n")
}

/// 本文に紛れた会話テンプレートの特別な記号(`<|im_end|>` など)を無効にする。文字起こしは「データ」であって「指示」ではない。
fn neutralize(s: &str) -> String {
    s.replace("<|", "< |").replace("|>", "| >").replace("<s>", "< s>").replace("</s>", "< /s>").replace("[INST]", "[ INST]").replace("[/INST]", "[ /INST]")
}

fn generate(
    model: &LlamaModel,
    backend: &LlamaBackend,
    threads: i32,
    n_ctx: u32,
    r: &Request,
) -> Result<serde_json::Value, String> {
    let prompt = build_prompt(model, &neutralize(&r.system), &neutralize(&r.user));
    // 会話テンプレートの特別なトークンを解釈させるため parse_special を使う。そのぶん、本文に紛れた特別な記号は先に無効にする
    let tokens = model.vocab().tokenize(prompt.as_bytes(), true, true);
    if tokens.len() as u32 + r.max_tokens > n_ctx {
        return Err(format!("too_long: 入力 {} トークン + 出力 {} が文脈 {} を超えます", tokens.len(), r.max_tokens, n_ctx));
    }
    let params = LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(n_ctx))
        .with_n_batch(512)
        .with_n_threads(threads)
        .with_n_threads_batch(threads);
    let mut ctx = model.new_context(backend, params).map_err(|e| format!("文脈を作れません: {e}"))?;
    let mut batch = LlamaBatch::new(512, 1);

    // 入力の読み込み(prefill)
    let t0 = Instant::now();
    let mut pos = 0i32;
    let n = tokens.len();
    for chunk in tokens.chunks(512) {
        batch.clear();
        for (i, t) in chunk.iter().enumerate() {
            let last = pos as usize + 1 == n && i + 1 == chunk.len();
            batch.add(*t, pos, &[0], last).map_err(|e| e.to_string())?;
            pos += 1;
        }
        ctx.decode(&mut batch).map_err(|e| format!("decode に失敗: {e}"))?;
    }
    let prefill = t0.elapsed().as_secs_f64();

    // 出力(温度0 = 毎回同じ結果。文法があれば、その形に必ず従う)
    let mut chain = Vec::new();
    if let Some(g) = r.grammar.as_deref().filter(|g| !g.is_empty()) {
        chain.push(LlamaSampler::grammar(model, g, "root").map_err(|e| format!("文法が不正: {e}"))?);
    }
    chain.push(LlamaSampler::penalties(model.n_vocab(), 256, 1.05, 0.0, 0.0));
    chain.push(LlamaSampler::greedy());
    let mut sampler = LlamaSampler::chain_simple(chain);
    let vocab = model.vocab();
    let mut out: Vec<u8> = Vec::new();
    let mut gen = 0u32;
    let t1 = Instant::now();
    let mut idx = batch.n_tokens() - 1;
    while gen < r.max_tokens {
        let tok = sampler.sample(&ctx, idx);
        if vocab.is_eog(tok) {
            break;
        }
        out.extend_from_slice(&vocab.token_to_piece(tok, false, None));
        gen += 1;
        if gen % 16 == 0 {
            emit(json!({"type":"progress","id":r.id,"generated":gen}));
        }
        batch.clear();
        batch.add(tok, pos, &[0], true).map_err(|e| e.to_string())?;
        pos += 1;
        idx = 0;
        ctx.decode(&mut batch).map_err(|e| format!("decode に失敗: {e}"))?;
    }
    Ok(json!({
        "type":"done","id":r.id,"text":String::from_utf8_lossy(&out),
        "prompt_tokens":n,"gen_tokens":gen,"prefill_secs":prefill,"gen_secs":t1.elapsed().as_secs_f64()
    }))
}

fn main() {
    let Some(model_path) = arg("--model") else {
        eprintln!("--model が必要です");
        std::process::exit(2);
    };
    let threads: i32 = arg("--threads").and_then(|s| s.parse().ok()).unwrap_or(6);
    let n_ctx: u32 = arg("--ctx").and_then(|s| s.parse().ok()).unwrap_or(8192);
    let t0 = Instant::now();
    let backend = match LlamaBackend::init() {
        Ok(b) => b,
        Err(e) => {
            emit(json!({"type":"error","id":0,"message":format!("初期化に失敗: {e}")}));
            std::process::exit(1);
        }
    };
    // CPU だけで動かす(GPU に載せない)。mmap で読むので、使うときだけメモリに載る
    let mp = LlamaModelParams::default().with_n_gpu_layers(0);
    let model = match LlamaModel::load_from_file(&backend, &model_path, &mp) {
        Ok(m) => m,
        Err(e) => {
            emit(json!({"type":"error","id":0,"message":format!("モデルを読み込めません: {e}")}));
            std::process::exit(1);
        }
    };
    emit(json!({"type":"ready","n_ctx":n_ctx,"load_secs":t0.elapsed().as_secs_f64()}));

    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let r: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                emit(json!({"type":"error","id":0,"message":format!("依頼を読めません: {e}")}));
                continue;
            }
        };
        match r.op.as_str() {
            "count" => {
                let n = model.vocab().tokenize(neutralize(&r.text).as_bytes(), false, false).len();
                emit(json!({"type":"count","id":r.id,"tokens":n}));
            }
            "generate" => match generate(&model, &backend, threads, n_ctx, &r) {
                Ok(v) => emit(v),
                Err(m) => emit(json!({"type":"error","id":r.id,"message":m})),
            },
            other => emit(json!({"type":"error","id":r.id,"message":format!("不明な op: {other}")})),
        }
    }
}
