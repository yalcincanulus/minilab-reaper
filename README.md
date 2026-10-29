# MiniLab 3 for Reaper

An experiment: give the Arturia MiniLab 3 the kind of Reaper support Arturia ships for Ableton and Bitwig. It is a
Reaper extension written in Rust. Linux first (tested on CachyOS with PipeWire).

Status: early, but it works on the device. The MiniLab protocol is mapped and verified on hardware
(`docs/minilab3-protocol.md`).

## What it does

The MiniLab must be in its DAW program (Shift + Pad 3).

| On the MiniLab | In Reaper |
|---|---|
| Screen | Selected track: name, position (2/5), volume; play, record and arm icons |
| Pads 1–8 (bank A) | Lit in the colours of tracks 1–8, the selected track bright. Press: select that track |
| Shift + encoder | Select the previous or next track |
| Shift + encoder click | Arm or disarm the selected track |
| Shift + pads 4–8 | Loop, stop, play, record, tap tempo. Their lights follow Reaper's state while Shift is held |
| Fader 1 | Volume of the selected track |
| Knobs 1–8 | Parameters 1–8 of the selected track's first plugin. Turning a knob shows the name and value on the screen |
| Keys | Play the armed track, as usual |

It also recovers when the MiniLab is unplugged and plugged back in, and gives the MiniLab back its own screen and
colours when Reaper quits.

Not yet: faders 2–4 (sends, pan), pads bank B, the encoder without Shift.

## Build and install

Needs Rust (stable) and Reaper 7.

```sh
tools/reaper.sh install    # cargo build, then link the build into ~/.config/REAPER/UserPlugins
```

Restart Reaper after each build: Reaper loads extensions at start and can't unload them. The extension logs to
`~/.config/REAPER/minilab3.log`.

In Reaper's Preferences → MIDI Devices:

- **Input** `Minilab3 MIDI`: enabled (keys reach tracks, controls reach the extension).
- **Output** `Minilab3 MIDI`: **disabled**. The extension opens it itself.

## Known limits

- Knob and fader moves also reach armed tracks as MIDI, since Reaper and the extension share the input.
- Shift + Record with no armed track opens Reaper's "Record Warning" dialog.
- The recovery after a replug uses `/proc/asound/cards` to wait until the MiniLab is back (Linux only).

## Layout

- `crates/core`: protocol, paced output queue, rules (Reaper state → device output, device input → commands),
  replug watchdog. No Reaper, no device: `cargo test`.
- `crates/reaper-ext`: the Reaper extension (a control surface on [reaper-rs](https://github.com/helgoboss/reaper-rs)).
- `docs/minilab3-protocol.md`: the MiniLab 3 MIDI and SysEx protocol, verified on hardware where marked.
- `reaper/test.RPP`: a small test project. `reaper/spike/`: the Lua prototype.
- `tools/`: `ml3.py` (talk to the device from the shell), `reaper.sh` (build, install, start and script Reaper),
  `fake-input.sh` (fake MiniLab input for Reaper), `cam-shot.sh` and `phone-shot.sh` (camera shots of the device).
