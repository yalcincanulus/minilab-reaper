#!/usr/bin/env bash
# Take a webcam shot of the MiniLab 3 with fixed camera settings.
#
# The webcam (Creative Live! Cam Sync 1080p V2) is fixed-focus and forgets its
# settings when unplugged, so this sets them before every shot.
#
#   tools/cam-shot.sh                      full frame
#   tools/cam-shot.sh --rotate 180         turn the image: 0, 90, 180 or 270 (clockwise); remembered for later runs
#   tools/cam-shot.sh --test               first put test text on the screen and colours on the pads
#   tools/cam-shot.sh --crop 400x200+760+80   also save a 4x zoomed crop (WxH+X+Y, in the rotated frame)
#   tools/cam-shot.sh --exposure 300       exposure time, 156-5000 (default 156, the shortest)
#
# Shots go to .scratch/shots/: <time>.jpg (+ <time>-crop.png), and latest.jpg / latest-crop.png.
# The rotation is saved in .scratch/shots/rotation (default 0).

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/.scratch/shots"
ml3="$root/tools/ml3.py"
dev=/dev/video0
exposure=156
crop=""
rotate=""
test=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --crop) crop="$2"; shift 2 ;;
    --exposure) exposure="$2"; shift 2 ;;
    --rotate) rotate="$2"; shift 2 ;;
    --test) test=1; shift ;;
    --device) dev="$2"; shift 2 ;;
    -h|--help) sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

if users="$(fuser "$dev" 2>/dev/null)" && [[ -n "$users" ]]; then
  echo "The webcam is in use by: $(ps -o comm= -p $users | tr '\n' ' ')- close it first." >&2
  exit 1
fi

if [[ $test -eq 1 ]]; then
  python3 "$ml3" hello >/dev/null
  python3 "$ml3" screen "HELLO 0123" "ABCDEFGHIJKLMNOPQR" --play --arm >/dev/null
  python3 "$ml3" rainbow >/dev/null
fi

# Manual exposure: auto blows out the screen and the LEDs.
# Fixed white balance so colours compare between shots. Light sharpening only.
v4l2-ctl -d "$dev" \
  -c auto_exposure=1 -c exposure_time_absolute="$exposure" -c gain=32 \
  -c white_balance_automatic=0 -c white_balance_temperature=4650 \
  -c brightness=0 -c contrast=15 -c saturation=10 -c gamma=188 -c sharpness=3 \
  -c backlight_compensation=0 -c power_line_frequency=1

mkdir -p "$out"
saved="$(cat "$out/rotation" 2>/dev/null || echo 0)"
case "${rotate:-$saved}" in
  0) turn="" ;;
  90) turn=",transpose=clock" ;;
  180) turn=",hflip,vflip" ;;
  270) turn=",transpose=cclock" ;;
  *) echo "--rotate must be 0, 90, 180 or 270" >&2; exit 1 ;;
esac
[[ -n "$rotate" ]] && echo "$rotate" > "$out/rotation"
stamp="$(date +%Y%m%d-%H%M%S)"
shot="$out/$stamp.jpg"
filter="select=gte(n\,30)$turn"  # skip the first frames while the camera settles

ffmpeg -loglevel error -y -f v4l2 -input_format mjpeg -video_size 1920x1080 -i "$dev" \
  -vf "$filter" -frames:v 1 -q:v 2 "$shot"
cp "$shot" "$out/latest.jpg"
echo "$shot"

if [[ -n "$crop" ]]; then
  if [[ ! "$crop" =~ ^([0-9]+)x([0-9]+)\+([0-9]+)\+([0-9]+)$ ]]; then
    echo "--crop must look like WxH+X+Y, e.g. 400x200+760+80" >&2
    exit 1
  fi
  w=${BASH_REMATCH[1]} h=${BASH_REMATCH[2]} x=${BASH_REMATCH[3]} y=${BASH_REMATCH[4]}
  cropped="$out/$stamp-crop.png"
  ffmpeg -loglevel error -y -i "$shot" -vf "crop=$w:$h:$x:$y,scale=iw*4:ih*4:flags=lanczos" "$cropped"
  cp "$cropped" "$out/latest-crop.png"
  echo "$cropped"
fi
