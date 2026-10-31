#!/bin/bash
# End-to-end smoke test of the real app. Requires a debug binary (`pnpm tauri build --debug --no-bundle`).
# Everything destructive happens inside a throwaway HOME: the app's data, approved folders and
# cleanup targets all live there. Usage: e2e/run.sh <path-to-debug-binary> <output-dir>
set -u
BIN="$1"; OUT="$2"; ROOT="$(cd "$(dirname "$0")" && pwd)"
FAKE="$OUT/home"; rm -rf "$FAKE"; mkdir -p "$FAKE" "$OUT/tmp" "$OUT/shots"

write() { mkdir -p "$(dirname "$FAKE/$1")"; head -c "$2" /dev/urandom > "$FAKE/$1"; }
write "Library/Caches/VendorApp/cache.bin" 3000000
write "Library/Caches/com.gone.tool/blob.bin" 1500000
write "Library/Logs/Vendor/app.log" 200000
write "Library/Keychains/login.keychain-db" 5000
write "Library/Application Support/Google/Chrome/Default/Cookies" 5000
write "Library/Application Support/Google/Chrome/Default/Login Data" 5000
write "Library/Application Support/Google/Chrome/Default/Code Cache/js/a" 200000
write ".ssh/id_ed25519" 400
write ".npm/_cacache/index" 900000
write "Documents/thesis.pdf" 2000000
write "Movies/clip.mov" 140000000
write "project/node_modules/lib/index.js" 22000000
head -c 4000000 /dev/urandom > "$OUT/dup.bin"
for d in Documents Downloads Desktop; do mkdir -p "$FAKE/$d"; cp "$OUT/dup.bin" "$FAKE/$d/copy.bin"; done
rm "$OUT/dup.bin"
mkfile -n 8g "$FAKE/project/vm-disk.raw"
echo '{"name":"p"}' > "$FAKE/project/package.json"

HOME="$FAKE" TMPDIR="$OUT/tmp/" NEXUS_E2E_SCRIPT="$ROOT/smoke.js" "$BIN" > "$OUT/run.log" 2>&1 &
PID=$!

window_id() {
  /usr/bin/swift - <<SWIFT 2>/dev/null
import CoreGraphics
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
for w in list where (w["kCGWindowOwnerPID"] as? Int) == $PID && (w["kCGWindowLayer"] as? Int) == 0 {
    print(w["kCGWindowNumber"] as! Int); break
}
SWIFT
}

WID=""; DONE=0; SEEN=0
while kill -0 $PID 2>/dev/null; do
  NEW=$(wc -l < "$OUT/run.log")
  if [ "$NEW" -gt "$SEEN" ]; then
    sed -n "$((SEEN + 1)),${NEW}p" "$OUT/run.log" | while read -r line; do
      case "$line" in
        "SHOT "*) [ -n "${WID:-}" ] || WID=$(window_id); screencapture -x -o -l "$WID" "$OUT/shots/${line#SHOT }.png" ;;
        *) echo "$line" ;;
      esac
    done
    SEEN=$NEW
  fi
  sleep 0.25
done
wait $PID; CODE=$?
sed -n "$((SEEN + 1)),\$p" "$OUT/run.log" | grep -v '^SHOT '

echo "--- disk assertions"
fail=0
gone() { if [ -e "$FAKE/$1" ]; then echo "FAIL still present: $1"; fail=1; else echo "PASS removed: $1"; fi; }
kept() { if [ -e "$FAKE/$1" ]; then echo "PASS untouched: $1"; else echo "FAIL missing: $1"; fail=1; fi; }
gone "Library/Caches/VendorApp"
gone "Library/Logs/Vendor"
kept "Library/Caches/com.gone.tool"
kept "Library/Keychains/login.keychain-db"
kept "Library/Application Support/Google/Chrome/Default/Cookies"
kept "Library/Application Support/Google/Chrome/Default/Login Data"
kept ".ssh/id_ed25519"
kept ".npm/_cacache/index"
kept "Documents/thesis.pdf"
kept "Movies/clip.mov"
kept "project/node_modules/lib/index.js"
kept "Documents/copy.bin"; kept "Downloads/copy.bin"; kept "Desktop/copy.bin"
kept "report.md"
echo "exit=$CODE disk_failures=$fail"
[ "$CODE" -eq 0 ] && [ "$fail" -eq 0 ]
