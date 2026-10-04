//! オフラインで検証できるライセンス(通信なし)。詳細は `docs/license.md`。
//!
//! - ライセンスキーは、発行者の秘密鍵(Ed25519)で署名した JSON。アプリには**公開鍵だけ**を埋め込み、
//!   署名の検証はこの端末の中だけで行う。通信は一切しない(認証サーバーは無い)。
//! - キーの文字列は `MNT1-XXXXXX-XXXXXX-…`(Base32 を6文字ずつダッシュで区切ったもの。メール・紙で写せる)。
//!   中身は 署名 64 バイト + JSON。`.license` ファイルは、このキーに `#` で始まる説明行を付けたもの。
//! - 端末の固定は任意。`machine` が空なら、どの端末でも使える(購入者名を画面に出して抑止する)。
//!   入っていれば、この端末の「端末コード」と一致したときだけ有効。
//! - **限界**: 通信で失効させる手段が無い。配られたキーは、アプリの更新で `REVOKED_IDS` に載せるまで使える。
//!   core の `license`(販売プラットフォームに問い合わせる形)とは別物。オンライン認証を足すときは、
//!   `Grant` を返す別の入口を `resolve` に足す(この型を共通の出口にする)。
//! - 秘密鍵はこのリポジトリに置かない。置くのは公開鍵と、開発用の鍵(デバッグビルドでだけ受け入れる)だけ。

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 製品用の公開鍵(16進 64 文字)。**空のうちは、配布用のビルドで有効になるライセンスが無い。**
/// 発行者の鍵を作ったら(`docs/license.md`)、ここに貼る。
pub const PRODUCTION_PUBLIC_KEY_HEX: &str = "";

/// 開発用の秘密鍵の種。公開してよい(デバッグビルドとテストだけがこの鍵の署名を受け入れる)。
pub const DEV_SEED: [u8; 32] = *b"minutes-DEV-ONLY-license-seed-01";

/// 失効させたキーの id(アプリの更新で足す。通信での失効はできない)
pub const REVOKED_IDS: &[&str] = &[];

pub const KEY_PREFIX: &str = "MNT1";
pub const MAX_LICENSE_BYTES: u64 = 64 * 1024;

pub const EDITION_PRO_OFFLINE: &str = "pro_offline";
pub const EDITION_PRO: &str = "pro";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Payload {
    pub v: u32,
    /// キーごとの乱数(失効の一覧に載せるときの目印)
    pub id: String,
    /// 購入者の名前・組織名(アプリに表示する)
    pub licensee: String,
    /// "pro_offline"(通信しない版)| "pro"
    pub edition: String,
    /// 発行日(YYYY-MM-DD)
    pub issued: String,
    pub features: Vec<String>,
    /// 端末コード。None なら端末に固定しない
    pub machine: Option<String>,
}

/// 検証に通ったライセンス(画面と判定に使う)
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LicenseInfo {
    pub id: String,
    pub licensee: String,
    pub edition: String,
    pub issued: String,
    pub features: Vec<String>,
    pub machine_bound: bool,
    /// 通信を一切しない版(ネットワークの機能を常に止める)
    pub offline: bool,
    /// 開発用の鍵で署名されたもの(デバッグビルドだけ)
    pub dev: bool,
}

impl LicenseInfo {
    pub fn has(&self, feature: &str) -> bool {
        self.features.iter().any(|f| f == feature)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LicenseError {
    /// キーの形になっていない
    Format,
    /// 新しい形式(このアプリより新しい)
    Version,
    /// 署名が合わない(書き換え・欠け・別の発行者)
    Signature,
    /// この端末用ではない(自分の端末コードを持つ)
    Machine(String),
    /// 失効したキー
    Revoked,
    /// 有料の機能が含まれていない
    NoFeature,
    /// 配布用のビルドに製品用の公開鍵がまだ入っていない
    NoKey,
}

impl std::fmt::Display for LicenseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Format => write!(f, "ライセンスキーの形式が違います。メールなどに書かれたキー(MNT1- から始まる文字列)を、そのまま貼り付けてください"),
            Self::Version => write!(f, "このライセンスは新しい形式です。アプリを最新版に更新してください"),
            Self::Signature => write!(f, "署名が合いません。キーが途中で欠けているか、書き換えられている可能性があります。もう一度コピーし直してください"),
            Self::Machine(code) => write!(f, "このライセンスは別の端末用です(この端末の端末コード: {code})。購入先に、この端末コードを伝えて再発行を依頼してください"),
            Self::Revoked => write!(f, "このライセンスは無効になっています。購入先に問い合わせてください"),
            Self::NoFeature => write!(f, "このライセンスには有料版の機能が含まれていません"),
            Self::NoKey => write!(f, "このビルドには、ライセンスを確かめる公開鍵が入っていません"),
        }
    }
}

// ---------------- Base32(RFC 4648、パディングなし) ----------------

const B32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

pub fn b32_encode(data: &[u8]) -> String {
    let mut out = String::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for &b in data {
        buf = (buf << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(B32[((buf >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(B32[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

pub fn b32_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = B32.iter().position(|&x| x == c.to_ascii_uppercase())? as u32;
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

// ---------------- キーの組み立てと読み取り ----------------

/// 署名つきのキー文字列を作る(発行者用)。6文字ずつダッシュで区切る。
pub fn encode_key(payload: &Payload, sk: &SigningKey) -> String {
    let json = serde_json::to_vec(payload).expect("JSON");
    let sig = sk.sign(&json);
    let mut raw = sig.to_bytes().to_vec();
    raw.extend_from_slice(&json);
    let body = b32_encode(&raw);
    let groups: Vec<&str> = body.as_bytes().chunks(6).map(|c| std::str::from_utf8(c).unwrap()).collect();
    format!("{KEY_PREFIX}-{}", groups.join("-"))
}

/// `.license` ファイルの中身(説明行つき。人が読むためのもので、検証には使わない)
pub fn license_file_text(payload: &Payload, key: &str) -> String {
    let mut s = String::new();
    s.push_str("# 議事録 ライセンスファイル(下のキーが本体です。この行は説明で、読み込みには使いません)\n");
    s.push_str(&format!("# 購入者: {}\n", payload.licensee));
    s.push_str(&format!("# 版: {} / 発行日: {} / id: {}\n", payload.edition, payload.issued, payload.id));
    s.push_str(&format!("# 端末の固定: {}\n", if payload.machine.is_some() { "あり" } else { "なし" }));
    // 読みやすいように 8 グループ(48 文字)ごとに改行
    let parts: Vec<&str> = key.split('-').collect();
    for line in parts.chunks(8) {
        s.push_str(&line.join("-"));
        s.push('\n');
    }
    s
}

/// テキスト(キーそのもの、または `.license` ファイルの中身)から、署名つきの生のバイト列を取り出す
fn parse_raw(text: &str) -> Result<Vec<u8>, LicenseError> {
    let mut body = String::new();
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        body.push_str(l);
    }
    let compact: String = body.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
    let rest = compact.to_ascii_uppercase();
    let rest = rest.strip_prefix(KEY_PREFIX).ok_or(LicenseError::Format)?;
    let raw = b32_decode(rest).ok_or(LicenseError::Format)?;
    if raw.len() <= 64 || raw.len() > 8192 {
        return Err(LicenseError::Format);
    }
    Ok(raw)
}

/// 受け入れる公開鍵: 製品用(埋め込み)+ デバッグビルドのみ開発用
pub fn trusted_keys() -> Vec<(VerifyingKey, bool)> {
    let mut v = Vec::new();
    if let Some(k) = production_key() {
        v.push((k, false));
    }
    if cfg!(debug_assertions) {
        v.push((dev_signing_key().verifying_key(), true));
    }
    v
}

fn production_key() -> Option<VerifyingKey> {
    parse_hex32(PRODUCTION_PUBLIC_KEY_HEX).and_then(|b| VerifyingKey::from_bytes(&b).ok())
}

pub fn dev_signing_key() -> SigningKey {
    SigningKey::from_bytes(&DEV_SEED)
}

pub fn parse_hex32(s: &str) -> Option<[u8; 32]> {
    let s = s.trim();
    if s.len() != 64 || !s.is_ascii() {
        return None;
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// 検証する。`keys` は (公開鍵, 開発用か)。`machine` はこの端末の端末コード(取れなければ None)。
/// 端末に固定されたキーは、端末コードが取れない・一致しない場合は無効。
pub fn verify(text: &str, keys: &[(VerifyingKey, bool)], machine: Option<&str>, revoked: &[&str]) -> Result<LicenseInfo, LicenseError> {
    let raw = parse_raw(text)?;
    if keys.is_empty() {
        return Err(LicenseError::NoKey);
    }
    let (sig_bytes, json) = raw.split_at(64);
    let sig = Signature::from_slice(sig_bytes).map_err(|_| LicenseError::Format)?;
    let dev = keys
        .iter()
        .find(|(k, _)| k.verify(json, &sig).is_ok())
        .map(|(_, dev)| *dev)
        .ok_or(LicenseError::Signature)?;
    let p: Payload = serde_json::from_slice(json).map_err(|_| LicenseError::Format)?;
    if p.v != 1 {
        return Err(LicenseError::Version);
    }
    if revoked.contains(&p.id.as_str()) {
        return Err(LicenseError::Revoked);
    }
    if !p.features.iter().any(|f| f == "pro") {
        return Err(LicenseError::NoFeature);
    }
    if let Some(m) = &p.machine {
        let this = machine.map(normalize_code).unwrap_or_default();
        if this.is_empty() || normalize_code(m) != this {
            return Err(LicenseError::Machine(machine.unwrap_or("取得できません").to_string()));
        }
    }
    Ok(LicenseInfo {
        id: p.id,
        licensee: p.licensee,
        offline: p.edition == EDITION_PRO_OFFLINE,
        edition: p.edition,
        issued: p.issued,
        features: p.features,
        machine_bound: p.machine.is_some(),
        dev,
    })
}

/// この端末で使う検証(製品用の鍵 + デバッグ時の開発用の鍵、実際の端末コード、失効一覧)
pub fn verify_here(text: &str) -> Result<LicenseInfo, LicenseError> {
    verify(text, &trusted_keys(), machine_code().as_deref(), REVOKED_IDS)
}

/// 発行者の `verify` コマンド用: 署名だけ確かめて中身を返す(端末・失効は見ない)
pub fn inspect(text: &str, vk: &VerifyingKey) -> Result<Payload, LicenseError> {
    let raw = parse_raw(text)?;
    let (sig_bytes, json) = raw.split_at(64);
    let sig = Signature::from_slice(sig_bytes).map_err(|_| LicenseError::Format)?;
    vk.verify(json, &sig).map_err(|_| LicenseError::Signature)?;
    serde_json::from_slice(json).map_err(|_| LicenseError::Format)
}

// ---------------- 端末コード ----------------

fn normalize_code(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_uppercase()
}

/// OS の端末 ID から端末コードを作る(塩つきの SHA-256 の先頭 10 バイトを Base32 にして 4 文字ずつ区切る。生の ID は外に出さない)
pub fn code_from_raw(raw_id: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"minutes-machine-code-v1|");
    h.update(raw_id.trim().to_ascii_lowercase().as_bytes());
    let d = h.finalize();
    let s = b32_encode(&d[..10]);
    s.as_bytes().chunks(4).map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join("-")
}

/// この端末の端末コード(OS の端末 ID が取れなければ None)
pub fn machine_code() -> Option<String> {
    raw_machine_id().filter(|s| !s.trim().is_empty()).map(|s| code_from_raw(&s))
}

fn run_capture(cmd: &str, args: &[&str]) -> Option<String> {
    let mut c = std::process::Command::new(cmd);
    c.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let o = c.output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

#[cfg(target_os = "macos")]
fn raw_machine_id() -> Option<String> {
    // IOPlatformUUID
    let out = run_capture("/usr/sbin/ioreg", &["-rd1", "-c", "IOPlatformExpertDevice"])?;
    out.lines().find(|l| l.contains("IOPlatformUUID")).and_then(|l| l.split('"').nth(3)).map(|s| s.to_string())
}

#[cfg(windows)]
fn raw_machine_id() -> Option<String> {
    // HKLM\SOFTWARE\Microsoft\Cryptography の MachineGuid
    let out = run_capture("reg", &["query", r"HKLM\SOFTWARE\Microsoft\Cryptography", "/v", "MachineGuid"])?;
    out.lines().find(|l| l.contains("MachineGuid")).and_then(|l| l.split_whitespace().last()).map(|s| s.to_string())
}

#[cfg(not(any(target_os = "macos", windows)))]
fn raw_machine_id() -> Option<String> {
    // 開発用(対応 OS は macOS と Windows のみ)
    std::fs::read_to_string("/etc/machine-id").ok()
}

// ---------------- 乱数(発行者用) ----------------

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).expect("OS の乱数が使えません");
    b
}

pub fn new_license_id() -> String {
    hex(&random_bytes::<8>())
}

pub fn today_ymd() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
}

// ---------------- 機能の判定(plan.rs から使う) ----------------

/// ライセンスの機能 → 画面・判定に使う可否
pub fn grants_summary(info: &LicenseInfo) -> bool {
    info.has("summary")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sk(n: u8) -> SigningKey {
        SigningKey::from_bytes(&[n; 32])
    }

    fn payload(machine: Option<&str>) -> Payload {
        Payload {
            v: 1,
            id: "0123456789abcdef".into(),
            licensee: "テスト大学 医学部 山田研究室".into(),
            edition: EDITION_PRO_OFFLINE.into(),
            issued: "2026-10-05".into(),
            features: vec!["pro".into(), "summary".into()],
            machine: machine.map(|s| s.into()),
        }
    }

    fn keys(n: u8) -> Vec<(VerifyingKey, bool)> {
        vec![(sk(n).verifying_key(), false)]
    }

    #[test]
    fn 正しい署名のキーは有効で_購入者名と版が取れる() {
        let key = encode_key(&payload(None), &sk(1));
        assert!(key.starts_with("MNT1-"));
        let info = verify(&key, &keys(1), None, &[]).unwrap();
        assert_eq!(info.licensee, "テスト大学 医学部 山田研究室");
        assert!(info.offline && !info.machine_bound && info.has("pro") && grants_summary(&info));
    }

    #[test]
    fn 別の鍵で署名したキーは無効() {
        let key = encode_key(&payload(None), &sk(2));
        assert_eq!(verify(&key, &keys(1), None, &[]).unwrap_err(), LicenseError::Signature);
    }

    #[test]
    fn 書き換えたキーは無効() {
        let key = encode_key(&payload(None), &sk(1));
        // 署名の後ろのどこか1文字を変える(中身の JSON の部分)
        let mut chars: Vec<char> = key.chars().collect();
        let i = chars.len() - 10;
        chars[i] = if chars[i] == 'A' { 'B' } else { 'A' };
        let t: String = chars.into_iter().collect();
        assert!(matches!(verify(&t, &keys(1), None, &[]).unwrap_err(), LicenseError::Signature | LicenseError::Format));
        // 購入者名を差し替えて署名はそのまま
        let raw = parse_raw(&key).unwrap();
        let json = String::from_utf8(raw[64..].to_vec()).unwrap().replace("山田", "田中");
        let mut forged = raw[..64].to_vec();
        forged.extend_from_slice(json.as_bytes());
        let t = format!("{KEY_PREFIX}-{}", b32_encode(&forged));
        assert_eq!(verify(&t, &keys(1), None, &[]).unwrap_err(), LicenseError::Signature);
    }

    #[test]
    fn 形式が違うものは形式エラー() {
        for t in ["", "hello", "MNT1-AAAA", "XYZ9-ABCDEF"] {
            assert_eq!(verify(t, &keys(1), None, &[]).unwrap_err(), LicenseError::Format, "{t}");
        }
        assert!(LicenseError::Format.to_string().contains("形式が違います"));
    }

    #[test]
    fn 端末に固定したキーは_その端末だけで有効() {
        let here = code_from_raw("AAAA-BBBB");
        let other = code_from_raw("CCCC-DDDD");
        assert_ne!(here, other);
        let bound = encode_key(&payload(Some(&here)), &sk(1));
        assert!(verify(&bound, &keys(1), Some(&here), &[]).unwrap().machine_bound);
        // 表記ゆれ(小文字・ダッシュなし)でも一致
        assert!(verify(&bound, &keys(1), Some(&here.replace('-', "").to_lowercase()), &[]).is_ok());
        let e = verify(&bound, &keys(1), Some(&other), &[]).unwrap_err();
        assert!(matches!(e, LicenseError::Machine(_)) && e.to_string().contains("別の端末用"));
        // 端末コードが取れない端末では、固定したキーは通らない
        assert!(verify(&bound, &keys(1), None, &[]).is_err());
        // 固定していないキーは、端末コードが何でも通る
        let free = encode_key(&payload(None), &sk(1));
        assert!(verify(&free, &keys(1), Some(&other), &[]).is_ok() && verify(&free, &keys(1), None, &[]).is_ok());
    }

    #[test]
    fn 版から機能が決まり_pro_の機能が無ければ無効() {
        let mut p = payload(None);
        p.edition = EDITION_PRO.into();
        p.features = vec!["pro".into()];
        let info = verify(&encode_key(&p, &sk(1)), &keys(1), None, &[]).unwrap();
        assert!(!info.offline && !grants_summary(&info));
        p.features = vec!["summary".into()];
        assert_eq!(verify(&encode_key(&p, &sk(1)), &keys(1), None, &[]).unwrap_err(), LicenseError::NoFeature);
    }

    #[test]
    fn 失効一覧と新しい形式() {
        let key = encode_key(&payload(None), &sk(1));
        assert_eq!(verify(&key, &keys(1), None, &["0123456789abcdef"]).unwrap_err(), LicenseError::Revoked);
        let mut p = payload(None);
        p.v = 2;
        assert_eq!(verify(&encode_key(&p, &sk(1)), &keys(1), None, &[]).unwrap_err(), LicenseError::Version);
    }

    #[test]
    fn ライセンスファイルの説明行は無視され_ダッシュや改行があっても読める() {
        let p = payload(None);
        let key = encode_key(&p, &sk(1));
        let file = license_file_text(&p, &key);
        assert!(file.contains("購入者") && file.lines().count() > 5);
        assert!(verify(&file, &keys(1), None, &[]).is_ok());
        assert!(verify(&format!("  {}  \r\n", key.to_lowercase()), &keys(1), None, &[]).is_ok());
        assert_eq!(inspect(&file, &sk(1).verifying_key()).unwrap(), p);
    }

    #[test]
    fn 鍵が無いビルドは何も通さない_開発用の鍵は開発用と分かる() {
        let key = encode_key(&payload(None), &dev_signing_key());
        assert_eq!(verify(&key, &[], None, &[]).unwrap_err(), LicenseError::NoKey);
        let info = verify(&key, &[(dev_signing_key().verifying_key(), true)], None, &[]).unwrap();
        assert!(info.dev);
        // 製品用の公開鍵が空の間は、配布用のビルドに有効な鍵が無い
        assert!(PRODUCTION_PUBLIC_KEY_HEX.is_empty() || production_key().is_some());
    }

    #[test]
    fn 開発用の鍵はデバッグビルドだけが受け入れる() {
        let dev_key = encode_key(&payload(None), &dev_signing_key());
        let r = verify_here(&dev_key);
        if cfg!(debug_assertions) {
            assert!(r.unwrap().dev);
        } else {
            // 配布用のビルド: 開発用の鍵の署名は通らない(製品用の公開鍵が空なら鍵そのものが無い)
            assert!(matches!(r.unwrap_err(), LicenseError::Signature | LicenseError::NoKey));
        }
        assert_eq!(trusted_keys().iter().any(|(_, dev)| *dev), cfg!(debug_assertions));
    }

    #[test]
    fn base32_と日付() {
        for d in [&b""[..], b"f", b"fo", b"foo", b"foob", b"fooba", b"foobar"] {
            assert_eq!(b32_decode(&b32_encode(d)).unwrap(), d);
        }
        assert_eq!(b32_encode(b"foobar"), "MZXW6YTBOI");
        let t = today_ymd();
        assert!(t.len() == 10 && t.starts_with("20"), "{t}");
    }

    #[test]
    fn この端末の端末コードは取れて_毎回同じ() {
        // macOS / Windows では OS の端末 ID から、Linux の CI では /etc/machine-id から(無ければ None)
        let a = machine_code();
        assert_eq!(a, machine_code());
        if let Some(c) = a {
            assert_eq!(c.len(), 19, "{c}");
        }
    }
}
