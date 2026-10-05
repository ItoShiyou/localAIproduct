# whisper.cpp / llama.cpp(ggml)の CPU 命令を、ビルドした機械に合わせず、固定の基準(x86-64 の AVX2・FMA・F16C・BMI2。AVX-512 は使わない)にする。
# CMAKE_PROJECT_INCLUDE で読ませる(whisper-rs-sys は GGML_ で始まる環境変数を CMake に渡さないが、CMAKE_ で始まるものは渡すため)。
# 理由: MSVC で GGML_NATIVE が ON(既定)だと、ビルドした機械の CPU を調べて命令を決める。CI のランナーは機種が日によって違い、
#       AVX-512 のある機種でビルドされると、AVX-512 の無い CPU(例: Intel 第12世代 i5-12500)で「不正な命令」で落ちる配布物になる。
# 基準: AVX2 + FMA + F16C + BMI2 = Intel Haswell(2013)以降、AMD Excavator / Zen 以降。それより古い CPU では動かない。
set(GGML_NATIVE OFF CACHE BOOL "" FORCE)
set(GGML_SSE42 ON CACHE BOOL "" FORCE)
set(GGML_AVX ON CACHE BOOL "" FORCE)
set(GGML_AVX2 ON CACHE BOOL "" FORCE)
set(GGML_BMI2 ON CACHE BOOL "" FORCE)
set(GGML_FMA ON CACHE BOOL "" FORCE)
set(GGML_F16C ON CACHE BOOL "" FORCE)
set(GGML_AVX512 OFF CACHE BOOL "" FORCE)
