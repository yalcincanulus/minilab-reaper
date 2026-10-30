//! The MiniLab 3 MIDI protocol: decode what the device sends, build what we send.
//!
//! See `docs/minilab3-protocol.md` for the source of each message and whether it is verified on hardware.

pub const SYSEX_HEADER: [u8; 6] = [0xF0, 0x00, 0x20, 0x6B, 0x7F, 0x42];
const END: u8 = 0xF7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Program {
    Arturia,
    Daw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadBank {
    A,
    B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportButton {
    Loop,
    Stop,
    Play,
    Record,
    Tap,
}

impl TransportButton {
    pub const ALL: [TransportButton; 5] = [Self::Loop, Self::Stop, Self::Play, Self::Record, Self::Tap];
}

// --- Inbound -------------------------------------------------------------------------------------------------

/// CC numbers of one program. The Arturia and DAW programs use disjoint numbers, so both decode at once.
struct CcMap {
    knobs: [u8; 8],
    faders: [u8; 4],
    shift: u8,
    encoder: u8,
    encoder_click: u8,
    shifted_encoder: Option<u8>,
    shifted_encoder_click: Option<u8>,
}

const ARTURIA: CcMap = CcMap {
    knobs: [74, 71, 76, 77, 93, 18, 19, 16],
    faders: [82, 83, 85, 17],
    shift: 9,
    encoder: 114,
    encoder_click: 115,
    shifted_encoder: None,
    shifted_encoder_click: None,
};

const DAW: CcMap = CcMap {
    knobs: [86, 87, 89, 90, 110, 111, 116, 117],
    faders: [14, 15, 30, 31],
    shift: 27,
    encoder: 28,
    encoder_click: 118,
    shifted_encoder: Some(29),
    shifted_encoder_click: Some(119),
};

pub const DAW_KNOB_CCS: [u8; 8] = DAW.knobs;
pub const DAW_FADER_CCS: [u8; 4] = DAW.faders;
pub const MOD_CC: u8 = 1;
/// 0-based, i.e. MIDI channel 10.
pub const PAD_CHANNEL: u8 = 9;
pub const PAD_A_NOTE: u8 = 36;
const PAD_B_NOTE: u8 = 44;
const TRANSPORT_CC_FIRST: u8 = 105; // 105-109: loop, stop, play, record, tap

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Key { note: u8, velocity: u8, on: bool, channel: u8 },
    Pad { index: u8, bank: PadBank, velocity: u8, on: bool },
    PadPressure { index: u8, bank: PadBank, value: u8 },
    Knob { index: u8, value: u8, program: Program },
    Fader { index: u8, value: u8, program: Program },
    /// -1.0..=1.0
    PitchBend(f32),
    Mod(u8),
    /// -1 or +1 per step.
    Encoder { delta: i8, shifted: bool },
    EncoderClick { pressed: bool, shifted: bool },
    Shift { pressed: bool },
    Transport { button: TransportButton, pressed: bool },
    /// Sent when the user switches program, and in reply to [`request_program`].
    Program(Program),
    PadBank(PadBank),
    Identity(Vec<u8>),
    Unknown(Vec<u8>),
}

fn pad_from_note(note: u8) -> Option<(u8, PadBank)> {
    match note {
        n if (PAD_A_NOTE..PAD_A_NOTE + 8).contains(&n) => Some((n - PAD_A_NOTE, PadBank::A)),
        n if (PAD_B_NOTE..PAD_B_NOTE + 8).contains(&n) => Some((n - PAD_B_NOTE, PadBank::B)),
        _ => None,
    }
}

fn decode_cc(cc: u8, value: u8) -> Option<Event> {
    let pressed = value > 0;
    // Relative encoder: 64 is a "rest" value between steps.
    let delta = (value as i16 - 64).signum() as i8;
    for (map, program) in [(&ARTURIA, Program::Arturia), (&DAW, Program::Daw)] {
        if let Some(index) = map.knobs.iter().position(|&c| c == cc) {
            return Some(Event::Knob { index: index as u8, value, program });
        }
        if let Some(index) = map.faders.iter().position(|&c| c == cc) {
            return Some(Event::Fader { index: index as u8, value, program });
        }
        if cc == map.shift {
            return Some(Event::Shift { pressed });
        }
        let encoder = if cc == map.encoder {
            Some(false)
        } else if Some(cc) == map.shifted_encoder {
            Some(true)
        } else {
            None
        };
        if let Some(shifted) = encoder {
            return (delta != 0).then_some(Event::Encoder { delta, shifted });
        }
        if cc == map.encoder_click {
            return Some(Event::EncoderClick { pressed, shifted: false });
        }
        if Some(cc) == map.shifted_encoder_click {
            return Some(Event::EncoderClick { pressed, shifted: true });
        }
    }
    if cc == MOD_CC {
        return Some(Event::Mod(value));
    }
    if (TRANSPORT_CC_FIRST..TRANSPORT_CC_FIRST + 5).contains(&cc) {
        let button = TransportButton::ALL[(cc - TRANSPORT_CC_FIRST) as usize];
        return Some(Event::Transport { button, pressed });
    }
    None
}

/// Whether a message from the device is for the extension only, not for tracks: the DAW program's controls
/// (knobs, faders, encoder, Shift, transport), bank B's pads (markers) and the device's SysEx. Keys, bank A's pads,
/// mod, pitch bend and everything in the Arturia program pass. Runs in the audio thread: no allocation.
pub fn is_daw_control(msg: &[u8]) -> bool {
    let Some(&first) = msg.first() else { return false };
    let d1 = msg.get(1).copied().unwrap_or(0);
    match (first & 0xF0, first & 0x0F) {
        (0xF0, _) => first == 0xF0,
        (0x80 | 0x90 | 0xA0, PAD_CHANNEL) => matches!(pad_from_note(d1), Some((_, PadBank::B))),
        (0xB0, _) => {
            DAW.knobs.contains(&d1)
                || DAW.faders.contains(&d1)
                || [DAW.shift, DAW.encoder, DAW.encoder_click].contains(&d1)
                || DAW.shifted_encoder == Some(d1)
                || DAW.shifted_encoder_click == Some(d1)
                || (TRANSPORT_CC_FIRST..TRANSPORT_CC_FIRST + 5).contains(&d1)
        }
        _ => false,
    }
}

fn decode_sysex(data: &[u8]) -> Event {
    // Universal identity reply: F0 7E <dev> 06 02 ...
    if data.len() > 4 && data[1] == 0x7E && data[3] == 0x06 && data[4] == 0x02 {
        return Event::Identity(data.to_vec());
    }
    if data.starts_with(&SYSEX_HEADER) {
        let body = &data[SYSEX_HEADER.len()..data.len().saturating_sub(1)];
        match body {
            // 02 00 40 62 <p>: program changed, 1 = Arturia, 2 = DAW
            [0x02, 0x00, 0x40, 0x62, p] => {
                return Event::Program(if *p == 2 { Program::Daw } else { Program::Arturia })
            }
            // 02 00 40 01 <p>: reply to request program, 1 = DAW, 0 = Arturia
            [0x02, 0x00, 0x40, 0x01, p] => {
                return Event::Program(if *p == 1 { Program::Daw } else { Program::Arturia })
            }
            // 02 00 40 63 <b>: pad bank, 0 = A, 1 = B
            [0x02, 0x00, 0x40, 0x63, b] => return Event::PadBank(if *b == 1 { PadBank::B } else { PadBank::A }),
            _ => {}
        }
    }
    Event::Unknown(data.to_vec())
}

/// Decode one complete MIDI message from the `Minilab3 MIDI` port. `None` for messages with no effect (the
/// encoder's rest value). Splitting a byte stream into messages is [`MidiParser`]'s job.
pub fn decode(data: &[u8]) -> Option<Event> {
    let (&first, rest) = data.split_first()?;
    if first == 0xF0 {
        return Some(decode_sysex(data));
    }
    let status = first & 0xF0;
    let channel = first & 0x0F;
    let d1 = rest.first().copied().unwrap_or(0);
    let d2 = rest.get(1).copied().unwrap_or(0);
    let pad = if channel == PAD_CHANNEL { pad_from_note(d1) } else { None };
    match status {
        0x80 | 0x90 => {
            let on = status == 0x90 && d2 > 0;
            Some(match pad {
                Some((index, bank)) => Event::Pad { index, bank, velocity: d2, on },
                None => Event::Key { note: d1, velocity: d2, on, channel },
            })
        }
        0xA0 => match pad {
            Some((index, bank)) => Some(Event::PadPressure { index, bank, value: d2 }),
            None => Some(Event::Unknown(data.to_vec())),
        },
        0xB0 => match decode_cc(d1, d2) {
            Some(e) => Some(e),
            None if (d1 == DAW.encoder || Some(d1) == DAW.shifted_encoder || d1 == ARTURIA.encoder) => None,
            None => Some(Event::Unknown(data.to_vec())),
        },
        0xE0 => {
            let raw = ((d2 as i32) << 7) | d1 as i32;
            Some(Event::PitchBend(((raw - 8192) as f32 / 8191.0).max(-1.0)))
        }
        _ => Some(Event::Unknown(data.to_vec())),
    }
}

/// Splits a raw MIDI byte stream into messages: running status, SysEx, several messages per read, realtime bytes
/// in between. (A message-per-event assumption lost note-offs in the browser app.)
#[derive(Default)]
pub struct MidiParser {
    status: Option<u8>,
    buf: Vec<u8>,
    in_sysex: bool,
}

impl MidiParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, bytes: &[u8], mut out: impl FnMut(&[u8])) {
        for &b in bytes {
            if b >= 0xF8 {
                out(&[b]); // realtime: may appear anywhere, even inside SysEx
                continue;
            }
            if b == 0xF0 {
                self.buf.clear();
                self.buf.push(b);
                self.in_sysex = true;
                continue;
            }
            if self.in_sysex {
                self.buf.push(b);
                if b == END {
                    out(&self.buf);
                    self.buf.clear();
                    self.in_sysex = false;
                }
                continue;
            }
            if b & 0x80 != 0 {
                self.status = (b < 0xF0).then_some(b); // system common messages cancel running status
                self.buf.clear();
                self.buf.push(b);
            } else {
                let Some(status) = self.status else { continue };
                if self.buf.is_empty() {
                    self.buf.push(status); // running status
                }
                self.buf.push(b);
            }
            if let Some(status) = self.status {
                let len = if matches!(status & 0xF0, 0xC0 | 0xD0) { 2 } else { 3 };
                if self.buf.len() == len {
                    out(&self.buf);
                    self.buf.clear();
                }
            }
        }
    }
}

// --- Outbound -------------------------------------------------------------------------------------------------

fn sysex(body: &[u8]) -> Vec<u8> {
    let mut m = Vec::with_capacity(SYSEX_HEADER.len() + body.len() + 1);
    m.extend_from_slice(&SYSEX_HEADER);
    m.extend_from_slice(body);
    m.push(END);
    m
}

pub const IDENTITY_REQUEST: [u8; 6] = [0xF0, 0x7E, 0x7F, 0x06, 0x01, END];

/// Tells the device a DAW is connected. Required before screen messages work. The device answers with its pad bank.
pub fn connect() -> Vec<u8> {
    sysex(&[0x02, 0x00, 0x40, 0x6A, 0x21])
}

/// LEDs go back to the device's own colours.
pub fn disconnect() -> Vec<u8> {
    sysex(&[0x02, 0x00, 0x40, 0x6A, 0x20])
}

pub fn request_program() -> Vec<u8> {
    sysex(&[0x01, 0x00, 0x40, 0x01])
}

/// LED IDs for [`led`]. All verified on hardware (2026-10-29).
pub mod led_id {
    pub const SHIFT: u8 = 0x30;
    pub const OCT_MINUS: u8 = 0x31;
    pub const HOLD: u8 = 0x32;
    pub const OCT_PLUS: u8 = 0x33;
    pub const PADS_A: u8 = 0x34; // ..0x3B
    pub const PADS_B: u8 = 0x44; // ..0x4B
    /// Shift layer, pads 4-8: loop, stop, play, record, tap. Shown only while Shift is held.
    pub const TRANSPORT: u8 = 0x57; // ..0x5B
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const OFF: Rgb = Rgb { r: 0, g: 0, b: 0 };

    /// From 8-bit colour (0-255 per channel), as Reaper gives track colours.
    pub fn from_8bit(r: u8, g: u8, b: u8) -> Self {
        Rgb { r: r >> 1, g: g >> 1, b: b >> 1 }
    }

    /// Scale brightness, 0.0-1.0.
    pub fn scaled(self, k: f32) -> Self {
        let s = |c: u8| (c as f32 * k.clamp(0.0, 1.0)).round() as u8;
        Rgb { r: s(self.r), g: s(self.g), b: s(self.b) }
    }
}

/// Set one LED. Channels are 7-bit (0-127).
pub fn led(id: u8, color: Rgb) -> Vec<u8> {
    sysex(&[0x02, 0x02, 0x16, id & 0x7F, color.r.min(127), color.g.min(127), color.b.min(127)])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransportIcon {
    #[default]
    None,
    Play,
    Record,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Icons {
    /// Left of line 1.
    pub transport: TransportIcon,
    /// Left of line 2.
    pub arm: bool,
    /// Right of line 2; shows only while the device's Arp is on.
    pub arp: bool,
}

/// The device cuts lines by pixel width (proportional font): 14 capitals or 10 `W` fit on line 1, 18 digits on
/// line 2. Until there is a glyph-width table, cap at what always fits plus the device's own cut.
pub const LINE_1_MAX_CHARS: usize = 16;
pub const LINE_2_MAX_CHARS: usize = 24;

/// ASCII only: other characters become `?`.
fn ascii(text: &str, max: usize) -> impl Iterator<Item = u8> + '_ {
    text.chars().take(max).map(|c| if (' '..='~').contains(&c) { c as u8 } else { b'?' })
}

/// The persistent two-line screen.
pub fn screen(line1: &str, line2: &str, icons: Icons) -> Vec<u8> {
    let transport = match icons.transport {
        TransportIcon::None => 0,
        TransportIcon::Play => 2,
        TransportIcon::Record => 3,
    };
    let mut body = vec![0x04, 0x02, 0x60, 0x1F, 0x07, 0x01, transport, if icons.arm { 4 } else { 0 }];
    body.extend([if icons.arp { 1 } else { 0 }, 0x00, 0x01]);
    body.extend(ascii(line1, LINE_1_MAX_CHARS));
    body.extend([0x00, 0x02]);
    body.extend(ascii(line2, LINE_2_MAX_CHARS));
    body.push(0x00);
    sysex(&body)
}

/// Parameter name and value for the control with this CC. Ableton sends it whenever the mapped parameter changes;
/// when the device shows it is not verified yet.
pub fn parameter(cc: u8, name: &str, value: &str) -> Vec<u8> {
    let mut body = vec![0x04, 0x02, 0x60, 0x1F, 0x08, 0x02, cc & 0x7F, 0x00, 0x01];
    body.extend(ascii(name, LINE_1_MAX_CHARS));
    body.extend([0x00, 0x02]);
    body.extend(ascii(value, LINE_2_MAX_CHARS));
    body.push(0x00);
    sysex(&body)
}

/// The control with this CC has no parameter.
pub fn blank_parameter(cc: u8) -> Vec<u8> {
    sysex(&[0x04, 0x02, 0x60, 0x1F, 0x09, 0x02, cc & 0x7F, 0x00])
}

/// Set a knob's value in the device, so the next turn continues from it (verified: realign to 100, next turn
/// sends 101). Knob index 0-7.
pub fn knob_value(knob: u8, value: u8) -> Vec<u8> {
    sysex(&[0x21, 0x10, 0x00, 7 + (knob & 7), 0x00, value.min(127)])
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(s: &str) -> Vec<u8> {
        s.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect()
    }

    #[test]
    fn decodes_daw_controls() {
        assert_eq!(decode(&h("B0 56 40")), Some(Event::Knob { index: 0, value: 64, program: Program::Daw }));
        assert_eq!(decode(&h("B0 1F 7F")), Some(Event::Fader { index: 3, value: 127, program: Program::Daw }));
        assert_eq!(decode(&h("B0 1B 7F")), Some(Event::Shift { pressed: true }));
        assert_eq!(decode(&h("B0 1D 41")), Some(Event::Encoder { delta: 1, shifted: true }));
        assert_eq!(decode(&h("B0 1C 3F")), Some(Event::Encoder { delta: -1, shifted: false }));
        assert_eq!(decode(&h("B0 77 7F")), Some(Event::EncoderClick { pressed: true, shifted: true }));
        assert_eq!(decode(&h("B0 6B 7F")), Some(Event::Transport { button: TransportButton::Play, pressed: true }));
    }

    #[test]
    fn encoder_rest_value_is_dropped() {
        assert_eq!(decode(&h("B0 1C 40")), None);
        assert_eq!(decode(&h("B0 72 40")), None);
    }

    #[test]
    fn decodes_arturia_program_controls() {
        assert_eq!(decode(&h("B0 4A 10")), Some(Event::Knob { index: 0, value: 16, program: Program::Arturia }));
        assert_eq!(decode(&h("B0 09 7F")), Some(Event::Shift { pressed: true }));
    }

    #[test]
    fn daw_controls_stay_away_from_tracks() {
        for control in ["B0 56 40", "B0 0E 10", "B0 1C 41", "B0 1C 40", "B0 76 7F", "B0 1B 7F", "B0 6B 7F", "99 2C 64",
            "89 33 00", "A9 2D 30", "F0 00 20 6B 7F 42 02 00 40 63 01 F7"]
        {
            assert!(is_daw_control(&h(control)), "{control}");
        }
        // Keys, bank A, mod, pitch bend, sustain, the Arturia program's knobs.
        for music in ["90 3C 64", "80 3C 00", "99 24 64", "A9 25 30", "B0 01 40", "E0 00 40", "B0 40 7F", "B0 4A 20",
            "D0 30"]
        {
            assert!(!is_daw_control(&h(music)), "{music}");
        }
    }

    #[test]
    fn decodes_pads_and_keys() {
        assert_eq!(decode(&h("99 24 64")), Some(Event::Pad { index: 0, bank: PadBank::A, velocity: 100, on: true }));
        assert_eq!(decode(&h("99 33 00")), Some(Event::Pad { index: 7, bank: PadBank::B, velocity: 0, on: false }));
        assert_eq!(decode(&h("A9 25 30")), Some(Event::PadPressure { index: 1, bank: PadBank::A, value: 48 }));
        assert_eq!(decode(&h("90 3C 50")), Some(Event::Key { note: 60, velocity: 80, on: true, channel: 0 }));
        // Pad notes on another channel are keys.
        assert!(matches!(decode(&h("90 24 50")), Some(Event::Key { .. })));
    }

    #[test]
    fn decodes_status_sysex() {
        assert_eq!(decode(&h("F0 00 20 6B 7F 42 02 00 40 62 02 F7")), Some(Event::Program(Program::Daw)));
        assert_eq!(decode(&h("F0 00 20 6B 7F 42 02 00 40 62 01 F7")), Some(Event::Program(Program::Arturia)));
        assert_eq!(decode(&h("F0 00 20 6B 7F 42 02 00 40 01 01 F7")), Some(Event::Program(Program::Daw)));
        assert_eq!(decode(&h("F0 00 20 6B 7F 42 02 00 40 01 00 F7")), Some(Event::Program(Program::Arturia)));
        assert_eq!(decode(&h("F0 00 20 6B 7F 42 02 00 40 63 01 F7")), Some(Event::PadBank(PadBank::B)));
        assert!(matches!(decode(&h("F0 7E 7F 06 02 00 20 6B 02 00 04 04 45 00 02 01 F7")), Some(Event::Identity(_))));
    }

    #[test]
    fn pitch_bend_range() {
        assert_eq!(decode(&h("E0 00 40")), Some(Event::PitchBend(0.0)));
        assert_eq!(decode(&h("E0 7F 7F")), Some(Event::PitchBend(1.0)));
        assert_eq!(decode(&h("E0 00 00")), Some(Event::PitchBend(-1.0)));
    }

    #[test]
    fn parser_handles_running_status_sysex_and_realtime() {
        let mut p = MidiParser::new();
        let mut got = Vec::new();
        // Two notes with running status, a SysEx with a clock byte inside, then a CC split across two reads.
        p.push(&h("90 3C 50 3E 51 F0 00 20 F8 6B F7 B0 56"), |m| got.push(m.to_vec()));
        p.push(&h("40"), |m| got.push(m.to_vec()));
        assert_eq!(got, vec![h("90 3C 50"), h("90 3E 51"), h("F8"), h("F0 00 20 6B F7"), h("B0 56 40")]);
    }

    #[test]
    fn builds_verified_messages() {
        assert_eq!(connect(), h("F0 00 20 6B 7F 42 02 00 40 6A 21 F7"));
        assert_eq!(led(led_id::PADS_A, Rgb { r: 127, g: 0, b: 64 }), h("F0 00 20 6B 7F 42 02 02 16 34 7F 00 40 F7"));
        assert_eq!(knob_value(0, 100), h("F0 00 20 6B 7F 42 21 10 00 07 00 64 F7"));
        let icons = Icons { transport: TransportIcon::Play, arm: true, arp: false };
        assert_eq!(
            screen("HI", "AB", icons),
            h("F0 00 20 6B 7F 42 04 02 60 1F 07 01 02 04 00 00 01 48 49 00 02 41 42 00 F7")
        );
    }

    #[test]
    fn screen_text_is_ascii() {
        let m = screen("Çello", "", Icons::default());
        let line1 = &m[17..22];
        assert_eq!(line1, b"?ello");
    }

    #[test]
    fn colour_helpers() {
        assert_eq!(Rgb::from_8bit(255, 128, 0), Rgb { r: 127, g: 64, b: 0 });
        assert_eq!(Rgb { r: 100, g: 50, b: 0 }.scaled(0.5), Rgb { r: 50, g: 25, b: 0 });
    }
}
