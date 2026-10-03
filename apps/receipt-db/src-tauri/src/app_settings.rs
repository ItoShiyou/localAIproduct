//! アプリ固有の設定まわり。通信一覧は SPEC.md の「通信一覧」と同じ内容にそろえる。

use factory_core::settings::{AppData, NetworkEntry};

pub const UPDATE_PURPOSE: &str = "更新の確認";

pub fn network_entries() -> Vec<NetworkEntry> {
    let e = |p: &str, d: &str, c: &str, stoppable: bool| NetworkEntry {
        purpose: p.into(), destination: d.into(), content: c.into(), stoppable, enabled: true,
    };
    vec![
        e("ライセンス認証・検証", "販売プラットフォームのAPI", "ライセンスキー、端末の識別名", false),
        e(UPDATE_PURPOSE, "配布元", "アプリのバージョン", true),
        e("モデルの取得(操作したときのみ)", "モデルの配布元", "モデル名", false),
    ]
}

/// 設定画面に出す通信一覧(更新の確認のオン/オフを反映)。領収書・画像・読み取り結果を送る項目は無い。
pub fn network_list(app: &AppData) -> Vec<NetworkEntry> {
    app.network_list(&network_entries(), UPDATE_PURPOSE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use factory_core::settings::Settings;

    #[test]
    fn 通信一覧は3件で_更新確認だけ止められ_設定が反映される() {
        let d = std::env::temp_dir().join(format!("rdb-set-{}", std::process::id()));
        let app = AppData::init(&d).unwrap();
        let l = network_list(&app);
        assert_eq!(l.len(), 3);
        assert_eq!(l.iter().filter(|e| e.stoppable).count(), 1);
        app.save_settings(&Settings { update_check: false, ..Default::default() }).unwrap();
        assert!(!network_list(&app).iter().find(|e| e.purpose == UPDATE_PURPOSE).unwrap().enabled);
        std::fs::remove_dir_all(&d).ok();
    }
}
