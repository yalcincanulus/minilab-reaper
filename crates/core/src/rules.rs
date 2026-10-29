//! The rules: Reaper state -> what the device shows, device input -> what Reaper does.
//!
//! Pure functions over plain data, so they are tested without Reaper. The extension fills [`State`], pushes the
//! output of [`render`] into the [`OutQueue`](crate::queue::OutQueue), and runs the [`Command`]s from [`command`].

use crate::protocol::{self, led_id, Event, Icons, PadBank, Program, Rgb, TransportButton, TransportIcon};
use crate::queue::Key;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Track {
    pub name: String,
    /// 8-bit colour from Reaper, `None` when the track has no custom colour.
    pub color: Option<(u8, u8, u8)>,
}

/// A parameter on a knob: the selected track's first FX, parameters 1-8.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Param {
    pub name: String,
    /// Reaper's formatted value, e.g. "3.0" or "-6.00".
    pub value: String,
    /// 0.0..=1.0
    pub normalized: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Transport {
    pub playing: bool,
    pub recording: bool,
    pub repeat: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    /// The first 8 tracks (one per pad).
    pub tracks: Vec<Track>,
    pub track_count: usize,
    /// 0-based index of the selected track.
    pub selected: Option<usize>,
    pub selected_name: String,
    /// Linear volume of the selected track (1.0 = 0 dB).
    pub selected_volume: f64,
    pub selected_armed: bool,
    pub transport: Transport,
    /// Knobs 1-8; `None` where no parameter is mapped.
    pub knobs: [Option<Param>; 8],
}

const DIM: f32 = 0.15;
/// Ableton's "off" level for transport LEDs: 20 of 127.
const TRANSPORT_OFF: f32 = 0.16;
const UNCOLORED: Rgb = Rgb { r: 64, g: 64, b: 64 };

pub fn volume_text(volume: f64) -> String {
    if volume < 1e-7 {
        "-inf dB".into()
    } else {
        let db = 20.0 * volume.log10();
        // Avoid "-0.0".
        format!("{:.1} dB", if db.abs() < 0.05 { 0.0 } else { db })
    }
}

/// Everything the device should show, as keyed messages. Push all of it every time: the queue drops what is
/// unchanged.
pub fn render(state: &State) -> Vec<(Key, Vec<u8>)> {
    let mut out = Vec::new();

    let icons = Icons {
        transport: if state.transport.recording {
            TransportIcon::Record
        } else if state.transport.playing {
            TransportIcon::Play
        } else {
            TransportIcon::None
        },
        arm: state.selected.is_some() && state.selected_armed,
        arp: false,
    };
    let screen = match state.selected {
        Some(i) => protocol::screen(
            &state.selected_name,
            &format!("{}/{}  {}", i + 1, state.track_count, volume_text(state.selected_volume)),
            icons,
        ),
        None if state.track_count == 0 => protocol::screen("No tracks", "", icons),
        None => protocol::screen("No track", "selected", icons),
    };
    out.push((Key::Screen, screen));

    for pad in 0..8u8 {
        let color = match state.tracks.get(pad as usize) {
            Some(t) => {
                let base = t.color.map(|(r, g, b)| Rgb::from_8bit(r, g, b)).unwrap_or(UNCOLORED);
                if state.selected == Some(pad as usize) {
                    base
                } else {
                    base.scaled(DIM)
                }
            }
            None => Rgb::OFF,
        };
        let id = led_id::PADS_A + pad;
        out.push((Key::Led(id), protocol::led(id, color)));
    }

    // Knobs: the device keeps a value and a popup text per knob. Realign so the next turn continues from the
    // parameter's value; the queue drops both while nothing changed.
    for (i, param) in state.knobs.iter().enumerate() {
        let cc = protocol::DAW_KNOB_CCS[i];
        match param {
            Some(p) => {
                out.push((Key::Knob(i as u8), protocol::knob_value(i as u8, knob_position(p.normalized))));
                out.push((Key::Parameter(cc), protocol::parameter(cc, &p.name, &p.value)));
            }
            None => out.push((Key::Parameter(cc), protocol::blank_parameter(cc))),
        }
    }

    let t = &state.transport;
    let transport_leds = [
        (TransportButton::Loop, t.repeat, Rgb { r: 127, g: 50, b: 0 }),
        (TransportButton::Stop, !t.playing && !t.recording, Rgb { r: 127, g: 127, b: 127 }),
        (TransportButton::Play, t.playing, Rgb { r: 0, g: 127, b: 0 }),
        (TransportButton::Record, t.recording, Rgb { r: 127, g: 0, b: 0 }),
        (TransportButton::Tap, false, Rgb { r: 127, g: 127, b: 127 }),
    ];
    for (i, (_, on, color)) in transport_leds.into_iter().enumerate() {
        let id = led_id::TRANSPORT + i as u8;
        out.push((Key::Led(id), protocol::led(id, if on { color } else { color.scaled(TRANSPORT_OFF) })));
    }
    out
}

pub fn knob_position(normalized: f64) -> u8 {
    (normalized.clamp(0.0, 1.0) * 127.0).round() as u8
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Select the track this many places up (-) or down (+), clamped to the track list.
    SelectTrackBy(i32),
    SelectTrack(usize),
    ToggleArm,
    Transport(TransportButton),
    /// Fader position 0..=127 for the selected track's volume.
    SetVolumeFader(u8),
    /// Knob index 0-7 and its position 0..=127, for the mapped parameter.
    SetKnob(u8, u8),
    /// The device switched program or pad bank: it forgot what we drew.
    Redraw,
}

/// What a device event means for Reaper. `None` for events Reaper should simply receive as MIDI (keys, ...).
pub fn command(event: &Event) -> Option<Command> {
    match *event {
        Event::Encoder { delta, shifted: true } => Some(Command::SelectTrackBy(delta as i32)),
        Event::EncoderClick { pressed: true, shifted: true } => Some(Command::ToggleArm),
        Event::Transport { button, pressed: true } => Some(Command::Transport(button)),
        Event::Fader { index: 0, value, program: Program::Daw } => Some(Command::SetVolumeFader(value)),
        Event::Knob { index, value, program: Program::Daw } => Some(Command::SetKnob(index, value)),
        Event::Pad { index, bank: PadBank::A, on: true, .. } => Some(Command::SelectTrack(index as usize)),
        _ => None,
    }
}

/// Tracks the device's program and pad bank, to tell real changes from repeats. The device answers connect with its
/// pad bank status; redrawing on every status message would loop (seen in the Lua spike).
#[derive(Debug, Default)]
pub struct DeviceStatus {
    pub program: Option<Program>,
    pub pad_bank: Option<PadBank>,
}

impl DeviceStatus {
    /// `Some(Command::Redraw)` when a known value changed.
    pub fn update(&mut self, event: &Event) -> Option<Command> {
        fn changed<T: PartialEq + Copy>(slot: &mut Option<T>, new: T) -> bool {
            let was = slot.replace(new);
            matches!(was, Some(old) if old != new)
        }
        let changed = match *event {
            Event::Program(p) => changed(&mut self.program, p),
            Event::PadBank(b) => changed(&mut self.pad_bank, b),
            _ => false,
        };
        changed.then_some(Command::Redraw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> State {
        State {
            tracks: vec![
                Track { name: "Lead".into(), color: Some((220, 60, 60)) },
                Track { name: "Bass".into(), color: None },
            ],
            track_count: 2,
            selected: Some(0),
            selected_name: "Lead".into(),
            selected_volume: 1.0,
            selected_armed: true,
            transport: Transport { playing: true, ..Default::default() },
            knobs: [const { None }; 8],
        }
    }

    fn find(out: &[(Key, Vec<u8>)], key: Key) -> Vec<u8> {
        out.iter().find(|(k, _)| *k == key).map(|(_, m)| m.clone()).expect("key rendered")
    }

    #[test]
    fn screen_shows_selected_track() {
        let out = render(&state());
        let icons = Icons { transport: TransportIcon::Play, arm: true, arp: false };
        assert_eq!(find(&out, Key::Screen), protocol::screen("Lead", "1/2  0.0 dB", icons));
    }

    #[test]
    fn pads_show_track_colours_selected_bright() {
        let out = render(&state());
        assert_eq!(find(&out, Key::Led(0x34)), protocol::led(0x34, Rgb { r: 110, g: 30, b: 30 }));
        assert_eq!(find(&out, Key::Led(0x35)), protocol::led(0x35, UNCOLORED.scaled(DIM)));
        assert_eq!(find(&out, Key::Led(0x36)), protocol::led(0x36, Rgb::OFF));
    }

    #[test]
    fn transport_leds_follow_play_state() {
        let out = render(&state());
        assert_eq!(find(&out, Key::Led(0x59)), protocol::led(0x59, Rgb { r: 0, g: 127, b: 0 }));
        let stop_dim = Rgb { r: 127, g: 127, b: 127 }.scaled(TRANSPORT_OFF);
        assert_eq!(find(&out, Key::Led(0x58)), protocol::led(0x58, stop_dim));
    }

    #[test]
    fn knobs_realign_and_preload_popups() {
        let mut s = state();
        s.knobs[0] = Some(Param { name: "Attack".into(), value: "3.0".into(), normalized: 0.5 });
        let out = render(&s);
        assert_eq!(find(&out, Key::Knob(0)), protocol::knob_value(0, 64));
        assert_eq!(find(&out, Key::Parameter(86)), protocol::parameter(86, "Attack", "3.0"));
        assert_eq!(find(&out, Key::Parameter(87)), protocol::blank_parameter(87));
        assert!(!out.iter().any(|(k, _)| *k == Key::Knob(1)));
        let knob = Event::Knob { index: 0, value: 100, program: Program::Daw };
        assert_eq!(command(&knob), Some(Command::SetKnob(0, 100)));
    }

    #[test]
    fn empty_project() {
        let out = render(&State::default());
        assert_eq!(find(&out, Key::Screen), protocol::screen("No tracks", "", Icons::default()));
    }

    #[test]
    fn volume_text_formats() {
        assert_eq!(volume_text(1.0), "0.0 dB");
        assert_eq!(volume_text(0.5), "-6.0 dB");
        assert_eq!(volume_text(0.0), "-inf dB");
        assert_eq!(volume_text(0.99999), "0.0 dB");
    }

    #[test]
    fn commands_from_events() {
        assert_eq!(command(&Event::Encoder { delta: -1, shifted: true }), Some(Command::SelectTrackBy(-1)));
        assert_eq!(command(&Event::Encoder { delta: 1, shifted: false }), None);
        assert_eq!(
            command(&Event::Transport { button: TransportButton::Play, pressed: true }),
            Some(Command::Transport(TransportButton::Play))
        );
        assert_eq!(command(&Event::Transport { button: TransportButton::Play, pressed: false }), None);
        let pad = Event::Pad { index: 2, bank: PadBank::A, velocity: 90, on: true };
        assert_eq!(command(&pad), Some(Command::SelectTrack(2)));
    }

    #[test]
    fn device_status_redraws_only_on_real_change() {
        let mut s = DeviceStatus::default();
        assert_eq!(s.update(&Event::PadBank(PadBank::A)), None); // first report (reply to connect)
        assert_eq!(s.update(&Event::PadBank(PadBank::A)), None); // repeat
        assert_eq!(s.update(&Event::PadBank(PadBank::B)), Some(Command::Redraw));
        assert_eq!(s.update(&Event::Program(Program::Daw)), None);
        assert_eq!(s.update(&Event::Program(Program::Arturia)), Some(Command::Redraw));
    }
}
