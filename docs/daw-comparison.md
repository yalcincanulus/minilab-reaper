# MiniLab 3 in each DAW, compared with Reaper

What the MiniLab 3 does in each DAW Arturia supports, side by side with Reaper and this extension. The facts about
the other DAWs come from Arturia's guides and integration scripts; sources and details are in
`docs/daw-integrations.md`.

All integrations need the MiniLab in its DAW program (Shift + Pad 3). In the Arturia program, the MiniLab is a
plain MIDI controller in every DAW.

In the tables:

- **Same:** works as in the column to its left.
- **–:** nothing happens, or the guide and script don't say.
- **Not verified:** likely, but not seen in the script code.

## The encoder

| Control | Live | Bitwig 1.1 | FL Studio | Logic Pro | Reason | Cubase | **Reaper (this extension)** |
|---|---|---|---|---|---|---|---|
| Turn | Scenes (Session view); timeline (Arrangement view) | Scenes (bank A); pad note range (bank B) | Channel Rack / Browser items | Tracks | Tracks | Tracks | **Tracks** |
| Click | Launch scene; play (Arrangement view) | Launch scene | Show/close plugin | Show/close plugin | Preset / track selection | Show/close plugin | **Show/close plugin**; a track with no plugin opens its FX chain, where Reaper offers its plugin list |
| Click + turn | – | Device parameter pages | – | – | – | – | **Plugin parameter pages**; in mixer mode: the next or previous 8 tracks |
| Long press | – | – (Shift + long press: insert a device) | – | – | – | – | **Mixer mode on/off** (500 ms) |
| Shift + turn | Tracks | Tracks | Presets; fast forward / rewind | Fast forward / rewind | Timeline | Fast forward / rewind | **Playhead by bars**; bar and time on the screen |
| Shift + click | Arm track | Preset browser | Channel Rack / Browser | Show/close library | – | Device / Mixer mode | **Arm track** |

## Knobs and faders

| Control | Live | Bitwig 1.1 | FL Studio | Logic Pro | Reason | Cubase | **Reaper (this extension)** |
|---|---|---|---|---|---|---|---|
| Knobs 1–8 | Focused plugin | Focused device's remote controls, paged | Focused plugin | Focused plugin | Focused device, or mixer channel | Focused plugin, or 8 track volumes (Mixer mode) | **Focused plugin** (else the track's first), paged; or **8 track volumes** (mixer mode) |
| Knob popup (name and value) | Yes | Yes | Yes | Yes | Yes | Yes | **Yes** |
| Knob realign (no jumps) | Yes | Not verified | Not verified | Not verified | Not verified | Not verified | **Yes** |
| Fader 1 | Volume | Volume | Plugin parameter, else volume | Volume | Plugin, or volume (mixer) | Volume | **Volume** |
| Fader 2 | Send A | Send 1 | Plugin parameter | Send | Plugin, or send | Send 1 | **Send 1** |
| Fader 3 | Send B | Send 2 | Plugin parameter | Send | Plugin, or send | Send 2 | **Send 2** |
| Fader 4 | Pan | Pan | Plugin parameter, else pan | Pan | Plugin, or pan | Pan | **Pan** |
| No jump after a value changed elsewhere | Not verified | Not verified | Not verified | Not verified | Not verified | "Scaled" takeover: the value moves in proportion until fader and value meet | **Pickup**: a fader takes over when it reaches the value; until then the popup says "(up)" or "(down)" |

## Pads

| Control | Live | Bitwig 1.1 | FL Studio | Logic Pro | Reason | Cubase | **Reaper (this extension)** |
|---|---|---|---|---|---|---|---|
| Bank A | Clip launch | Clip launch (a long press stops) | Drum notes (FPC layout) | Drum notes | Drum notes | Drum notes | **Drum notes** |
| Bank A lights | Clip colours | Clip colours; playing clips bright, recording clips blink red | White while pressed, dim after | Device's own colours (Arturia's code for it is commented out) | White while pressed, dim after | Same | **The selected track's colour**: dim, bright while pressed |
| Bank B | Drum Rack notes | Drum notes from C1; the encoder shifts the range | Drum notes (FPC layout) | Drum notes | Drum notes | Drum notes | **Markers 1–8**: a pad jumps to the marker with that number |
| Bank B lights | Each pad in its drum sound's colour | Drum colours; a pad lights when a clip plays its note | White while pressed | Device's own colours | White while pressed | Same | **Marker colours**; the marker the playhead is in is bright; no marker: dark |
| Screen on a pad press | – | – | – | – | – | – | **Drum name**, if the track names the note (bank A); **marker name and bar** (bank B) |
| Shift + pads 4–8 | Loop, stop, play/pause, record, tap tempo | Same | Same | Same | Same | Same | **Same** |

## Transport, screen and lights

| Feature | Live | Bitwig 1.1 | FL Studio | Logic Pro | Reason | Cubase | **Reaper (this extension)** |
|---|---|---|---|---|---|---|---|
| Play = play/pause | Yes | Yes | Yes | Yes | Yes | Yes | **Yes** |
| Transport lights follow the DAW | Yes | Yes | Yes | Yes | Yes | Yes | **Yes** |
| Play and record blink while running | Yes | Yes | Yes | Yes | Yes | No | **Yes** |
| Track name on the screen | Yes | Yes | Yes | Yes | Yes | Yes | **Yes**, with position (3/7) and volume |
| Play, record and arm icons | Arm icon | Not verified | Not verified | Not verified | Not verified | Not verified | **Yes** |
| Short messages | Connected, tap tempo BPM, loop on/off, time | Not verified | Not verified | Not verified | Loop on/off | Not verified | **Tap tempo BPM, loop on/off**, playhead position, plugin pages, mixer mode, markers |
| Record with no track armed | The DAW's usual behaviour | Same | Same | Same | Same | Same | **The screen says "No track armed"** instead of Reaper's warning, which would block Reaper until clicked |

## What reaches the tracks

| Feature | Live | Bitwig 1.1 | FL Studio | Logic Pro | Reason | Cubase | **Reaper (this extension)** |
|---|---|---|---|---|---|---|---|
| Controls kept out of tracks and recordings | Yes (framework; not verified in code) | Yes: filtered note inputs | Yes: the script marks each message handled | Not verified | Yes: the script sorts the messages | Not verified | **Yes**: a hook in Reaper's audio engine removes them before tracks get them, with no added delay |
| Knob moves while recording | Automation | Automation | Automation | Automation | Automation | Automation | **Automation**, with the track in Latch mode |

## Device handling

| Feature | Live | Bitwig 1.1 | FL Studio | Logic Pro | Reason | Cubase | **Reaper (this extension)** |
|---|---|---|---|---|---|---|---|
| Program or bank switch on the device | Redraw | Not verified | Not verified | Not verified | Not verified | Not verified | **Redraw**; a bank switch keeps the bank |
| Unplug and plug back in | The DAW reopens its MIDI ports | Same | Same | Same | Same | Same | **Recovers**: resets Reaper's MIDI devices when the MiniLab is back, then reconnects |
| The DAW quits | The device gets its own screen and colours back | Not verified | Not verified | Not verified | Not verified | Not verified | **Same as Live** |
| Setup | Install the script, choose ports | Automatic | Install the script | Install the script | Install the script | Import the script | **Copy the extension into UserPlugins**; MiniLab input on, its output off in Reaper's MIDI settings |

## Design choices for Reaper

- **Reaper follows the linear DAWs (Logic, Cubase, Reason)** for the encoder: tracks, plugin window, playhead.
  Reaper has no clips or scenes, so the clip-launch layout of Live and Bitwig doesn't fit.
- **Shift + click arms the track**, as in Live. Cubase uses it for mixer mode; here mixer mode is on a long press.
- **Parameter pages (click + turn)** come from Bitwig. Most other integrations reach only the first 8 parameters.
- **Bank B plays no drums; it jumps to markers.** No other integration does this. Markers are the closest thing
  Reaper has to scenes. This choice may change.
- **Not seen in the other scripts:** drum and marker names on the screen, the "No track armed" message, and
  recovery after unplugging. Fader pickup exists in Cubase in another form ("scaled" takeover).

## What other DAWs have and Reaper doesn't

| Feature | Who has it | In Reaper |
|---|---|---|
| Clip launching on the pads | Live, Bitwig | Reaper has no clips (only with the Helgobox Playtime add-on) |
| Drum pads lit in each drum's colour | Live, Bitwig | Reaper has no drum colours; the pads use the track colour |
| Pads light when a recorded part plays their note | Bitwig | Not yet |
| Preset browsing from the encoder | Bitwig, FL Studio, Reason | Not yet |
| Shifting the drum pads' note range | Bitwig | No |

## Not tested

- Windows and macOS.
- Projects with more than 8 tracks: mixer mode groups, pads.
