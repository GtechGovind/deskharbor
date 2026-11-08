#!/usr/bin/env bash
set -euo pipefail
# Developer build helper. Fetches the exact upstream source revision and builds
# only VP8; it does not install software or alter system permissions.
mode="${1:?linux|macos|android|windows-project}"
root="${2:?absolute task-owned build directory}"
case "$root" in /*) ;; *) echo 'build directory must be absolute' >&2; exit 2;; esac
case "$root" in *[[:space:]]*) echo 'libvpx build directory must not contain whitespace' >&2; exit 2;; esac
commit=1024874c5919305883187e2953de8fcb4c3d7fa6
mkdir -p "$root"
source_dir="$root/source"
build_dir="$root/build"
if [[ ! -d "$source_dir/.git" ]]; then
  if [[ -e "$source_dir" ]]; then echo 'source path already exists without its pinned checkout' >&2; exit 2; fi
  git -c core.autocrlf=false -c core.eol=lf clone --depth 1 --branch v1.16.0 https://chromium.googlesource.com/webm/libvpx "$source_dir"
fi
[[ "$(git -C "$source_dir" rev-parse HEAD)" == "$commit" ]] || { echo 'native dependency revision mismatch' >&2; exit 2; }
git -C "$source_dir" diff --exit-code --quiet
git -C "$source_dir" diff --cached --exit-code --quiet
mkdir -p "$build_dir"
cd "$build_dir"
args=(--disable-examples --disable-tools --disable-docs --disable-unit-tests --disable-vp9 --disable-libyuv --disable-webm-io --enable-realtime-only --size-limit=4096x4096)
case "$mode" in
  windows-project)
    bash ../source/configure --target=x86_64-win64-vs17 --enable-external-build --as=nasm "${args[@]}"
    # External-build configuration intentionally skips compiler detection. Supply
    # only the MSVC project generator's supported defines/includes, not GCC -O3.
    make -j2 NO_LAUNCH_DEVENV=1 'CFLAGS=-DNDEBUG -I. -I../source'
    ;;
  linux)
    bash ../source/configure --target=x86_64-linux-gcc --enable-pic --as=nasm "${args[@]}"
    make -j2
    ;;
  macos)
    case "$(uname -m)" in arm64) target=arm64-darwin-gcc;; x86_64) target=x86_64-darwin20-gcc;; *) exit 2;; esac
    bash ../source/configure --target="$target" --enable-pic "${args[@]}"
    make -j2
    ;;
  android)
    : "${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME to the pinned installed NDK}"
    : "${ANDROID_ABI:=arm64-v8a}"
    case "$ANDROID_ABI" in
      arm64-v8a) target=arm64-android-gcc; triple=aarch64-linux-android30;;
      x86_64) target=x86_64-android-gcc; triple=x86_64-linux-android30;;
      *) echo 'unsupported Android ABI' >&2; exit 2;;
    esac
    ndk_bin="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
    export CC="$ndk_bin/$triple-clang" CXX="$ndk_bin/$triple-clang++" AR="$ndk_bin/llvm-ar" LD="$ndk_bin/ld.lld" STRIP="$ndk_bin/llvm-strip"
    export PATH="$ndk_bin:$PATH"
    bash ../source/configure --target="$target" --enable-pic "${args[@]}"
    make -j2
    ;;
  *) echo 'unknown native build mode' >&2; exit 2;;
esac
printf 'VPX_INCLUDE_DIR=%s\nVPX_LIB_DIR=%s\n' "$source_dir" "$build_dir"
