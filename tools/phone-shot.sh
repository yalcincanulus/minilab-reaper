#!/usr/bin/env bash
# Take a photo of the MiniLab 3 screen with the Galaxy M31 over adb.
#
# Uses the Samsung camera app in Pro mode (manual focus and exposure, set once
# by hand on the phone and remembered by the app). Auto exposure, and so
# scrcpy's camera feed, blooms the screen text into blobs.
#
#   tools/phone-shot.sh                       full frame
#   tools/phone-shot.sh --test                first put test text on the screen and colours on the pads
#   tools/phone-shot.sh --crop 760x220+2220+950   also save a crop (WxH+X+Y, in the rotated frame)
#   tools/phone-shot.sh --rotate 0            turn the image: 0, 90, 180 or 270 (clockwise, default 180)
#   tools/phone-shot.sh --ui                  also save a screenshot of the phone's screen (to check the mode)
#
# Shots go to .scratch/shots/: phone-<time>.jpg (+ -crop.png), and phone-latest.jpg / phone-latest-crop.png.
# The photo is deleted from the phone after it is copied.

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/.scratch/shots"
ml3="$root/tools/ml3.py"
camera=com.sec.android.app.camera
dcim=/sdcard/DCIM/Camera
crop=""
rotate=180
test=0
ui=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --crop) crop="$2"; shift 2 ;;
    --rotate) rotate="$2"; shift 2 ;;
    --test) test=1; shift ;;
    --ui) ui=1; shift ;;
    -h|--help) sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

case "$rotate" in
  0) turn="null" ;;
  90) turn="transpose=clock" ;;
  180) turn="hflip,vflip" ;;
  270) turn="transpose=cclock" ;;
  *) echo "--rotate must be 0, 90, 180 or 270" >&2; exit 1 ;;
esac

if ! adb get-state >/dev/null 2>&1; then
  echo "The phone is not connected (adb devices: $(adb devices | sed -n 2p))." >&2
  exit 1
fi

# The tap positions below are for landscape (rotation 1), so keep the phone there.
adb shell settings put system accelerometer_rotation 0
adb shell settings put system user_rotation 1
adb shell input keyevent KEYCODE_WAKEUP

if [[ $test -eq 1 ]]; then
  python3 "$ml3" hello >/dev/null
  python3 "$ml3" screen "HELLO 0123" "ABCDEFGHIJKLMNOPQR" --play --arm >/dev/null
  python3 "$ml3" rainbow >/dev/null
fi

# The camera app always reopens in Photo mode. If it is not in front, open it
# and switch to Pro: "DAHA FAZLA" (More) at the top of the mode strip, then PRO.
# If it is in front, this script left it in Pro mode.
if ! adb shell dumpsys window | grep -q "mCurrentFocus=.*$camera"; then
  adb shell am start -n "$camera/.Camera" >/dev/null
  sleep 4
  adb shell input tap 1790 70
  sleep 2
  adb shell input tap 481 310
  sleep 3
fi

mkdir -p "$out"
stamp="$(date +%Y%m%d-%H%M%S)"
if [[ $ui -eq 1 ]]; then
  adb exec-out screencap -p > "$out/phone-$stamp-ui.png"
  echo "$out/phone-$stamp-ui.png"
fi

before="$(adb shell ls -t "$dcim" | head -1 | tr -d '\r')"
adb shell input keyevent KEYCODE_CAMERA
photo=""
for _ in $(seq 20); do
  sleep 0.5
  photo="$(adb shell ls -t "$dcim" | head -1 | tr -d '\r')"
  [[ -n "$photo" && "$photo" != "$before" ]] && break
  photo=""
done
if [[ -z "$photo" ]]; then
  echo "No new photo on the phone. Check the camera app (run with --ui to see the phone's screen)." >&2
  exit 1
fi
sleep 1  # let the camera app finish writing the file

raw="$out/phone-$stamp-raw.jpg"
shot="$out/phone-$stamp.jpg"
adb pull "$dcim/$photo" "$raw" >/dev/null
adb shell rm "$dcim/$photo"
ffmpeg -loglevel error -y -i "$raw" -vf "$turn" -q:v 2 "$shot"
rm "$raw"
cp "$shot" "$out/phone-latest.jpg"
echo "$shot"

if [[ -n "$crop" ]]; then
  if [[ ! "$crop" =~ ^([0-9]+)x([0-9]+)\+([0-9]+)\+([0-9]+)$ ]]; then
    echo "--crop must look like WxH+X+Y, e.g. 760x220+2220+950" >&2
    exit 1
  fi
  w=${BASH_REMATCH[1]} h=${BASH_REMATCH[2]} x=${BASH_REMATCH[3]} y=${BASH_REMATCH[4]}
  cropped="$out/phone-$stamp-crop.png"
  ffmpeg -loglevel error -y -i "$shot" -vf "crop=$w:$h:$x:$y" "$cropped"
  cp "$cropped" "$out/phone-latest-crop.png"
  echo "$cropped"
fi
