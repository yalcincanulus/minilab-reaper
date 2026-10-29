#!/usr/bin/env bash
# Fake MiniLab input for Reaper: messages sent to ALSA's "Midi Through" port reach Reaper on the same input as the
# MiniLab (device "Minilab3: MIDI"), so scripts and the extension see them as coming from the device.
#
#   tools/fake-input.sh link            connect Midi Through into Reaper's MiniLab input (Reaper must be running)
#   tools/fake-input.sh send B0 1D 41   send one message (hex), e.g. Shift + encoder right
#   tools/fake-input.sh unlink          remove the connection
#
# Useful messages (DAW program): Shift+encoder B0 1D 41/3F, Shift+click B0 77 7F, fader 1 B0 0E <v>,
# knob 1 B0 56 <v>, Shift+play B0 6B 7F, Shift+stop B0 6A 7F, pad 1 99 24 64.

set -euo pipefail

through="Midi-Bridge:Midi Through: Port-0 (capture)"
reaper_in() { pw-link -l | awk '/Minilab3: MIDI \(capture\)/{f=1;next} f&&/\|->/&&!done{sub(/.*\|-> /,""); print; done=1}'; }

case "${1:-}" in
  link)
    target="$(reaper_in)"
    [[ -n "$target" ]] || { echo "Reaper's MiniLab input is not connected (is Reaper running?)." >&2; exit 1; }
    pw-link "$through" "$target" 2>/dev/null || true
    echo "Midi Through -> $target" ;;
  unlink)
    target="$(reaper_in)"
    [[ -n "$target" ]] && pw-link -d "$through" "$target" 2>/dev/null || true ;;
  send)
    shift
    aseqsend -p 14:0 "$*" ;;
  *) sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'; exit 1 ;;
esac
