#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
case "$(uname -s)" in
    Linux)  RID=linux-x64;  EXT=so    ;;
    Darwin) RID=osx-arm64;  EXT=dylib ;;
    *)      RID=win-x64;    EXT=dll   ;;
esac
OUT="../../src-tauri/sidecars"
mkdir -p "$OUT"
dotnet publish -c Release -r "$RID" --self-contained true -o "$OUT"
echo "built listen-for-vrc.$EXT -> $OUT"
