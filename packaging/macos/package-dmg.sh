#!/usr/bin/env bash
set -euo pipefail

app_name="Koko"
bundle_id="dev.lizardbyte.app.Koko"
target=""
version=""
binary_path=""
output_dir="artifacts"
work_dir="target/macos-package"
sign_bundle="false"
codesign_identity="${APPLE_CODESIGN_IDENTITY:-}"

function usage() {
  cat <<EOF
Usage: $0 --target TARGET --version VERSION [options]

Options:
  --target TARGET       Rust target triple, such as aarch64-apple-darwin.
  --version VERSION     Version to write into Info.plist.
  --binary PATH         Path to the built Koko binary.
  --output-dir PATH     Directory for the generated DMG. Default: artifacts.
  --sign                Sign the app bundle and DMG with APPLE_CODESIGN_IDENTITY.
  -h, --help            Show this help text.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target)
      target="$2"
      shift 2
      ;;
    --target=*)
      target="${1#*=}"
      shift
      ;;
    --version)
      version="$2"
      shift 2
      ;;
    --version=*)
      version="${1#*=}"
      shift
      ;;
    --binary)
      binary_path="$2"
      shift 2
      ;;
    --binary=*)
      binary_path="${1#*=}"
      shift
      ;;
    --output-dir)
      output_dir="$2"
      shift 2
      ;;
    --output-dir=*)
      output_dir="${1#*=}"
      shift
      ;;
    --sign)
      sign_bundle="true"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "${target}" ]]; then
  target="$(rustc -vV | sed -n 's/^host: //p')"
fi

if [[ -z "${version}" ]]; then
  version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
fi

if [[ -z "${binary_path}" ]]; then
  binary_path="target/${target}/release/koko"
fi

if [[ -z "${target}" || -z "${version}" ]]; then
  echo "Both --target and --version must resolve to non-empty values." >&2
  exit 1
fi

if [[ ! -f "${binary_path}" ]]; then
  echo "Koko binary not found: ${binary_path}" >&2
  exit 1
fi
chmod +x "${binary_path}"

if [[ ! -f "crates/client-web/dist/index.html" ]]; then
  echo "Web client bundle not found. Run npm run build in crates/client-web first." >&2
  exit 1
fi

if [[ "${sign_bundle}" == "true" && -z "${codesign_identity}" ]]; then
  echo "APPLE_CODESIGN_IDENTITY must be set when --sign is used." >&2
  exit 1
fi

bundle_version="$(
  printf '%s' "${version}" \
    | sed -E 's/^[vV]//; s/[^0-9.]/./g; s/\.+/./g; s/^\.//; s/\.$//'
)"
if [[ -z "${bundle_version}" ]]; then
  bundle_version="0"
fi

package_dir="${work_dir}/${target}"
app_dir="${package_dir}/${app_name}.app"
contents_dir="${app_dir}/Contents"
macos_dir="${contents_dir}/MacOS"
resources_dir="${contents_dir}/Resources"
dmg_root="${package_dir}/dmg-root"
dmg_path="${output_dir}/koko-${target}.dmg"

rm -rf "${package_dir}"
mkdir -p "${macos_dir}" "${resources_dir}" "${dmg_root}" "${output_dir}"

install -m 0755 "${binary_path}" "${macos_dir}/koko"
ditto "assets" "${resources_dir}/assets"
ditto "crates/client-web/dist" "${resources_dir}/client-web/dist"
install -m 0644 "LICENSE" "${resources_dir}/LICENSE"

iconset_dir="${package_dir}/${app_name}.iconset"
mkdir -p "${iconset_dir}"
for size in 16 32 128 256 512; do
  sips -z "${size}" "${size}" "assets/Koko.png" \
    --out "${iconset_dir}/icon_${size}x${size}.png" >/dev/null

  retina_size=$((size * 2))
  if [[ "${retina_size}" -le 512 ]]; then
    sips -z "${retina_size}" "${retina_size}" "assets/Koko.png" \
      --out "${iconset_dir}/icon_${size}x${size}@2x.png" >/dev/null
  fi
done
iconutil -c icns "${iconset_dir}" -o "${resources_dir}/${app_name}.icns"

cat > "${contents_dir}/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>${app_name}</string>
  <key>CFBundleExecutable</key>
  <string>koko</string>
  <key>CFBundleIconFile</key>
  <string>${app_name}.icns</string>
  <key>CFBundleIdentifier</key>
  <string>${bundle_id}</string>
  <key>CFBundleName</key>
  <string>${app_name}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${bundle_version}</string>
  <key>CFBundleVersion</key>
  <string>${bundle_version}</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.entertainment</string>
  <key>LSMinimumSystemVersion</key>
  <string>14.2</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSLocalNetworkUsageDescription</key>
  <string>${app_name} serves your media library on your local network.</string>
</dict>
</plist>
EOF
plutil -lint "${contents_dir}/Info.plist"

if [[ "${sign_bundle}" == "true" ]]; then
  xattr -rc "${app_dir}"
  codesign --force --timestamp --options runtime \
    --sign "${codesign_identity}" \
    "${macos_dir}/koko"
  codesign --force --timestamp --options runtime \
    --sign "${codesign_identity}" \
    "${app_dir}"
  codesign --verify --deep --strict --verbose=2 "${app_dir}"
fi

ditto "${app_dir}" "${dmg_root}/${app_name}.app"
ln -s /Applications "${dmg_root}/Applications"

rm -f "${dmg_path}"
if ! hdiutil create -volname "${app_name}" -srcfolder "${dmg_root}" -ov -format UDZO "${dmg_path}"; then
  echo "hdiutil failed, retrying once..." >&2
  sleep 5
  hdiutil create -volname "${app_name}" -srcfolder "${dmg_root}" -ov -format UDZO "${dmg_path}"
fi

if [[ "${sign_bundle}" == "true" ]]; then
  codesign --force --timestamp \
    --sign "${codesign_identity}" \
    "${dmg_path}"
  codesign --verify --verbose=2 "${dmg_path}"
fi

echo "Created ${dmg_path}"
