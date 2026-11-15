#!/usr/bin/env bash
#
# Bundle VideoWallpaperService (Java) into the cargo-apk output APK.
# Linux/CI twin of package-service-apk.ps1 — see AndroidManifest-service.xml
# for why the manifest + full resource set must be re-linked with aapt2 in one
# pass and merged back into the base APK here.
#
# Usage: packaging/android/package-service-apk.sh [input.apk] [output.apk]
#
# Requires: ANDROID_HOME (SDK with build-tools;34.0.0 + platforms;android-34),
#           JDK (javac), python3, and the signing keystore env vars (same names
#           cargo-apk uses): CARGO_APK_RELEASE_KEYSTORE,
#           CARGO_APK_RELEASE_KEYSTORE_PASSWORD, CARGO_APK_RELEASE_KEYSTORE_KEY_PASSWORD.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
input_apk="${1:-$repo_root/target/release/apk/wallpaper-rs.apk}"
output_apk="${2:-$repo_root/target/release/apk/wallpaper-rs-android-video.apk}"

sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-/usr/local/lib/android/sdk}}"
build_tools="$sdk/build-tools/34.0.0"
platform="$sdk/platforms/android-34/android.jar"
work="$repo_root/target/release/apk/service-work"

if [[ ! -f "$input_apk" ]]; then
    echo "找不到输入 APK：$input_apk（先运行 cargo apk build --release --lib）" >&2
    exit 1
fi
: "${CARGO_APK_RELEASE_KEYSTORE:?需要 CARGO_APK_RELEASE_KEYSTORE 指定签名密钥}"

rm -rf "$work"
mkdir -p "$work/classes" "$work/res" "$work/compiled-res"

# 1) 资源：服务资源 + 应用资源（必须与清单一起在同一次 aapt2 link 里编译，
#    否则 @mipmap/ic_launcher 等资源 id 会与清单对不上）。
cp -r "$repo_root/packaging/android/res/." "$work/res/"
cp -r "$repo_root/assets/res/." "$work/res/"

# 2) 合并清单的版本号取自 base APK（系统按 versionCode 判断升级安装）。
badging="$("$build_tools/aapt" dump badging "$input_apk")"
version_code="$(sed -n "s/.*versionCode='\([0-9]*\)'.*/\1/p" <<<"$badging" | head -n1)"
version_name="$(sed -n "s/.*versionName='\([^']*\)'.*/\1/p" <<<"$badging" | head -n1)"
[[ -n "$version_code" && -n "$version_name" ]] || {
    echo "无法从 $input_apk 解析 versionCode / versionName" >&2
    exit 1
}
sed -e "s/__VERSION_CODE__/$version_code/" -e "s/__VERSION_NAME__/$version_name/" \
    "$repo_root/packaging/android/AndroidManifest-service.xml" > "$work/AndroidManifest-service.xml"
echo "合并清单版本：versionCode=$version_code versionName=$version_name"

# 3) Java -> dex。
mapfile -t java_files < <(find "$repo_root/packaging/android/java" -name '*.java')
javac -encoding UTF-8 --release 8 -cp "$platform" -d "$work/classes" "${java_files[@]}"
mapfile -t class_files < <(find "$work/classes" -name '*.class')
"$build_tools/d8" --release --lib "$platform" --min-api 24 --output "$work/classes" \
    "${class_files[@]}"

# 4) aapt2：编译资源并连同清单链接。
"$build_tools/aapt2" compile --dir "$work/res" -o "$work/resources.zip"
"$build_tools/aapt2" link -I "$platform" --manifest "$work/AndroidManifest-service.xml" \
    -o "$work/service-res.apk" "$work/resources.zip"

# 5) 合并：resources.arsc 必须 STORED（Android 11+ 要求未压缩且对齐）；
#    清单 / 资源 / classes.dex 来自服务构建，其余（native 库等）来自 base APK。
python3 - "$input_apk" "$work/service-res.apk" "$work/classes/classes.dex" "$work/merged.apk" <<'PY'
import sys
import zipfile


def copy_entry(src, name, dst, compress_type):
    info = zipfile.ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
    info.compress_type = compress_type
    info.external_attr = 0o644 << 16
    dst.writestr(info, src.read(name))


base_path, svc_path, dex_path, out_path = sys.argv[1:5]
with zipfile.ZipFile(base_path) as base, zipfile.ZipFile(svc_path) as svc, \
        zipfile.ZipFile(out_path, "w") as out:
    svc_names = set(svc.namelist())
    # (1) resources.arsc 放最前，保持未压缩。
    copy_entry(svc, "resources.arsc", out, zipfile.ZIP_STORED)
    # (2) 服务 APK 的清单与资源（覆盖 base 同名条目）。
    for name in svc.namelist():
        if name != "resources.arsc":
            copy_entry(svc, name, out, zipfile.ZIP_DEFLATED)
    # (3) base 的其余内容（native 库等）；跳过被替换的条目。
    for info in base.infolist():
        name = info.filename
        if name in ("resources.arsc", "classes.dex") or name in svc_names:
            continue
        copy_entry(base, name, out, zipfile.ZIP_DEFLATED)
    # (4) 壁纸服务的字节码。
    with open(dex_path, "rb") as f:
        dex = f.read()
    info = zipfile.ZipInfo("classes.dex", date_time=(2026, 1, 1, 0, 0, 0))
    info.compress_type = zipfile.ZIP_DEFLATED
    out.writestr(info, dex)
PY

# 6) 对齐 + 签名（与 cargo-apk 同一密钥，保证覆盖安装可升级）。
"$build_tools/zipalign" -f -p 4 "$work/merged.apk" "$work/aligned.apk"
"$build_tools/apksigner" sign \
    --ks "$CARGO_APK_RELEASE_KEYSTORE" \
    --ks-pass "pass:${CARGO_APK_RELEASE_KEYSTORE_PASSWORD:-android}" \
    --key-pass "pass:${CARGO_APK_RELEASE_KEYSTORE_KEY_PASSWORD:-android}" \
    --out "$output_apk" "$work/aligned.apk"
"$build_tools/apksigner" verify --verbose "$output_apk"
"$build_tools/aapt" dump badging "$output_apk" \
    | grep -E "native-code|sdkVersion|uses-permission|provides-component"
echo "Packaged: $output_apk"
