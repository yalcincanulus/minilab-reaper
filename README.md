# MiniLab 3 for Reaper

An experiment: give the Arturia MiniLab 3 the kind of Reaper support Arturia ships for Ableton and Bitwig. Screen
text, pad lights, track navigation, plugin parameters on the knobs, transport on Shift + pads.

Status: early. The device protocol is mostly mapped (`docs/minilab3-protocol.md`); no Reaper code yet.

- `docs/minilab3-protocol.md`: the MiniLab 3 MIDI and SysEx protocol, verified on hardware where marked.
- `tools/ml3.py`: talk to the device from the shell (needs `amidi` from alsa-utils).
- `tools/cam-shot.sh`, `tools/phone-shot.sh`: camera shots of the device, for checking its screen and lights.
