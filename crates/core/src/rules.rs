//! The rules: Reaper state -> what the device shows, device input -> what Reaper does.
//!
//! Pure functions over plain data, so they are tested without Reaper. The extension fills [`State`], pushes the
//! output of [`render`] into the [`OutQueue`](crate::queue::OutQueue), and runs the [`Command`]s from [`command`].

use crate::protocol::{self, led_id, Event, Icons, PadBank, Program, Rgb, TransportButton, TransportIcon};
use crate::queue::Key;

/// A marker on a pad of bank B.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Marker {
    /// 8-bit colour from Reaper, `None` when the marker has no custom colour.
    pub color: Option<(u8, u8, u8)>,
    /// The playhead is at or after this marker, before the next one.
    pub current: bool,
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

/// What a fader controls: 1 volume, 2 send 1, 3 send 2, 4 pan of the selected track.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FaderTarget {
    pub name: String,
    pub value: String,
    /// Where the value is, as a fader position 0..=127 (for pickup).
    pub position: u8,
    /// `Some(up)` while the fader has not caught the value yet: which way to move it.
    pub waiting: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Transport {
    pub playing: bool,
    pub recording: bool,
    pub repeat: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    pub track_count: usize,
    /// 0-based index of the selected track.
    pub selected: Option<usize>,
    pub selected_name: String,
    /// 8-bit colour of the selected track, `None` when it has no custom colour.
    pub selected_color: Option<(u8, u8, u8)>,
    /// Linear volume of the selected track (1.0 = 0 dB).
    pub selected_volume: f64,
    pub selected_armed: bool,
    pub transport: Transport,
    /// Bank A pads held down right now (drum notes).
    pub pads_held: [bool; 8],
    /// Bank B: markers 1-8; `None` where the project has no marker with that number.
    pub markers: [Option<Marker>; 8],
    /// Knobs 1-8; `None` where no parameter is mapped.
    pub knobs: [Option<Param>; 8],
    /// Faders 1-4; `None` where there is nothing to control (no such send).
    pub faders: [Option<FaderTarget>; 4],
    /// A short message that replaces the screen for a moment ("Tap Tempo", "120 BPM").
    pub notification: Option<(String, String)>,
    /// Blink phase: play and record LEDs alternate full and dim while running.
    pub blink: bool,
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

/// Pan -1.0 (left) ..= 1.0 (right) as text: "Center", "25% L", "100% R".
pub fn pan_text(pan: f64) -> String {
    let pct = (pan.abs() * 100.0).round();
    if pct < 1.0 {
        "Center".into()
    } else {
        format!("{pct:.0}% {}", if pan < 0.0 { "L" } else { "R" })
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
    let screen = match (&state.notification, state.selected) {
        (Some((line1, line2)), _) => protocol::screen(line1, line2, Icons::default()),
        (None, Some(i)) => protocol::screen(
            &state.selected_name,
            &format!("{}/{}  {}", i + 1, state.track_count, volume_text(state.selected_volume)),
            icons,
        ),
        (None, None) if state.track_count == 0 => protocol::screen("No tracks", "", icons),
        (None, None) => protocol::screen("No track", "selected", icons),
    };
    out.push((Key::Screen, screen));

    let rgb = |color: Option<(u8, u8, u8)>| color.map(|(r, g, b)| Rgb::from_8bit(r, g, b)).unwrap_or(UNCOLORED);
    // Bank A: drum pads in the selected track's colour, bright while held.
    let drums = rgb(state.selected_color);
    for (pad, held) in state.pads_held.iter().enumerate() {
        let id = led_id::PADS_A + pad as u8;
        out.push((Key::Led(id), protocol::led(id, if *held { drums } else { drums.scaled(DIM) })));
    }
    // Bank B: markers in their colours, the one the playhead is in bright. The device keeps these while bank A is
    // up.
    for (pad, marker) in state.markers.iter().enumerate() {
        let color = match marker {
            Some(m) if m.current => rgb(m.color),
            Some(m) => rgb(m.color).scaled(DIM),
            None => Rgb::OFF,
        };
        let id = led_id::PADS_B + pad as u8;
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

    // Faders: the device shows the popup while a fader moves. Before pickup, the value says which way to go.
    for (i, target) in state.faders.iter().enumerate() {
        let cc = protocol::DAW_FADER_CCS[i];
        match target {
            Some(f) => {
                let value = match f.waiting {
                    Some(true) => format!("{} (up)", f.value),
                    Some(false) => format!("{} (down)", f.value),
                    None => f.value.clone(),
                };
                out.push((Key::Parameter(cc), protocol::parameter(cc, &f.name, &value)));
            }
            None => out.push((Key::Parameter(cc), protocol::blank_parameter(cc))),
        }
    }

    let t = &state.transport;
    let transport_leds = [
        (TransportButton::Loop, t.repeat, Rgb { r: 127, g: 50, b: 0 }),
        (TransportButton::Stop, !t.playing && !t.recording, Rgb { r: 127, g: 127, b: 127 }),
        // Play and record blink while running, as in Arturia's integrations.
        (TransportButton::Play, t.playing && state.blink, Rgb { r: 0, g: 127, b: 0 }),
        (TransportButton::Record, t.recording && state.blink, Rgb { r: 127, g: 0, b: 0 }),
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

/// The bar (0-based) the playhead goes to after `step` encoder steps from inside `bar`. Going back from the middle
/// of a bar first goes to its start, as a rewind button does.
pub fn bar_step(bar: i32, at_bar_start: bool, step: i32) -> i32 {
    let target = if step < 0 && !at_bar_start { bar + step + 1 } else { bar + step };
    target.max(0)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Select the track this many places up (-) or down (+), clamped to the track list.
    SelectTrackBy(i32),
    /// Bank A pad pressed (`true`) or released: a drum note. Reaper gets the note itself; this is for the lights.
    DrumPad(u8, bool),
    /// Bank B pad: jump to the marker with this number (1-8).
    GoToMarker(u8),
    ToggleArm,
    Transport(TransportButton),
    /// Fader index 0-3 moved to this position 0..=127. Pickup decides whether it applies.
    MoveFader(u8, u8),
    /// Knob index 0-7 and its position 0..=127, for the mapped parameter.
    SetKnob(u8, u8),
    /// Encoder clicked and turned: the knobs move this many pages of 8 parameters.
    KnobPage(i32),
    /// Encoder clicked and released without turning: show or close the selected track's FX.
    ToggleFx,
    /// Shift + encoder: move the playhead this many bars (see [`bar_step`]).
    MovePlayhead(i32),
    /// The device switched program: connect again and redraw everything.
    Redraw,
    /// The device switched pad bank: send everything again, without connecting (connect switches it back to
    /// bank A).
    Repaint,
}

/// What a device event means for Reaper. `None` for events Reaper should simply receive as MIDI (keys, ...).
pub fn command(event: &Event) -> Option<Command> {
    match *event {
        Event::Encoder { delta, shifted: false } => Some(Command::SelectTrackBy(delta as i32)),
        Event::Encoder { delta, shifted: true } => Some(Command::MovePlayhead(delta as i32)),
        Event::EncoderClick { pressed: true, shifted: true } => Some(Command::ToggleArm),
        Event::Transport { button, pressed: true } => Some(Command::Transport(button)),
        Event::Fader { index, value, program: Program::Daw } => Some(Command::MoveFader(index, value)),
        Event::Knob { index, value, program: Program::Daw } => Some(Command::SetKnob(index, value)),
        Event::Pad { index, bank: PadBank::A, on, .. } => Some(Command::DrumPad(index, on)),
        Event::Pad { index, bank: PadBank::B, on: true, .. } => Some(Command::GoToMarker(index + 1)),
        _ => None,
    }
}

/// Device input -> commands, for inputs that depend on what came before: the encoder held down and turned
/// ("click + turn", parameter pages) is not a click.
#[derive(Debug, Default)]
pub struct Controls {
    encoder_held: bool,
    turned_while_held: bool,
}

impl Controls {
    pub fn handle(&mut self, event: &Event) -> Option<Command> {
        match *event {
            Event::EncoderClick { pressed: true, shifted: false } => {
                self.encoder_held = true;
                self.turned_while_held = false;
                None
            }
            Event::EncoderClick { pressed: false, shifted: false } => {
                let was_held = std::mem::take(&mut self.encoder_held);
                (was_held && !self.turned_while_held).then_some(Command::ToggleFx)
            }
            Event::Encoder { delta, shifted: false } if self.encoder_held => {
                self.turned_while_held = true;
                Some(Command::KnobPage(delta as i32))
            }
            _ => command(event),
        }
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
    /// A command when a known value changed: [`Command::Redraw`] for the program, [`Command::Repaint`] for the
    /// pad bank.
    pub fn update(&mut self, event: &Event) -> Option<Command> {
        fn changed<T: PartialEq + Copy>(slot: &mut Option<T>, new: T) -> bool {
            let was = slot.replace(new);
            matches!(was, Some(old) if old != new)
        }
        match *event {
            Event::Program(p) => changed(&mut self.program, p).then_some(Command::Redraw),
            Event::PadBank(b) => changed(&mut self.pad_bank, b).then_some(Command::Repaint),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> State {
        State {
            track_count: 2,
            selected: Some(0),
            selected_name: "Lead".into(),
            selected_color: Some((220, 60, 60)),
            selected_volume: 1.0,
            selected_armed: true,
            transport: Transport { playing: true, ..Default::default() },
            pads_held: [false; 8],
            markers: [const { None }; 8],
            knobs: [const { None }; 8],
            faders: [const { None }; 4],
            notification: None,
            blink: true,
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
    fn drum_pads_in_track_colour_bright_while_held() {
        let mut s = state();
        s.pads_held[1] = true;
        let out = render(&s);
        let lead = Rgb { r: 110, g: 30, b: 30 };
        assert_eq!(find(&out, Key::Led(0x34)), protocol::led(0x34, lead.scaled(DIM)));
        assert_eq!(find(&out, Key::Led(0x35)), protocol::led(0x35, lead));
        s.selected_color = None;
        assert_eq!(find(&render(&s), Key::Led(0x34)), protocol::led(0x34, UNCOLORED.scaled(DIM)));
    }

    #[test]
    fn marker_pads_in_marker_colours_current_bright() {
        let mut s = state();
        s.markers[0] = Some(Marker { color: None, current: false });
        s.markers[2] = Some(Marker { color: Some((0, 220, 0)), current: true });
        let out = render(&s);
        assert_eq!(find(&out, Key::Led(0x44)), protocol::led(0x44, UNCOLORED.scaled(DIM)));
        assert_eq!(find(&out, Key::Led(0x45)), protocol::led(0x45, Rgb::OFF));
        assert_eq!(find(&out, Key::Led(0x46)), protocol::led(0x46, Rgb::from_8bit(0, 220, 0)));
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
    fn fader_popups_and_pickup_hint() {
        let mut s = state();
        s.faders[0] = Some(FaderTarget { name: "Volume".into(), value: "-4.1 dB".into(), position: 90, waiting: None });
        s.faders[3] = Some(FaderTarget { name: "Pan".into(), value: "Center".into(), position: 64, waiting: Some(true) });
        let out = render(&s);
        assert_eq!(find(&out, Key::Parameter(14)), protocol::parameter(14, "Volume", "-4.1 dB"));
        assert_eq!(find(&out, Key::Parameter(15)), protocol::blank_parameter(15));
        assert_eq!(find(&out, Key::Parameter(31)), protocol::parameter(31, "Pan", "Center (up)"));
        let fader = Event::Fader { index: 2, value: 10, program: Program::Daw };
        assert_eq!(command(&fader), Some(Command::MoveFader(2, 10)));
    }

    #[test]
    fn notification_replaces_the_screen() {
        let mut s = state();
        s.notification = Some(("Tap Tempo".into(), "120 BPM".into()));
        let out = render(&s);
        assert_eq!(find(&out, Key::Screen), protocol::screen("Tap Tempo", "120 BPM", Icons::default()));
    }

    #[test]
    fn play_led_blinks_while_playing() {
        let mut s = state();
        let full = protocol::led(0x59, Rgb { r: 0, g: 127, b: 0 });
        assert_eq!(find(&render(&s), Key::Led(0x59)), full);
        s.blink = false;
        assert_ne!(find(&render(&s), Key::Led(0x59)), full);
    }

    #[test]
    fn empty_project() {
        let out = render(&State::default());
        assert_eq!(find(&out, Key::Screen), protocol::screen("No tracks", "", Icons::default()));
    }

    #[test]
    fn pan_text_formats() {
        assert_eq!(pan_text(0.0), "Center");
        assert_eq!(pan_text(0.004), "Center");
        assert_eq!(pan_text(-0.25), "25% L");
        assert_eq!(pan_text(1.0), "100% R");
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
        assert_eq!(command(&Event::Encoder { delta: -1, shifted: false }), Some(Command::SelectTrackBy(-1)));
        assert_eq!(command(&Event::Encoder { delta: 2, shifted: true }), Some(Command::MovePlayhead(2)));
        assert_eq!(command(&Event::EncoderClick { pressed: true, shifted: true }), Some(Command::ToggleArm));
        assert_eq!(
            command(&Event::Transport { button: TransportButton::Play, pressed: true }),
            Some(Command::Transport(TransportButton::Play))
        );
        assert_eq!(command(&Event::Transport { button: TransportButton::Play, pressed: false }), None);
        let pad = |bank, on| Event::Pad { index: 2, bank, velocity: 90, on };
        assert_eq!(command(&pad(PadBank::A, true)), Some(Command::DrumPad(2, true)));
        assert_eq!(command(&pad(PadBank::A, false)), Some(Command::DrumPad(2, false)));
        assert_eq!(command(&pad(PadBank::B, true)), Some(Command::GoToMarker(3)));
        assert_eq!(command(&pad(PadBank::B, false)), None);
    }

    #[test]
    fn click_and_turn_pages_click_alone_clicks() {
        let mut c = Controls::default();
        let click = |pressed| Event::EncoderClick { pressed, shifted: false };
        let turn = |delta| Event::Encoder { delta, shifted: false };
        assert_eq!(c.handle(&click(true)), None);
        assert_eq!(c.handle(&turn(1)), Some(Command::KnobPage(1)));
        assert_eq!(c.handle(&turn(1)), Some(Command::KnobPage(1)));
        assert_eq!(c.handle(&click(false)), None); // it was a page turn, not a click
        assert_eq!(c.handle(&click(true)), None);
        assert_eq!(c.handle(&click(false)), Some(Command::ToggleFx));
        assert_eq!(c.handle(&turn(-1)), Some(Command::SelectTrackBy(-1))); // released: turns pick tracks again
        let shifted = Event::Encoder { delta: 1, shifted: true };
        assert_eq!(c.handle(&shifted), Some(Command::MovePlayhead(1)));
    }

    #[test]
    fn playhead_moves_by_bars() {
        assert_eq!(bar_step(4, true, 1), 5);
        assert_eq!(bar_step(4, false, 1), 5);
        assert_eq!(bar_step(4, true, -1), 3);
        assert_eq!(bar_step(4, false, -1), 4); // mid-bar: back to its start first
        assert_eq!(bar_step(4, false, -2), 3);
        assert_eq!(bar_step(0, true, -3), 0);
    }

    #[test]
    fn device_status_redraws_only_on_real_change() {
        let mut s = DeviceStatus::default();
        assert_eq!(s.update(&Event::PadBank(PadBank::A)), None); // first report (reply to connect)
        assert_eq!(s.update(&Event::PadBank(PadBank::A)), None); // repeat
        assert_eq!(s.update(&Event::PadBank(PadBank::B)), Some(Command::Repaint));
        assert_eq!(s.update(&Event::Program(Program::Daw)), None);
        assert_eq!(s.update(&Event::Program(Program::Arturia)), Some(Command::Redraw));
    }
}
