#!/bin/bash
# End-to-end test of the real app. Requires a debug binary (`pnpm tauri build --debug --no-bundle`).
#
# HOME is a throwaway folder on a disposable APFS disk image, so the app's data, approved
# folders and cleanup targets all live there, and items moved to the Trash land in the image's
# own .Trashes folder instead of yours. Docker is a stand-in, because prune acts on a whole
# daemon. Finder's Empty Trash is never called, because it would empty your real Trash.
#
# Usage: e2e/run.sh <path-to-debug-binary> <output-dir>
set -u
BIN="$1"; mkdir -p "$2"; OUT="$(cd "$2" && pwd -P)"; ROOT="$(cd "$(dirname "$0")" && pwd)"
VOL="$OUT/volume"; IMAGE="$OUT/e2e.sparseimage"; FAKE="$VOL/home"; TRASH="$VOL/.Trashes/$(id -u)"

eject() { hdiutil detach "$VOL" -force -quiet 2>/dev/null; rm -f "$IMAGE"; }
trap eject EXIT
eject
rm -rf "$VOL" "$OUT/shots" "$OUT/docker-calls.log"
mkdir -p "$VOL" "$OUT/shots"
hdiutil create -quiet -size 12g -type SPARSE -fs APFS -volname NexusE2E "$OUT/e2e" || exit 1
hdiutil attach -quiet -nobrowse -owners on -mountpoint "$VOL" "$IMAGE" || exit 1
mkdir -p "$FAKE" "$VOL/tmp"

write() { mkdir -p "$(dirname "$FAKE/$1")"; head -c "$2" /dev/urandom > "$FAKE/$1"; }
for i in 1 2 3 4 5 6; do write "Library/Caches/VendorApp$i/cache.bin" 500000; done
write "Library/Caches/com.gone.tool/blob.bin" 1500000
write "Library/Logs/Vendor/app.log" 200000
write ".Trash/old-archive.zip" 300000
write "Library/Keychains/login.keychain-db" 5000
write "Library/Application Support/Google/Chrome/Default/Cookies" 5000
write "Library/Application Support/Google/Chrome/Default/Login Data" 5000
write "Library/Application Support/Google/Chrome/Default/Code Cache/js/a" 200000
write ".ssh/id_ed25519" 400
write ".npm/_cacache/index" 900000
write "Documents/thesis.pdf" 2000000
write "Movies/clip.mov" 140000000
write "project/node_modules/lib/index.js" 22000000
for i in $(seq 1 60); do write "Documents/many/note$i.txt" 2000; done
head -c 4000000 /dev/urandom > "$OUT/dup.bin"
for d in Documents Downloads Desktop; do mkdir -p "$FAKE/$d"; cp "$OUT/dup.bin" "$FAKE/$d/copy.bin"; done
rm "$OUT/dup.bin"
mkfile -n 8g "$FAKE/project/vm-disk.raw"
echo '{"name":"p"}' > "$FAKE/project/package.json"

APP="$FAKE/Applications/DisposableTool.app"
write "Applications/DisposableTool.app/Contents/MacOS/tool" 300000
plutil -create xml1 "$APP/Contents/Info.plist"
plutil -insert CFBundleName -string DisposableTool "$APP/Contents/Info.plist"
plutil -insert CFBundleIdentifier -string com.nexustest.disposable "$APP/Contents/Info.plist"
plutil -insert CFBundleShortVersionString -string 1.0 "$APP/Contents/Info.plist"
write "Library/Application Support/DisposableTool/data.bin" 400000
write "Library/Preferences/com.nexustest.disposable.plist" 2000

cat > "$OUT/docker-stub.sh" <<STUB
#!/bin/sh
echo "\$*" >> "$OUT/docker-calls.log"
case "\$1 \$2" in
  "system df")
    echo '{"Active":"0","Reclaimable":"500MB (100%)","Size":"500MB","TotalCount":"12","Type":"Build Cache"}'
    echo '{"Active":"1","Reclaimable":"0B (0%)","Size":"2GB","TotalCount":"3","Type":"Local Volumes"}'
    ;;
esac
STUB
chmod +x "$OUT/docker-stub.sh"

HOME="$FAKE" TMPDIR="$VOL/tmp/" NEXUS_E2E_SCRIPT="$ROOT/smoke.js" NEXUS_E2E_DOCKER="$OUT/docker-stub.sh" \
  NEXUS_E2E_SLOW_MS=500 "$BIN" > "$OUT/run.log" 2>&1 &
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

report() {
  sed -n "$(($1 + 1)),$2p" "$OUT/run.log" | while read -r line; do
    case "$line" in
      "SHOT "*) WID=$(window_id); [ -n "$WID" ] && screencapture -x -o -l "$WID" "$OUT/shots/${line#SHOT }.png" ;;
      *) echo "$line" ;;
    esac
  done
}

SEEN=0
while kill -0 $PID 2>/dev/null; do
  NEW=$(wc -l < "$OUT/run.log")
  if [ "$NEW" -gt "$SEEN" ]; then report "$SEEN" "$NEW"; SEEN=$NEW; fi
  sleep 0.25
done
wait $PID; CODE=$?
report "$SEEN" "$(wc -l < "$OUT/run.log")"

echo "--- disk assertions"
fail=0
pass() { echo "PASS $1"; }
miss() { echo "FAIL $1"; fail=1; }
gone() { if [ -e "$FAKE/$1" ]; then miss "still present: $1"; else pass "removed: $1"; fi; }
kept() { if [ -e "$FAKE/$1" ]; then pass "untouched: $1"; else miss "missing: $1"; fi; }
trashed() { if [ -e "$TRASH/$1" ]; then pass "in the Trash: $1"; else miss "not in the Trash: $1"; fi; }

for i in 1 2 3 4 5 6; do gone "Library/Caches/VendorApp$i"; done
gone "Library/Logs/Vendor"
gone ".Trash/old-archive.zip"
gone "Movies/clip.mov"
gone "Applications/DisposableTool.app"
gone "Library/Application Support/DisposableTool"
kept "Library/Caches/com.gone.tool"
kept "Library/Preferences/com.nexustest.disposable.plist"
kept "Library/Keychains/login.keychain-db"
kept "Library/Application Support/Google/Chrome/Default/Cookies"
kept "Library/Application Support/Google/Chrome/Default/Login Data"
kept ".ssh/id_ed25519"
kept ".npm/_cacache/index"
kept "Documents/thesis.pdf"
kept "project/node_modules/lib/index.js"
kept "report.md"
trashed "clip.mov"
trashed "DisposableTool.app"
trashed "DisposableTool"

left=0; for d in Documents Downloads Desktop; do [ -e "$FAKE/$d/copy.bin" ] && left=$((left + 1)); done
binned=$(find "$TRASH" -maxdepth 1 -name 'copy*.bin' | wc -l | tr -d ' ')
if [ "$left" -eq 1 ] && [ "$binned" -eq 2 ]; then pass "one duplicate kept, two in the Trash"; else miss "duplicates: $left kept, $binned in the Trash"; fi

CALLS=$(cat "$OUT/docker-calls.log" 2>/dev/null)
if echo "$CALLS" | grep -q '^builder prune -f$'; then pass "Docker prune ran for the selected resource"; else miss "Docker prune did not run"; fi
if echo "$CALLS" | grep -q 'volume prune'; then miss "Docker volumes were pruned without being selected"; else pass "Docker volumes left alone"; fi

echo "exit=$CODE disk_failures=$fail"
[ "$CODE" -eq 0 ] && [ "$fail" -eq 0 ]
