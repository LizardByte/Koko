#!/bin/sh
export PATH="/app/ffmpeg/bin:${PATH}"
export LD_LIBRARY_PATH="/app/libexec/koko/live-tv${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
export KOKO_ASSETS_DIR="${KOKO_ASSETS_DIR:-/app/share/koko/assets}"
export KOKO_WEB_CLIENT_DIST="${KOKO_WEB_CLIENT_DIST:-/app/share/koko/client-web}"
exec /app/libexec/koko/koko "$@"
