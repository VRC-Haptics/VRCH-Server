#!/usr/bin/env bash
set -euo pipefail

PROJ_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$PROJ_DIR/../.." && pwd)"

case "$(uname -m)" in
    x86_64|amd64)   ARCH=x64   ;;
    aarch64|arm64)  ARCH=arm64 ;;
    *) echo "unsupported machine: $(uname -m)" >&2; exit 1 ;;
esac

case "$(uname -s)" in
    Linux)               RID="linux-$ARCH"; EXT=so    ;;
    Darwin)              RID="osx-$ARCH";   EXT=dylib ;;
    MINGW*|MSYS*|CYGWIN*) RID="win-$ARCH";  EXT=dll   ;;
    *) echo "unsupported OS: $(uname -s)" >&2; exit 1 ;;
esac

LIB="listen-for-vrc.$EXT"
STAGE="$PROJ_DIR/bin/publish-$RID"
OUT="$ROOT/src-tauri/sidecars"

rm -rf "$STAGE"
mkdir -p "$OUT"

# NativeAOT shared library — required for a real .so/.dylib/.dll.
# NativeLib/PublishAot are passed here so the csproj stays portable.
dotnet publish "$PROJ_DIR" \
    -c Release \
    -r "$RID" \
    --self-contained true \
    -p:PublishAot=true \
    -p:NativeLib=Shared \
    -o "$STAGE"

if [ ! -f "$STAGE/$LIB" ]; then
    echo "error: expected $LIB in $STAGE, found:" >&2
    ls -1 "$STAGE" >&2
    exit 1
fi

# Copy only the native artifact; do not pollute sidecars with runtime files.
install -m 0755 "$STAGE/$LIB" "$OUT/$LIB"
echo "built $LIB ($RID) -> $OUT/$LIB"
