#!/usr/bin/env bash
#
# Verifies that every yt-dlp and ffmpeg asset the app downloads on first run still
# exists upstream.
#
# These URLs live in src-tauri/src/ytdlp.rs. They are the one part of the app that can
# break without anybody touching this repo — upstream renames an asset, and the next
# person to install the app gets a failed first run. Keep this list in sync with
# ytdlp_asset_name() and ffmpeg_asset_name().
#
# Usage:  bash scripts/verify-binary-urls.sh

set -uo pipefail

YTDLP_BASE="https://github.com/yt-dlp/yt-dlp/releases/latest/download"
FFMPEG_BASE="https://github.com/eugeneware/ffmpeg-static/releases/latest/download"

ASSETS=(
  "$YTDLP_BASE/yt-dlp.exe"
  "$YTDLP_BASE/yt-dlp_macos"
  "$YTDLP_BASE/yt-dlp_linux"
  "$YTDLP_BASE/yt-dlp_linux_aarch64"
  "$YTDLP_BASE/SHA2-256SUMS"
  "$FFMPEG_BASE/ffmpeg-win32-x64"
  "$FFMPEG_BASE/ffmpeg-darwin-arm64"
  "$FFMPEG_BASE/ffmpeg-darwin-x64"
  "$FFMPEG_BASE/ffmpeg-linux-x64"
  "$FFMPEG_BASE/ffmpeg-linux-arm64"
)

failed=0

for url in "${ASSETS[@]}"; do
  # -L to follow the /latest/download redirect; --fail so a 404 is a non-zero exit.
  code=$(curl -sIL --fail -o /dev/null -w "%{http_code}" "$url" 2>/dev/null)
  status=$?

  if [ $status -eq 0 ] && [ "$code" = "200" ]; then
    printf '  ok    %s\n' "$url"
  else
    printf '  FAIL  %s  (HTTP %s)\n' "$url" "${code:-none}"
    failed=1
  fi
done

if [ $failed -ne 0 ]; then
  cat <<'EOF'

One or more helper binaries could not be found upstream.

This means a brand-new install would fail during first-run setup. Check whether the
upstream project renamed the asset, then update the URL table in
src-tauri/src/ytdlp.rs and the list in this script.
EOF
  exit 1
fi

echo
echo "All helper binary URLs resolve."
