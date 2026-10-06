//! Match the packaged Windows ggml x64 CPU baseline before entering inference.
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
