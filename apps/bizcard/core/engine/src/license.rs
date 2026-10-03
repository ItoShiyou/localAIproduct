//! ライセンスの判定ロジックと、販売プラットフォームとの接続口。
//!
//! 作るのは「キーの有効化・検証・無効化を呼ぶ」ことと、「いつまで動かすか」の判定だけ。
//! 決済・キーの発行・返金は、販売プラットフォーム側に任せる。
//!
//! 方針(`core/README.md` と同じ):
//! - ライセンスは無期限。期限で動作を止めない。
//! - 検証できない間も、最後に成功してから `grace_days`(既定30日)は動かす。
//! - 無料アップデートの権利だけが期限つき(ビルド日が、有効化から `free_update_days` 以内か)。

use crate::error::CoreError;
use std::time::Duration;

pub const DAY: u64 = 86_400;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LicenseStatus {
    Granted,
    Revoked,
    Disabled,
}

/// 販売プラットフォームごとの実装を差し替えるための口。
pub trait LicenseBackend {
    /// キーを有効化し、有効化IDを返す。`label` は端末の名前など(利用者が後で識別できるもの)。
    fn activate(&self, key: &str, label: &str) -> Result<String, CoreError>;
    fn validate(&self, key: &str, activation_id: &str) -> Result<LicenseStatus, CoreError>;
    fn deactivate(&self, key: &str, activation_id: &str) -> Result<(), CoreError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct LicenseState {
    pub key: String,
    pub activation_id: String,
    /// 有効化した時刻(UNIX秒)。無料アップデートの起点として使う。
    /// 要確認: 販売プラットフォームの応答から購入日時を取れるなら、そちらを使うほうが正確。
    pub activated_at: u64,
    pub last_validated_at: u64,
    pub revoked: bool,
}

impl LicenseState {
    pub fn new(key: &str, activation_id: &str, now: u64) -> Self {
        Self {
            key: key.to_string(),
            activation_id: activation_id.to_string(),
            activated_at: now,
            last_validated_at: now,
            revoked: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Policy {
    pub grace_days: u64,
    pub free_update_days: u64,
}

impl Default for Policy {
    fn default() -> Self {
        Self { grace_days: 30, free_update_days: 365 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Access {
    Allowed,
    /// 猶予を過ぎた。再検証(通信)を促す。アプリは止めずに案内を出す想定
    NeedsRevalidation,
    /// 失効・無効化。再度の有効化が必要
    Denied,
}

pub fn check_access(state: &LicenseState, now: u64, p: &Policy) -> Access {
    if state.revoked {
        return Access::Denied;
    }
    // 時計を巻き戻して猶予を延ばす操作への対策
    if now < state.last_validated_at {
        return Access::NeedsRevalidation;
    }
    if now <= state.last_validated_at + p.grace_days * DAY {
        Access::Allowed
    } else {
        Access::NeedsRevalidation
    }
}

/// このビルドを使ってよいか(無料アップデートの範囲か)。
/// `build_unix` はビルド日時(UNIX秒)。範囲外でも、すでに使っている古いバージョンは動き続ける。
pub fn update_allowed(activated_at: u64, build_unix: u64, p: &Policy) -> bool {
    build_unix <= activated_at + p.free_update_days * DAY
}

/// 検証を試み、状態を更新して、現在のアクセス可否を返す。
/// 通信できないときは状態を変えない(猶予内ならそのまま動く)。
pub fn refresh(backend: &dyn LicenseBackend, state: &mut LicenseState, now: u64, p: &Policy) -> Access {
    match backend.validate(&state.key, &state.activation_id) {
        Ok(LicenseStatus::Granted) => {
            state.last_validated_at = now;
            state.revoked = false;
        }
        Ok(LicenseStatus::Revoked) | Ok(LicenseStatus::Disabled) => state.revoked = true,
        Err(CoreError::Rejected(_)) => state.revoked = true,
        Err(_) => {}
    }
    check_access(state, now, p)
}

/// Polar 用の実装。
///
/// **未検証**: 公式文書で確認できたのは、検証のパス(`/v1/customer-portal/license-keys/validate`)と、
/// 台数制限・有効期限・検証APIがあること。有効化・無効化のパスと、応答の形は、
/// テスト商品で実際に通して確かめること(`docs/sales-platforms.md` の「最初の週にやる検証」)。
pub struct PolarBackend {
    base_url: String,
    organization_id: String,
    agent: ureq::Agent,
}

impl PolarBackend {
    pub fn new(organization_id: &str) -> Self {
        Self::with_base_url("https://api.polar.sh", organization_id)
    }

    pub fn with_base_url(base_url: &str, organization_id: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            organization_id: organization_id.to_string(),
            agent: ureq::AgentBuilder::new().timeout(Duration::from_secs(10)).build(),
        }
    }

    fn post(&self, path: &str, body: serde_json::Value) -> Result<serde_json::Value, CoreError> {
        let url = format!("{}{}", self.base_url, path);
        match self.agent.post(&url).send_json(body) {
            Ok(resp) => resp.into_json().map_err(|e| CoreError::Parse(e.to_string())),
            // 鍵が存在しない・使えない・入力が不正: サーバーが明示的に拒否した
            Err(ureq::Error::Status(code, _)) if matches!(code, 403 | 404 | 422) => {
                Err(CoreError::Rejected(format!("HTTP {code}")))
            }
            // 5xxなどは、あとで再試行すれば通る可能性があるので通信エラー扱い
            Err(ureq::Error::Status(code, _)) => Err(CoreError::Network(format!("HTTP {code}"))),
            Err(e) => Err(CoreError::Network(e.to_string())),
        }
    }
}

pub fn map_status(s: &str) -> Result<LicenseStatus, CoreError> {
    match s {
        "granted" => Ok(LicenseStatus::Granted),
        "revoked" => Ok(LicenseStatus::Revoked),
        "disabled" => Ok(LicenseStatus::Disabled),
        other => Err(CoreError::Parse(format!("未知のライセンス状態: {other}"))),
    }
}

impl LicenseBackend for PolarBackend {
    fn activate(&self, key: &str, label: &str) -> Result<String, CoreError> {
        let v = self.post(
            "/v1/customer-portal/license-keys/activate",
            serde_json::json!({ "key": key, "organization_id": self.organization_id, "label": label }),
        )?;
        v.get("id")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| CoreError::Parse("有効化IDが応答にありません".to_string()))
    }

    fn validate(&self, key: &str, activation_id: &str) -> Result<LicenseStatus, CoreError> {
        let v = self.post(
            "/v1/customer-portal/license-keys/validate",
            serde_json::json!({ "key": key, "organization_id": self.organization_id, "activation_id": activation_id }),
        )?;
        let status = v
            .get("status")
            .and_then(|x| x.as_str())
            .ok_or_else(|| CoreError::Parse("応答に status がありません".to_string()))?;
        map_status(status)
    }

    fn deactivate(&self, key: &str, activation_id: &str) -> Result<(), CoreError> {
        self.post(
            "/v1/customer-portal/license-keys/deactivate",
            serde_json::json!({ "key": key, "organization_id": self.organization_id, "activation_id": activation_id }),
        )
        .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    const T0: u64 = 1_800_000_000;

    struct Scripted(RefCell<Vec<Result<LicenseStatus, CoreError>>>);
    impl LicenseBackend for Scripted {
        fn activate(&self, _: &str, _: &str) -> Result<String, CoreError> {
            Ok("act-1".into())
        }
        fn validate(&self, _: &str, _: &str) -> Result<LicenseStatus, CoreError> {
            self.0.borrow_mut().remove(0)
        }
        fn deactivate(&self, _: &str, _: &str) -> Result<(), CoreError> {
            Ok(())
        }
    }

    fn state() -> LicenseState {
        LicenseState::new("KEY", "act-1", T0)
    }

    #[test]
    fn 猶予内は動き_過ぎたら再検証を促す() {
        let p = Policy::default();
        let s = state();
        assert_eq!(check_access(&s, T0 + 29 * DAY, &p), Access::Allowed);
        assert_eq!(check_access(&s, T0 + 30 * DAY, &p), Access::Allowed);
        assert_eq!(check_access(&s, T0 + 30 * DAY + 1, &p), Access::NeedsRevalidation);
    }

    #[test]
    fn 時計の巻き戻しでは猶予が延びない() {
        let p = Policy::default();
        assert_eq!(check_access(&state(), T0 - 1, &p), Access::NeedsRevalidation);
    }

    #[test]
    fn 失効したら拒否() {
        let mut s = state();
        s.revoked = true;
        assert_eq!(check_access(&s, T0, &Policy::default()), Access::Denied);
    }

    #[test]
    fn 無料アップデートの範囲() {
        let p = Policy::default();
        assert!(update_allowed(T0, T0 + 365 * DAY, &p));
        assert!(!update_allowed(T0, T0 + 365 * DAY + 1, &p));
    }

    #[test]
    fn 検証に成功すると猶予が更新される() {
        let p = Policy::default();
        let mut s = state();
        let b = Scripted(RefCell::new(vec![Ok(LicenseStatus::Granted)]));
        let now = T0 + 40 * DAY; // 猶予を過ぎている
        assert_eq!(refresh(&b, &mut s, now, &p), Access::Allowed);
        assert_eq!(s.last_validated_at, now);
    }

    #[test]
    fn 通信できなくても猶予内なら動き続け_状態は変わらない() {
        let p = Policy::default();
        let mut s = state();
        let b = Scripted(RefCell::new(vec![Err(CoreError::Network("offline".into()))]));
        assert_eq!(refresh(&b, &mut s, T0 + 10 * DAY, &p), Access::Allowed);
        assert_eq!(s.last_validated_at, T0);
        assert!(!s.revoked);
    }

    #[test]
    fn 通信できず猶予も過ぎたら再検証を促すだけで_拒否はしない() {
        let p = Policy::default();
        let mut s = state();
        let b = Scripted(RefCell::new(vec![Err(CoreError::Network("offline".into()))]));
        assert_eq!(refresh(&b, &mut s, T0 + 60 * DAY, &p), Access::NeedsRevalidation);
        assert!(!s.revoked);
    }

    #[test]
    fn 返金などで失効した鍵は拒否される() {
        let p = Policy::default();
        for r in [Ok(LicenseStatus::Revoked), Ok(LicenseStatus::Disabled), Err(CoreError::Rejected("HTTP 404".into()))] {
            let mut s = state();
            let b = Scripted(RefCell::new(vec![r]));
            assert_eq!(refresh(&b, &mut s, T0 + DAY, &p), Access::Denied);
            assert!(s.revoked);
        }
    }

    #[test]
    fn 状態の文字列対応() {
        assert_eq!(map_status("granted"), Ok(LicenseStatus::Granted));
        assert_eq!(map_status("revoked"), Ok(LicenseStatus::Revoked));
        assert_eq!(map_status("disabled"), Ok(LicenseStatus::Disabled));
        assert!(map_status("???").is_err());
    }
}
