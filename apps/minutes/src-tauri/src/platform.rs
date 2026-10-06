//! Match the packaged Windows ggml x64 CPU baseline before entering inference.
/// Keep speech inference within the machine's usable logical CPU budget.
/// In particular, the historical fixed six threads oversubscribed four-core
/// Windows runners. Invalid developer overrides must not reach native code.
pub fn speech_threads(requested: Option<i32>, available: usize) -> i32 {
    requested.filter(|threads| *threads > 0).unwrap_or(6)
        .min(available.clamp(1, 32) as i32)
}

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
    fn 音声処理のスレッドは正の数で利用可能なコア数を超えない() {
        use super::speech_threads;
        assert_eq!(speech_threads(None, 4), 4);
        assert_eq!(speech_threads(None, 8), 6);
        assert_eq!(speech_threads(Some(2), 8), 2);
        assert_eq!(speech_threads(Some(0), 4), 4);
        assert_eq!(speech_threads(Some(-1), 4), 4);
        assert_eq!(speech_threads(Some(i32::MAX), 4), 4);
        assert_eq!(speech_threads(None, 0), 1);
    }
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
