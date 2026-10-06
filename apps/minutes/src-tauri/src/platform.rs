//! Match the packaged Windows ggml x64 CPU baseline before entering inference.
/// Never mark a working directory as disposable application data when the OS
/// cannot resolve the dedicated location. The app's delete-all action relies on
/// this boundary, so path resolution must fail closed.
pub fn checked_app_data_dir(path: Option<std::path::PathBuf>, identifier: &str) -> Result<std::path::PathBuf, String> {
    let valid_id = !identifier.is_empty() && identifier != "." && identifier != ".."
        && !identifier.contains('/') && !identifier.contains('\\');
    path.filter(|path| valid_id && path.is_absolute() && path.file_name().and_then(|name| name.to_str()) == Some(identifier))
        .ok_or_else(|| "アプリ専用の保存場所を確認できません。ほかのファイルを保護するため、起動を中止しました".to_string())
}

pub fn check_inference_support() -> Result<(), String> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        let supported = std::is_x86_feature_detected!("avx2")
            && std::is_x86_feature_detected!("fma")
            && std::is_x86_feature_detected!("f16c")
            && std::is_x86_feature_detected!("bmi2")
            && std::is_x86_feature_detected!("sse4.2");
        if !supported {
            return Err("このWindowsのCPUは、音声処理に必要な命令（AVX2・FMA・F16C・BMI2・SSE4.2）に対応していません。対応する64ビットPCをお使いください".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::checked_app_data_dir;
    #[test]
    fn 専用の絶対パスだけを保存場所として受け入れる() {
        let path = std::env::temp_dir().join("dev.localaiproduct.minutes");
        assert_eq!(checked_app_data_dir(Some(path.clone()), "dev.localaiproduct.minutes").unwrap(), path);
    }
    #[test]
    fn 未取得_相対パス_一般フォルダーへの保存を拒否する() {
        let id = "dev.localaiproduct.minutes";
        for path in [None, Some(".".into()), Some(id.into()), Some(std::env::temp_dir())] {
            assert!(checked_app_data_dir(path, id).is_err());
        }
        assert!(checked_app_data_dir(Some(std::env::temp_dir()), "..").is_err());
    }
}
