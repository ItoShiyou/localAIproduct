#!/bin/sh
# Build the macOS library only; keep Windows runtime assets unchanged.
set -eu
source_dir=${1:?usage: build_ort_macos12.sh SOURCE_DIR BUILD_DIR}
build_dir=${2:?usage: build_ort_macos12.sh SOURCE_DIR BUILD_DIR}
expected=f2c39fe2f838cf35ce7da92824f5a5e3ee6e88a7
[ "$(git -C "$source_dir" rev-parse HEAD)" = "$expected" ] || { echo 'ONNX Runtime v1.30.0 pinned revision required' >&2; exit 1; }
[ -x "$source_dir/.venv/bin/python" ] || { echo 'Create an isolated Python virtual environment at SOURCE_DIR/.venv first' >&2; exit 1; }
cd "$source_dir"
MACOSX_DEPLOYMENT_TARGET=12.0 "$source_dir/.venv/bin/python" tools/ci_build/build.py \
  --config Release --update --build --build_shared_lib --parallel 4 --skip_tests --no_telemetry \
  --cmake_generator 'Unix Makefiles' --apple_deploy_target 12.0 --osx_arch arm64 \
  --build_dir "$build_dir" \
  --cmake_extra_defines CMAKE_OSX_DEPLOYMENT_TARGET=12.0 onnxruntime_BUILD_UNIT_TESTS=OFF
