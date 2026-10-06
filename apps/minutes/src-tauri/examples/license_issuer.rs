//! 発行者用のコマンド(アプリには入らない。販売者の手元だけで使う)。手順は `docs/license.md`。
//!
//! 使い方(通信はしない):
//!   cargo run --release --example license_issuer --no-default-features -- <コマンド> …
//!   keygen  [--out 鍵の保存先]                 発行用の鍵を作る(既定 ~/.config/minutes-license/issuer.key、権限 600)。公開鍵を表示
//!   pubkey  [--key 鍵]                          公開鍵(16進)を表示
//!   issue   --licensee 名前 [--edition pro_offline|pro] [--machine 端末コード] [--features pro,summary]
//!           [--key 鍵 | --dev] [--out ファイル.license]   ライセンスを発行
//!   verify  <キーまたは .license ファイル> [--pubkey 16進 | --key 鍵 | --dev]   署名と中身を確かめる
//!   machine-code                                  この端末の端末コードを表示
//!
//! 秘密鍵(`issuer.key`)はリポジトリの外に置く。**失くすと新しいライセンスを発行できず、漏れると誰でも発行できる。**
use ed25519_dalek::SigningKey;
use minutes::license::*;
use std::path::{Path, PathBuf};

fn die(msg: &str) -> ! {
    eprintln!("エラー: {msg}");
    std::process::exit(1);
}

fn default_key_path() -> PathBuf {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| die("ホームフォルダが分かりません。--out / --key で場所を指定してください"));
    Path::new(&home).join(".config").join("minutes-license").join("issuer.key")
}

fn opt(args: &[String], name: &str) -> Option<String> {
    option_position(args, name).and_then(|i| args.get(i + 1)).cloned()
}

fn flag(args: &[String], name: &str) -> bool {
    option_position(args, name).is_some()
}

// Customer-controlled values such as "--dev" are data, not CLI switches.
fn option_position(args: &[String], name: &str) -> Option<usize> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == name { return Some(i); }
        i += if args[i].starts_with("--") && args[i] != "--dev" { 2 } else { 1 };
    }
    None
}

fn write_new(path: &Path, body: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)] {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(body.as_bytes())?;
    file.sync_all()
}

/// 鍵のファイルが git の管理下(リポジトリの中)にあれば拒否する
fn inside_git_repo(p: &Path) -> bool {
    let abs = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(p) };
    let mut ancestor = abs.clone();
    while !ancestor.exists() {
        if !ancestor.pop() { return true; }
    }
    let Ok(resolved) = ancestor.canonicalize() else { return true; };
    let mut cur = Some(resolved.as_path());
    while let Some(d) = cur {
        if d.join(".git").exists() {
            return true;
        }
        cur = d.parent();
    }
    false
}

fn load_key(args: &[String]) -> SigningKey {
    if flag(args, "--dev") {
        eprintln!("(開発用の鍵を使います。この鍵のライセンスは、デバッグビルドのアプリだけが受け入れます)");
        return dev_signing_key();
    }
    let path = opt(args, "--key").map(PathBuf::from).unwrap_or_else(default_key_path);
    if inside_git_repo(&path) {
        die("秘密鍵はGitリポジトリの外から読み込んでください");
    }
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(&path).map(|m| m.permissions().mode() & 0o077 != 0).unwrap_or(true) {
            die("秘密鍵が読めないか、他の利用者にも読める権限です。所有者だけが読める状態（600）を確認してください");
        }
    }
    let text = std::fs::read_to_string(&path).unwrap_or_else(|_| die(&format!("鍵を読めません: {}(keygen で作るか、--key で指定)", path.display())));
    let seed = parse_hex32(text.lines().find(|l| !l.trim().is_empty() && !l.starts_with('#')).unwrap_or("")).unwrap_or_else(|| die("鍵のファイルの形式が違います"));
    SigningKey::from_bytes(&seed)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or(&[]).to_vec();
    match args.first().map(|s| s.as_str()) {
        Some("keygen") => {
            let path = opt(&rest, "--out").map(PathBuf::from).unwrap_or_else(default_key_path);
            if inside_git_repo(&path) {
                die("リポジトリの中には秘密鍵を置けません。リポジトリの外の場所を --out で指定してください");
            }
            if path.exists() {
                die(&format!("すでに鍵があります: {}(上書きしません。作り直すと、いままで発行したライセンスが検証できなくなります)", path.display()));
            }
            if let Some(d) = path.parent() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(d).unwrap_or_else(|e| die(&e.to_string()));
                }
                #[cfg(not(unix))]
                std::fs::create_dir_all(d).unwrap_or_else(|e| die(&e.to_string()));
            }
            let seed = random_bytes::<32>();
            let body = format!("# 議事録 ライセンス発行用の秘密鍵。他人に見せない・リポジトリに入れない・必ずバックアップする\n{}\n", hex(&seed));
            #[cfg(unix)]
            {
                use std::io::Write;
                use std::os::unix::fs::OpenOptionsExt;
                let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path).unwrap_or_else(|e| die(&e.to_string()));
                f.write_all(body.as_bytes()).unwrap_or_else(|e| die(&e.to_string()));
            }
            #[cfg(not(unix))]
            write_new(&path, &body).unwrap_or_else(|e| die(&e.to_string()));
            let vk = SigningKey::from_bytes(&seed).verifying_key();
            println!("秘密鍵を保存しました: {}", path.display());
            println!("公開鍵(src/license.rs の PRODUCTION_PUBLIC_KEY_HEX に貼る): {}", hex(vk.as_bytes()));
            println!("この秘密鍵は、失くすと新しいライセンスを発行できず、漏れると誰でも発行できます。オフラインの媒体に複数のバックアップを取ってください。");
        }
        Some("pubkey") => println!("{}", hex(load_key(&rest).verifying_key().as_bytes())),
        Some("machine-code") => match machine_code() {
            Some(c) => println!("{c}"),
            None => die("この端末の端末コードを取得できません"),
        },
        Some("issue") => {
            let licensee = opt(&rest, "--licensee").unwrap_or_else(|| die("--licensee(購入者の名前・組織名)を指定してください"));
            let edition = opt(&rest, "--edition").unwrap_or_else(|| EDITION_PRO_OFFLINE.into());
            if edition != EDITION_PRO_OFFLINE && edition != EDITION_PRO {
                die("--edition は pro_offline か pro");
            }
            let features: Vec<String> = opt(&rest, "--features").unwrap_or_else(|| "pro,summary".into()).split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            let machine = opt(&rest, "--machine");
            if let Some(m) = &machine {
                let n: String = m.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
                if n.len() != 16 {
                    die("端末コードの形式が違います(例: ABCD-EFGH-IJKL-MNOP)");
                }
            }
            let p = Payload { v: 1, id: new_license_id(), licensee, edition, issued: opt(&rest, "--issued").unwrap_or_else(today_ymd), features, machine: machine.map(|m| m.to_ascii_uppercase()) };
            let key = encode_key(&p, &load_key(&rest));
            if let Some(out) = opt(&rest, "--out") {
                write_new(Path::new(&out), &license_file_text(&p, &key)).unwrap_or_else(|e| die(&e.to_string()));
                println!("ライセンスファイルを書きました: {out}");
            }
            println!("id: {}  購入者: {}  版: {}  端末の固定: {}", p.id, p.licensee, p.edition, if p.machine.is_some() { "あり" } else { "なし" });
            println!("{key}");
            println!("(発行の記録として、id・購入者・発行日・注文番号を台帳に控えてください。返金や失効のときに id が要ります)");
        }
        Some("verify") => {
            let target = rest.first().unwrap_or_else(|| die("キーまたは .license ファイルを指定してください"));
            let text = if Path::new(target).is_file() { std::fs::read_to_string(target).unwrap_or_else(|e| die(&e.to_string())) } else { target.clone() };
            let vk = match opt(&rest, "--pubkey") {
                Some(h) => ed25519_dalek::VerifyingKey::from_bytes(&parse_hex32(&h).unwrap_or_else(|| die("--pubkey は 16進 64 文字"))).unwrap_or_else(|_| die("公開鍵が正しくありません")),
                None => load_key(&rest).verifying_key(),
            };
            match inspect(&text, &vk) {
                Ok(p) => {
                    println!("署名: 正しい");
                    println!("{}", serde_json::to_string_pretty(&p).unwrap());
                }
                Err(e) => die(&e.to_string()),
            }
        }
        _ => {
            eprintln!("使い方: license_issuer keygen | pubkey | issue | verify | machine-code(詳細はファイル先頭の説明と docs/license.md)");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn option_values_cannot_enable_dev_or_change_output() {
        let args = ["--licensee", "--dev", "--out", "result.license"].map(String::from);
        assert!(!flag(&args, "--dev"));
        assert_eq!(opt(&args, "--licensee").as_deref(), Some("--dev"));
        assert_eq!(opt(&args, "--out").as_deref(), Some("result.license"));
        assert!(flag(&["--dev".to_string()], "--dev"));
    }
    #[test]
    fn output_never_overwrites_an_existing_file() {
        let path = std::env::temp_dir().join(format!("minutes-issuer-output-{}.license", std::process::id()));
        write_new(&path, "original").unwrap();
        assert!(write_new(&path, "replacement").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn git_boundary_resolves_parent_traversal() {
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(inside_git_repo(&repo.join("../src-tauri/not-created.key")));
    }
}
