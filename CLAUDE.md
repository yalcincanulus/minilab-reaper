# MiniLab 3 for Reaper

Native Reaper support for the Arturia MiniLab 3 (screen, pad lights, navigation, plugin parameters): a Rust
extension, with Lua ReaScripts for experiments. Linux (CachyOS) first. An experiment: no public solution exists, and
it may not work out.

## Where things are

- **Plan and decisions:** `.scratch/roadmap.md`. Read it at the start of a session; tick items and log decisions
  with dates as they happen.
- **Device protocol:** `docs/minilab3-protocol.md`. Every message is marked verified (seen on hardware) or script
  (from Ableton's code). Update it whenever the hardware shows something new.
- **Background:** `.scratch/reaper-research.md` (existing tools, options, Reaper API notes) and
  `.scratch/lessons-from-synth-app.md` (device quirks from the earlier browser synth). The TypeScript code to port
  is in `.scratch/reaper-handoff/reference/`.
- `.scratch/` is git-ignored: working notes, shots, handoff material.

## The user

New to DAWs: has followed a few Reaper tutorials, no real projects. Explain Reaper terms briefly. For workflow
choices (what pads bank A or the encoder do), propose a default they can try in Reaper.

The MiniLab and the phone sit on the floor, so a physical action costs the user a trip. Do everything possible over
MIDI and adb first, and batch the hands-on steps into one request. A guided run that shows the instructions on the
MiniLab screen and detects each action over MIDI works well: the user can't see the terminal from there.

## Eyes on the device

- `tools/phone-shot.sh`: Galaxy M31 over adb, camera app in Pro mode. Reads the screen text and sees Shift, Hold,
  Oct and knob 1. `--ui` saves the phone's own screen when a shot fails.
- `tools/cam-shot.sh`: webcam, side view of the pads, faders and knobs. Pad colours, no text.
- `tools/ml3.py`: send connect, LEDs, screen, raw SysEx; listen to the device. While Reaper runs, PipeWire's MIDI
  bridge holds the raw port, and ml3.py falls back to the ALSA sequencer (`aseqsend`, `aseqdump`) by itself.
- Check output on the cameras yourself rather than asking the user. Take shots with hands clear of the device: a
  hand on knob 1 covers half the screen.

## Device rules

- DAW program (Shift + Pad 3 toggles it) and the connect message come before LED and screen messages.
- Pace outgoing SysEx about 2 ms apart through a keyed queue (a newer message replaces a pending one with the same
  key). Bursts with no gap lose messages.
- The device keeps Shift + pads 1–3 (Arp, Pad bank, Prog) for itself. Shift + pads 4–8 send the transport CCs
  105–109 (loop, stop, play, record, tap); the value is the press strength.

## Driving Reaper

- `tools/reaper.sh install | start | run script.lua | shot | quit [--discard]`. `install` builds the extension and
  links it into `~/.config/REAPER/UserPlugins`; Reaper can't unload extensions, so restart it after each build.
  The extension logs to `~/.config/REAPER/minilab3.log`.
  `run` executes a ReaScript inside the running Reaper; scripts report by writing a file. `shot` captures only Reaper's window (the user has two screens; their
  desktop stays private).
- Reaper's MIDI goes through PipeWire's JACK bridge: ports are named `Midi-Bridge:Minilab3: MIDI ...`. MIDI input
  device 1 is the MiniLab (`midiins=2` in `reaper.ini`). Its output is off in Reaper's preferences (`midiouts=0`):
  the extension opens it itself (`CreateMIDIOutput`), so its disconnect at exit gets out. The Lua spike sends
  through Reaper's outputs, so it needs `midiouts=2` again.
- `tools/fake-input.sh link | send <hex> | unlink`: fake MiniLab input. Reaper sees messages sent to Midi Through
  as coming from the MiniLab, so input features can be tested without the user.
- `reaper.ini` changes only stick while Reaper is closed: Reaper rewrites it on exit.
- `reaper/spike/minilab_spike.lua`: the Lua spike (stop it with `reaper/spike/stop.lua`). Log: `.scratch/spike.log`.
- `reaper/test.RPP`: the test project (Lead, Bass, Drums, Keys, Vocals; Reaper's own plugins; synth tracks take
  MiniLab input).

## Code

- Logic lives in the `core` crate, tested with `cargo test` without Reaper or the device. The Reaper layer stays
  thin.
- Develop against the system Reaper (7.83, from pacman). `~/.config/REAPER` is backed up before the first change.

## Writing

- Docs and UI text: plain, short sentences.
- Commits: conventional commits, lowercase: `feat(scope): message`, `docs(protocol): ...`, `chore(tools): ...`.
