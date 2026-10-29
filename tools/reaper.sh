#!/usr/bin/env bash
# Drive the system Reaper from a shell (KDE Wayland; Reaper runs on XWayland).
#
#   tools/reaper.sh start [project.rpp]   start Reaper (default: reaper/test.RPP) unless it is running
#   tools/reaper.sh run script.lua        run a ReaScript inside the running Reaper
#   tools/reaper.sh shot                  screenshot of Reaper's main window only -> .scratch/shots/reaper-latest.png
#   tools/reaper.sh quit [--discard]      quit Reaper; asks to save if the project changed, unless --discard
#
# Lua scripts have no stdout here: write results to a file and read it back.

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/.scratch/shots"
# Xwayland's auth file; the shell started by Claude Code doesn't get XAUTHORITY.
XAUTHORITY="${XAUTHORITY:-$(ls /run/user/"$(id -u)"/xauth_* 2>/dev/null | head -1)}"
export XAUTHORITY

main_window() {
  for w in $(xprop -root _NET_CLIENT_LIST | grep -o '0x[0-9a-f]*'); do
    xprop -id "$w" WM_NAME 2>/dev/null | grep -q 'REAPER v' && { echo "$w"; return; }
  done
  return 1
}

case "${1:-}" in
  start)
    if pgrep -x reaper >/dev/null; then echo "Reaper is already running."; exit 0; fi
    mkdir -p "$out"
    nohup reaper -nosplash "${2:-$root/reaper/test.RPP}" > "$out/reaper-stdout.log" 2>&1 &
    for _ in $(seq 30); do main_window >/dev/null && { echo "Reaper started."; exit 0; }; sleep 0.5; done
    echo "Reaper did not open a window in 15 s. See $out/reaper-stdout.log" >&2; exit 1 ;;
  run)
    [[ -f "${2:-}" ]] || { echo "usage: $0 run script.lua" >&2; exit 1; }
    reaper -nonewinst "$(realpath "$2")" >/dev/null 2>&1 ;;
  shot)
    w="$(main_window)" || { echo "No Reaper window." >&2; exit 1; }
    mkdir -p "$out"
    shot="$out/reaper-$(date +%Y%m%d-%H%M%S).png"
    import -window "$w" "$shot"
    cp "$shot" "$out/reaper-latest.png"
    echo "$shot" ;;
  quit)
    mode=":exit"
    [[ "${2:-}" == "--discard" ]] && mode=":nosave:exit"
    reaper -nonewinst "-closeall$mode" >/dev/null 2>&1 || true
    for _ in $(seq 20); do pgrep -x reaper >/dev/null || { echo "Reaper closed."; exit 0; }; sleep 0.5; done
    echo "Reaper is still running (a save prompt may be open on screen)." >&2; exit 1 ;;
  *) sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'; exit 1 ;;
esac
