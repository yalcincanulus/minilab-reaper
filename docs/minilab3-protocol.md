# MiniLab 3 MIDI protocol

Sources: captured from the device on 2026-10-23 (`reference/capture-arturia-mode.log`), Ableton Live 12's
`MiniLab_3` remote script (decompiled: github.com/gluon/AbletonLive12_MIDIRemoteScripts), and tests on the device
with `tools/ml3.py` and the cameras (2026-10-29). Started in the earlier `arturia-minilab-exploration` repo (a
browser synth using Web MIDI).

Status: **verified** = seen on hardware. **script** = from Ableton's script, not yet seen on hardware.

## Ports (Linux ALSA names)

| Port | Use |
|---|---|
| `Minilab3 MIDI` | Everything: controls in, SysEx out. Our code uses only this port. |
| `Minilab3 DIN THRU` | MIDI out to the 5-pin DIN jack |
| `Minilab3 MCU/HUI` | Mackie Control: Shift+transport pads also send notes 86/93/94/95 here |
| `Minilab3 ALV` | Analog Lab V integration |

Identity reply: `F0 7E 7F 06 02 00 20 6B 02 00 04 04 45 00 02 01 F7` (verified).

## Inbound

Channels are 1-based here.

| Control | Arturia program (1) | DAW program (2) | Status |
|---|---|---|---|
| Knobs 1–8 | CC 74 71 76 77 93 18 19 16 | CC 86 87 89 90 110 111 116 117 | verified / verified |
| Faders 1–4 | CC 82 83 85 17 | CC 14 15 30 31 | verified / verified |
| Shift | CC 9 | CC 27 | verified / verified |
| Encoder turn | CC 114 relative | CC 28 (Shift: CC 29) | verified / verified |
| Encoder click | CC 115 | CC 118 (Shift: CC 119) | verified / verified |
| Keys | Notes on the keyboard channel (default ch 1), 48–72 at Oct 0 | same | verified |
| Pads bank A | Notes 36–43, ch 10, velocity + poly aftertouch | same | verified |
| Pads bank B | Notes 44–51, ch 10 | same | verified |
| Pitch strip | Pitch bend | same | verified |
| Mod strip | CC 1 | same | verified |
| Shift + loop/stop/play/rec/tap | CC 105–109, from Shift + pads 4–8; value = press strength (a light tap sent 37), 0 on release | same | verified 2026-10-29 |
| Shift + pads 1–3 | Nothing: Arp on/off, pad bank A/B (status SysEx), program Arturia/DAW (status SysEx) | same | verified 2026-10-29 |

- The two programs' CC numbers do not overlap, so one decoder handles both.
- Relative encoder: value > 64 = clockwise, < 64 = counter-clockwise. A 64 "rest" value comes between steps; ignore it.
- Keys send no aftertouch. Pads send polyphonic aftertouch (per-pad pressure).
- Hold, Chord, Oct −/+ and Arp are handled inside the device. They send no messages; you see only their effect on notes.

Status SysEx from the device (`F0 00 20 6B 7F 42 …` header):

| Body | Meaning |
|---|---|
| `02 00 40 62 01/02` | Program changed: 1 = Arturia, 2 = DAW. After a switch back to DAW the device also sends the pad bank status (verified) |
| `02 00 40 63 00/01` | Pad bank changed: 0 = A, 1 = B |

## Outbound (header `F0 00 20 6B 7F 42`)

| Message | Body | Status |
|---|---|---|
| Connect (DAW hello) | `02 00 40 6A 21` | verified: required before LED/display messages take effect. Verified 2026-10-30: it also switches the pads back to bank A (the device then reports bank A), so don't send it on a bank change |
| Disconnect | `02 00 40 6A 20` | verified 2026-10-29: LEDs go back to the device defaults. After it, LED messages still work but screen messages don't (the device shows its own "DAW" page) |
| Request program | `01 00 40 01` | verified 2026-10-29: reply `02 00 40 01 <p>`, p = 1 in the DAW program, 0 in the Arturia program (verified both) |
| LED colour | `02 02 16 <id> <r> <g> <b>` (0–127 each) | verified |
| Screen, persistent | `04 02 60 1F 07 01 <icon> <arm> <arp> 00 01 <line1> 00 02 <line2> 00` | verified. Line lengths: see below |
| Screen, parameter popup | `04 02 60 1F 08 02 <cc> 00 01 <name> 00 02 <value> 00` | verified 2026-10-29: the device stores name and value per control and shows them only while that control moves: a round knob graphic, the name and the value (e.g. "Attack / 4685.0"). Sent alone it shows nothing. Send it whenever the mapped parameter changes |
| Knob value realign | `21 10 00 <7..14> 00 <value>` | verified 2026-10-29: realign knob 1 to 100, the next turn sent 101; to 10, it sent 11 |
| Blank parameter | `04 02 60 1F 09 02 <cc> 00` | script (Ableton: control not mapped) |

Icon bytes: 0 none, 1 arp, 2 play, 3 record, 4 arm. Verified 2026-10-29: play (triangle) and record (filled dot) show
left of line 1, arm (circled symbol) left of line 2. With arp = 1, the arp icon (rising dots) shows at the right of
line 2 while the device's Arp is on (Shift + Pad 1); with Arp off nothing shows (verified).

**Screen text** (verified 2026-10-29): lowercase, digits and ASCII symbols all show. The font is proportional and
the lines are cut by pixel width, not a character count: line 1 fit 14 capitals (`ABCDEFGHIJKLMN`) but only 10
`W`; line 2 fit 18 digits, or 18 `i` + `ABCDEF`. Ableton's 10/18 limits are a safe minimum. Text is centred.

LED IDs (scan of `00–7F` on 2026-10-29):

| ID | LED | Status |
|---|---|---|
| `0x04–0x0B` | Pads 1–8 of bank A. Same LEDs as `0x34–0x3B`: the last message wins. Nothing shows in bank B | verified |
| `0x30` | Shift | verified |
| `0x31` | Oct − | verified |
| `0x32` | Hold | verified |
| `0x33` | Oct + | verified |
| `0x34–0x3B` | Pads bank A | verified |
| `0x44–0x4B` | Pads bank B (not shown while bank A is up) | verified |
| `0x57–0x5B` | Shift layer, pads 4–8 (printed labels): loop, stop, play, record, tap. Shown only while Shift is held; pads 1–3 (Arp, Pad, Prog) keep the device's colours | verified 2026-10-29 |

No other ID in `00–7F` lit anything visible. Ableton's transport colours: play green, stop white, loop amber,
record red, tap white; "on" = 127, "off" = 20 (about 16%).

**Pacing** (measured 2026-10-29, raw ALSA writes to `/dev/snd/midiC5D0`): 12 or 50 LED messages in one write with
no gap all landed right. 200 random LED messages + a screen message + 12 final LEDs with no gap: messages were lost
(wrong final colours, screen not updated), twice. The same 200 with a 1, 3 or 5 ms gap: all correct. So the device
has an input buffer of roughly 0.6–2.4 KB. Use a ~2 ms gap and the keyed queue (a newer message replaces a pending one
with the same key). The browser app's problem with back-to-back messages likely came from the Web MIDI path.
