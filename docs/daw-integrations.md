# What Arturia's DAW integrations do

What the MiniLab 3 does in each DAW Arturia supports. How Reaper with this extension compares:
`docs/daw-comparison.md`.

All integrations need the MiniLab in its DAW program (Shift + Pad 3).

## Sources

- **Arturia's integration guides 1.0** for Live, Bitwig Studio, FL Studio, Logic Pro, Reason and Cubase:
  `https://www.arturia.com/api/download/products/minilab-3/daw-integration-guide/MiniLab_3_<DAW>_Daw_Integration_Guide_1_0_EN.pdf`
  with `Live`, `Bitwig_Studio`, `FL_Studio`, `Logic_Pro`, `Reason`, `Cubase`.
- **The integration scripts**, from the same resources page
  (`https://www.arturia.com/products/hybrid-synths/minilab-3/resources`):
  - FL Studio 1.0.1: Python.
  - Cubase 1.0.1: MIDI Remote JavaScript, inside a zlib-compressed `.midiremote` file.
  - Reason 1.0: a Lua codec and a remote map.
  - Logic Pro 1.0: `config.lua`.
  - Bitwig Studio 1.1 (2026-06-26): a Java `.bwextension`, with Bitwig's own MiniLab 3 manual inside.
  - Ableton Live: Ableton's script, decompiled (see `docs/minilab3-protocol.md`).

The guides are short and partly copied between DAWs: Bitwig's 1.0 guide gives Shift + click the FL Studio text
("switch between Channel Rack and Browser"). Where a guide and a script disagree, this page follows the script,
because it is what runs. Bitwig is described as of 1.1, from the manual in the extension.

## By control

| Control | Live | Bitwig 1.1 | FL Studio | Logic Pro | Reason | Cubase |
|---|---|---|---|---|---|---|
| Encoder turn | Scenes (Session view); timeline (Arrangement view) | Scenes (bank A); pad note range (bank B) | Channel Rack / Browser items | **Tracks** | **Tracks** | **Tracks** |
| Encoder click | Launch scene; play (Arrangement view) | Launch scene | Show/close plugin; select (Browser) | **Show/close plugin** | Preset / track selection | **Show/close plugin** |
| Click + turn | – | **Device parameter pages** | – | – | – | – |
| Shift + turn | **Tracks** | **Tracks** | Next/previous preset; fast forward / rewind | **Fast forward / rewind** | **Timeline** | **Fast forward / rewind** |
| Shift + click | **Arm track** | Preset browser (Shift + long press: insert a device) | Channel Rack / Browser | Show/close library | – | Device mode / Mixer mode |
| Knobs 1–8 | Focused plugin | Focused device's remote controls, paged | Focused plugin | Focused plugin | Focused device, or mixer channel | Focused plugin, or **8 track volumes** (Mixer mode) |
| Faders 1–4 | Volume, send A, send B, pan | Volume, send 1, send 2, pan | Plugin parameters; else 1 volume, 4 pan | Volume, sends, pan | Plugin, or volume/sends/pan (mixer) | Volume, send 1, send 2, pan, with "scaled" takeover |
| Pads bank A | Clip launch | Clip launch | Drum notes (FPC layout) | Drum notes | Drum notes | Drum notes |
| Pads bank B | Drum Rack notes | Drum notes | Drum notes (FPC layout) | Drum notes | Drum notes | Drum notes |
| Shift + pads 4–8 | Loop, stop, play/pause, record, tap tempo | Same | Same | Same | Same | Same |

"Scaled" takeover (Cubase): after the value changed elsewhere, a fader moves it in proportion until fader and
value meet, so the value doesn't jump.

## Screen and lights

- The screen shows names: track, scene, parameter name and value, the timeline position.
- Live shows the arm icon.
- Transport pads (Shift + pads 4–8) are lit full when on and dim when off. The guides say 33%; Ableton's script
  uses 20 of 127, about 16%.
- Play and record **blink** while playing or recording, in all but Cubase.
- Ableton's script shows short messages for about 1 s: "Live 12 / Connected" at start, "Tap Tempo / 120 BPM",
  "Loop Mode / ON", and the time while moving through the arrangement. Reason's script shows "Loop Mode" too.

## Pads in detail

- **Notes:** in the DAW program both banks send notes on channel 10: bank A 36–43, bank B 44–51.
- **Logic, Cubase, Reason, FL Studio:** every pad plays its note into the DAW. Nothing else. FL Studio remaps the
  notes to the layout of FPC (FL's drum pad plugin). Reason maps the drum volumes of Kong and Rytmic to the knobs.
- **Pad lights in those four:** white or grey while pressed, dim (`14 14 14`) when released. Logic's code for it is
  commented out ("PAD LED (not working)"), so in Logic the pads keep the device's own colours.
- **Cubase** has a commented-out bank B with mute, solo, editor and window-zone toggles: tried and dropped.
- **Bitwig 1.1:** bank A is clip mode. The pads launch the clips of the selected scene, in the clip colours; a long
  press stops a clip; playing clips are bright, recording clips blink red. Bank B is device mode: drum notes from
  C1, the encoder shifts the note range in steps of four, the pads take the drum instruments' colours (or the
  track colour), and a pad lights up when a clip plays its note.
- **Live:** bank A launches clips, bank B plays the Drum Rack with each pad in its sound's colour.

No integration uses the pads for track selection. The linear DAWs leave both banks to drum notes.

## What reaches the tracks

Each integration decides which MiniLab messages reach the tracks, so knob, fader and encoder moves don't end up in
recordings:

- **Bitwig:** the extension opens note inputs with filters. Keys, aftertouch, pitch bend and a few other messages
  pass; the pads (channel 10) get their own input that can remap or block notes. The controls are in no filter.
- **FL Studio:** the script marks each message as handled or not. It passes the pad notes on purpose and keeps the
  controls.
- **Reason:** the script sorts every message. Only notes ("Keyboard") go to the instrument; knobs and faders become
  controls.
- **Live, Cubase, Logic:** their control surface frameworks own the input too. What they pass was not checked in
  the code.

Knob moves are recorded as automation in all of them, not as MIDI.

## Two families

- **Clip-launch DAWs (Live, Bitwig):** the encoder walks scenes and launches them, pads bank A launch clips,
  Shift + encoder picks tracks.
- **Linear DAWs (Logic, Cubase, Reason; FL Studio partly):** the encoder walks **tracks**, the click opens the
  plugin, Shift + encoder moves the **playhead**, and both pad banks play drum notes.

Reaper is a linear DAW: it has no clips or scenes, unless the Helgobox Playtime add-on is installed. The extension
follows the linear family; `docs/daw-comparison.md` shows where it differs.
