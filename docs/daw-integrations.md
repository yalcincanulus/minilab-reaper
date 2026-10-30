# What Arturia's DAW integrations do

What the MiniLab 3 does in each DAW Arturia supports, to decide what the Reaper extension should match. Sources:
Arturia's integration guides, v1.0 (`https://dl.arturia.net/products/minilab-3/daw-integration-guide/MiniLab_3_<DAW>_Daw_Integration_Guide_1_0_EN.pdf`
for `Live`, `Bitwig_Studio`, `FL_Studio`, `Logic_Pro`, `Reason`, `Cubase`; read 2026-10-29), and Ableton's decompiled
script (`docs/minilab3-protocol.md`). All of them need the DAW program (Shift + Pad 3).

The guides are short and partly copy-pasted between DAWs: Bitwig's "Shift + click: switch between Channel Rack and
Browser" is FL Studio's text, and Live's transport section repeats a sentence about scene colours. Where a guide
and Ableton's script disagree, the script wins (it is what runs).

## By control

| Control | Live | Bitwig | FL Studio | Logic Pro | Reason | Cubase |
|---|---|---|---|---|---|---|
| Encoder turn | Scenes (Session); timeline (Arrangement) | Scenes | Channel Rack / Browser items | **Tracks** | **Tracks** | **Tracks** |
| Encoder click | Launch scene; play (Arrangement) | Launch scene | Show/close plugin; select (Browser) | **Show/close plugin** | Toggle preset / track selection | **Show/close plugin** |
| Click + turn | – | **Plugin parameter pages** | – | – | – | – |
| Shift + turn | **Tracks** | **Tracks** | Next/prev preset (plugin); fast forward / rewind | **Fast forward / rewind** | **Timeline** | **Fast forward / rewind** |
| Shift + click | **Arm track** | (Channel Rack / Browser: likely copied text) | Channel Rack / Browser | Show/close library | – | Device mode / Mixer mode |
| Knobs 1–8 | Focused plugin's parameters | Focused plugin, paged | Focused plugin | Focused plugin | Focused plugin, or mixer channel | Focused plugin, or **8 track volumes** (Mixer mode) |
| Faders 1–4 | Volume, send A, send B, pan | Volume, sends, pan | Plugin parameters; else 1 volume, 4 pan | Volume, sends, pan | Plugin, or volume/sends/pan (mixer) | Volume, pan, sends 1–2 |
| Pads bank A | Clip launch, clip colours | Clip launch, clip colours | Drum notes (FPC layout) | Drum notes | Drum notes | Drum notes |
| Pads bank B | Drum Rack, each pad in its sound's colour | Drum notes, drum colours, light on playback | Drum notes (FPC layout) | Drum notes | Drum notes | Drum notes |
| Shift + pads 4–8 | Loop, play/pause, stop, record, tap | same | same | same | same | same |

Feedback everywhere: the screen shows names (track, scene, parameter name and value, the timeline position); the
arm icon in Live; transport pads lit full when on and dim when off (guides: 33%; Ableton's script: 20 of 127,
about 16%), play and record **blink** while playing or recording (all but Cubase).

Ableton's script also shows short notifications for about 1 s: "Live 12 / Connected" at start, "Tap Tempo /
120 BPM", "Loop Mode / ON", and the time while moving the arrangement position.

## Pads in detail (from Arturia's scripts, read 2026-10-30)

The guides leave the pads out for Logic, Cubase and Reason. The scripts themselves are on Arturia's MiniLab 3
resources page: FL Studio 1.0.1 (Python), Cubase 1.0.1 (MIDI Remote, JavaScript inside a zlib-compressed
`.midiremote`), Reason 1.0 (Lua codec and remote map), Logic 1.0 (`config.lua`), Bitwig 1.1 of 2026-06-26 (a Java
`.bwextension` with Bitwig's own PDF manual inside).

- **Notes:** in the DAW program both banks send notes on channel 10: bank A 36–43, bank B 44–51.
- **Logic, Cubase, Reason, FL Studio:** every pad plays its note into the DAW. Nothing else. FL Studio remaps the
  notes to FPC's (FL's drum pad plugin) layout. Reason maps Kong and Rytmic drum volumes to the knobs.
- **Pad lights in those four:** white or grey while pressed, dim (`14 14 14`) when released. Logic's code for it is
  commented out ("PAD LED (not working)"), so in Logic the pads keep the device's own colours.
- **Cubase** has a commented-out bank B with mute, solo, editor and window-zone toggles: tried and dropped.
- **Bitwig 1.1:** bank A = clip mode (pads launch the clips of the selected scene, in clip colours; a long press
  stops; playing clips bright, recording clips blink red). Bank B = device mode: drum notes from C1, the encoder
  shifts the note range in steps of four, pads in the drum instruments' colours (or the track colour), and pads
  light up when a clip plays their note.

So no integration uses the pads for track selection. The linear DAWs leave both banks to drum notes.

## Two families

- **Clip-launch DAWs (Live, Bitwig):** the encoder walks scenes and launches them, pads bank A launch clips,
  Shift + encoder picks tracks.
- **Linear DAWs (Logic, Cubase, Reason; FL partly):** the encoder walks **tracks**, the click opens the plugin,
  Shift + encoder moves the **playhead**. The guides say nothing about the pads beyond bank switching (probably
  plain notes).

Reaper is a linear DAW (no scenes, unless Helgobox's Playtime is installed), so the second family is the closer
model. Today the extension follows Live for Shift + encoder (tracks) and Shift + click (arm).

## Compared with the Reaper extension (2026-10-29)

| Feature | Others | Reaper extension |
|---|---|---|
| Track navigation with name on screen | all | yes (Shift + encoder, pads) |
| Arm track | Live | yes (Shift + click) |
| Knobs on the focused plugin, name + value popup, realign | all | partly: always the **first** FX, not the focused one |
| Parameter pages | Bitwig | no |
| Faders: volume, sends, pan | all but FL | fader 1 volume only, no pickup |
| Transport on Shift + pads, lights | all | yes; no blinking |
| Play button = play/pause | all | play only |
| Encoder: playhead / timeline, time on screen | Logic, Cubase, Reason, FL, Live (Arrangement) | no |
| Encoder click: show/close plugin | Logic, Cubase, FL | no |
| Mixer mode: knobs on 8 track volumes | Cubase (Reason: mixer lock) | no |
| Drum pads in sound colours (bank B) | Live, Bitwig | no |
| Clip launch (bank A) | Live, Bitwig | no (Reaper has no clips) |
| Preset browsing | FL, Reason | no |
| Notifications (tap tempo BPM, loop on/off, connected) | Live | no |

## Suggested Reaper defaults

Following the linear family, keeping what already works:

- **Encoder turn:** tracks. **Encoder click:** show/close the selected track's FX (focused plugin).
- **Shift + encoder:** move the playhead (by bar), time on the screen. **Shift + click:** arm.
- **Knobs:** the focused FX (fall back to the selected track's first FX), click + turn for parameter pages.
- **Faders:** volume, send 1, send 2, pan, with pickup.
- **Pads:** see "Pads in detail": every linear DAW uses both banks for drum notes.
- **Transport:** play/pause, blinking play/record, notifications for tap tempo and loop.

This changes what Shift + encoder does today (tracks → playhead), so try it with the user before committing to it.
