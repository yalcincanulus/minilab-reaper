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
| Encoder click + turn | Page through the plugin's parameters, 8 at a time |
| Shift + pads 4–8 | Loop, play/pause, stop, record, tap tempo. Lights follow Reaper's state while Shift is held; play and record blink while running. Tap tempo and loop show a short message |
| Faders 1–4 | Volume, send 1, send 2, pan of the selected track. A fader takes over once it reaches the current value (pickup); until then the popup says which way to move it |
| Knobs 1–8 | Parameters of the plugin whose window is open on the selected track, else its first plugin. Turning a knob shows the name and value on the screen |
| Keys | Play the armed track, as usual |

It also recovers when the MiniLab is unplugged and plugged back in, and gives the MiniLab back its own screen and
colours when Reaper quits.

Shift + Record with no armed track shows "No track armed" instead of opening Reaper's blocking warning.

Not yet: the encoder without Shift (planned: tracks, click opens the plugin; Shift + encoder moves the playhead),
pads bank B, a mixer mode. `docs/daw-integrations.md` compares what Arturia's integrations do in other DAWs.

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
- Reaper's modal dialogs (e.g. "save or delete recorded files" after a recording) block the extension until
  closed. Turning that prompt off (Preferences → Recording) keeps the MiniLab responsive.
- The recovery after a replug uses `/proc/asound/cards` to wait until the MiniLab is back (Linux only).

## Layout

- `crates/core`: protocol, paced output queue, rules (Reaper state → device output, device input → commands),
  replug watchdog. No Reaper, no device: `cargo test`.
- `crates/reaper-ext`: the Reaper extension (a control surface on [reaper-rs](https://github.com/helgoboss/reaper-rs)).
- `docs/minilab3-protocol.md`: the MiniLab 3 MIDI and SysEx protocol, verified on hardware where marked.
- `reaper/test.RPP`: a small test project. `reaper/spike/`: the Lua prototype.
- `tools/`: `ml3.py` (talk to the device from the shell), `reaper.sh` (build, install, start and script Reaper),
  `fake-input.sh` (fake MiniLab input for Reaper), `cam-shot.sh` and `phone-shot.sh` (camera shots of the device).
