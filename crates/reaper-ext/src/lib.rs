//! Reaper extension: MiniLab 3 support.
//!
//! Thin layer. A control surface collects Reaper's state into [`rules::State`], pushes [`rules::render`]'s output
//! into the paced queue, polls the device input and runs [`rules::Command`]s. The logic lives in `minilab3-core`.
//!
//! Reaper calls `run()` about 30 times per second on the main thread, and the `set_*` callbacks on changes.

use std::cell::Cell;
use std::error::Error;
use std::ffi::{c_char, c_int, CStr};
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::ptr::null_mut;
use std::sync::OnceLock;
use std::time::Instant;

use fragile::Fragile;
use minilab3_core::protocol::{self, hex, MidiParser, TransportButton};
use minilab3_core::queue::{Key, OutQueue};
use minilab3_core::rules::{self, Command, DeviceStatus, Param, State, Track, Transport};
use minilab3_core::watchdog::{self, Watchdog};
use reaper_low::raw::MediaTrack;
use reaper_low::PluginContext;
use reaper_macros::reaper_extension_plugin;
use reaper_medium::{ControlSurface, ReaperSession};

const DEVICE_NAME: &str = "Minilab3: MIDI";
/// Messages per `run()` tick (~30 ms): well under what the device's input buffer takes.
const MAX_PER_TICK: usize = 8;
const LOG_FILE: &str = "minilab3.log";

#[reaper_extension_plugin]
fn plugin_main(context: PluginContext) -> Result<(), Box<dyn Error>> {
    let mut session = ReaperSession::load(context);
    let surface = Surface::new(*session.reaper().low());
    session.plugin_register_add_csurf_inst(Box::new(surface))?;
    let _ = SESSION.set(Fragile::new(session));
    Ok(())
}

/// Keeps the session (and so the registered surface) alive while Reaper runs.
static SESSION: OnceLock<Fragile<ReaperSession>> = OnceLock::new();

/// Small safe wrappers over the C API. All calls happen on Reaper's main thread.
struct Api(reaper_low::Reaper);

impl Api {
    fn find_device(&self, outputs: bool) -> Option<c_int> {
        let mut buf = [0 as c_char; 256];
        let count = if outputs { self.0.GetNumMIDIOutputs() } else { self.0.GetNumMIDIInputs() };
        (0..count).find(|&i| unsafe {
            let ok = if outputs {
                self.0.GetMIDIOutputName(i, buf.as_mut_ptr(), buf.len() as c_int)
            } else {
                self.0.GetMIDIInputName(i, buf.as_mut_ptr(), buf.len() as c_int)
            };
            ok && CStr::from_ptr(buf.as_ptr()).to_string_lossy().contains(DEVICE_NAME)
        })
    }

    fn send_midi(&self, output: c_int, msg: &[u8]) {
        unsafe { self.0.SendMIDIMessageToHardware(output, msg.as_ptr() as *const c_char, msg.len() as c_int) }
    }

    /// Input events newer than `after`, oldest first, as (index, device, bytes).
    fn recent_input(&self, after: c_int) -> (c_int, Vec<(c_int, Vec<u8>)>) {
        let mut events = Vec::new();
        let mut newest = after;
        let mut buf = [0u8; 1024];
        for i in 0.. {
            let mut size = buf.len() as c_int;
            let (mut ts, mut dev, mut pos, mut loops) = (0, 0, 0.0, 0);
            let idx = unsafe {
                self.0.MIDI_GetRecentInputEvent(
                    i,
                    buf.as_mut_ptr() as *mut c_char,
                    &mut size,
                    &mut ts,
                    &mut dev,
                    &mut pos,
                    &mut loops,
                )
            };
            if idx == 0 || idx <= after {
                break;
            }
            newest = newest.max(idx);
            events.push((dev, buf[..size.clamp(0, buf.len() as c_int) as usize].to_vec()));
        }
        events.reverse();
        (newest, events)
    }

    fn newest_input_index(&self) -> c_int {
        let mut size = 0;
        let (mut ts, mut dev, mut pos, mut loops) = (0, 0, 0.0, 0);
        unsafe {
            self.0.MIDI_GetRecentInputEvent(0, null_mut(), &mut size, &mut ts, &mut dev, &mut pos, &mut loops)
        }
    }

    fn track(&self, index: usize) -> Option<*mut MediaTrack> {
        let t = unsafe { self.0.GetTrack(null_mut(), index as c_int) };
        (!t.is_null()).then_some(t)
    }

    fn track_count(&self) -> usize {
        unsafe { self.0.CountTracks(null_mut()) as usize }
    }

    fn selected_track(&self) -> Option<*mut MediaTrack> {
        let t = unsafe { self.0.GetSelectedTrack(null_mut(), 0) };
        (!t.is_null()).then_some(t)
    }

    fn info(&self, track: *mut MediaTrack, key: &CStr) -> f64 {
        unsafe { self.0.GetMediaTrackInfo_Value(track, key.as_ptr()) }
    }

    fn set_info(&self, track: *mut MediaTrack, key: &CStr, value: f64) {
        unsafe { self.0.SetMediaTrackInfo_Value(track, key.as_ptr(), value) };
    }

    fn track_name(&self, track: *mut MediaTrack) -> String {
        let mut buf = [0 as c_char; 256];
        unsafe {
            self.0.GetTrackName(track, buf.as_mut_ptr(), buf.len() as c_int);
            CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
        }
    }

    fn track_color(&self, track: *mut MediaTrack) -> Option<(u8, u8, u8)> {
        let native = unsafe { self.0.GetTrackColor(track) };
        if native == 0 {
            return None;
        }
        let (mut r, mut g, mut b) = (0, 0, 0);
        unsafe { self.0.ColorFromNative(native & 0xFFFFFF, &mut r, &mut g, &mut b) };
        Some((r as u8, g as u8, b as u8))
    }

    /// Parameter `param` of the track's first FX, if it exists.
    fn knob_param(&self, track: *mut MediaTrack, param: c_int) -> Option<Param> {
        unsafe {
            if self.0.TrackFX_GetCount(track) == 0 || param >= self.0.TrackFX_GetNumParams(track, 0) {
                return None;
            }
            let mut name = [0 as c_char; 64];
            let mut value = [0 as c_char; 64];
            self.0.TrackFX_GetParamName(track, 0, param, name.as_mut_ptr(), name.len() as c_int);
            self.0.TrackFX_GetFormattedParamValue(track, 0, param, value.as_mut_ptr(), value.len() as c_int);
            Some(Param {
                name: CStr::from_ptr(name.as_ptr()).to_string_lossy().into_owned(),
                value: CStr::from_ptr(value.as_ptr()).to_string_lossy().into_owned(),
                normalized: self.0.TrackFX_GetParamNormalized(track, 0, param),
            })
        }
    }

    fn command(&self, id: c_int) {
        self.0.Main_OnCommand(id, 0)
    }

    fn resource_path(&self) -> String {
        unsafe { CStr::from_ptr(self.0.GetResourcePath()).to_string_lossy().into_owned() }
    }
}

struct Surface {
    api: Api,
    out_dev: Option<c_int>,
    in_dev: Option<c_int>,
    queue: OutQueue,
    status: DeviceStatus,
    parser: MidiParser,
    last_input: c_int,
    dirty: Cell<bool>,
    started: Instant,
    last_device_scan: Option<Instant>,
    watchdog: Watchdog,
    log: Option<File>,
}

impl fmt::Debug for Surface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Surface").field("out_dev", &self.out_dev).field("in_dev", &self.in_dev).finish()
    }
}

impl Surface {
    fn new(low: reaper_low::Reaper) -> Self {
        let api = Api(low);
        let log = File::create(format!("{}/{}", api.resource_path(), LOG_FILE)).ok();
        let last_input = api.newest_input_index();
        let mut s = Surface {
            api,
            out_dev: None,
            in_dev: None,
            queue: OutQueue::default(),
            status: DeviceStatus::default(),
            parser: MidiParser::new(),
            last_input,
            dirty: Cell::new(true),
            started: Instant::now(),
            last_device_scan: None,
            watchdog: Watchdog::default(),
            log,
        };
        s.log("loaded");
        s
    }

    fn log(&mut self, msg: impl AsRef<str>) {
        let t = self.started.elapsed().as_secs_f64();
        if let Some(f) = self.log.as_mut() {
            let _ = writeln!(f, "{t:9.3} {}", msg.as_ref());
        }
    }

    /// Find the MiniLab's ports; retried every 2 s until found (Reaper may still be opening devices).
    fn ensure_devices(&mut self) -> bool {
        if self.out_dev.is_some() && self.in_dev.is_some() {
            return true;
        }
        if self.last_device_scan.is_some_and(|t| t.elapsed().as_secs() < 2) {
            return false;
        }
        self.last_device_scan = Some(Instant::now());
        self.out_dev = self.api.find_device(true);
        self.in_dev = self.api.find_device(false);
        let (o, i) = (self.out_dev, self.in_dev);
        self.log(format!("devices: out {o:?} in {i:?}"));
        if o.is_some() && i.is_some() {
            self.connect();
            true
        } else {
            false
        }
    }

    fn connect(&mut self) {
        self.queue.forget_sent();
        self.queue.push(Key::Connect, protocol::connect());
        self.queue.push(Key::Once, protocol::request_program());
        self.dirty.set(true);
    }

    fn state(&self) -> State {
        let api = &self.api;
        let count = api.track_count();
        let tracks = (0..count.min(8))
            .filter_map(|i| api.track(i))
            .map(|t| Track { name: api.track_name(t), color: api.track_color(t) })
            .collect();
        let selected = api.selected_track();
        let play = api.0.GetPlayState();
        let repeat = api.0.GetSetRepeat(-1) == 1;
        State {
            tracks,
            track_count: count,
            selected: selected.map(|t| api.info(t, c"IP_TRACKNUMBER") as usize - 1),
            selected_name: selected.map(|t| api.track_name(t)).unwrap_or_default(),
            selected_volume: selected.map(|t| api.info(t, c"D_VOL")).unwrap_or(1.0),
            selected_armed: selected.is_some_and(|t| api.info(t, c"I_RECARM") == 1.0),
            transport: Transport { playing: play & 1 != 0, recording: play & 4 != 0, repeat },
            knobs: std::array::from_fn(|i| selected.and_then(|t| api.knob_param(t, i as c_int))),
        }
    }

    fn read_input(&mut self) {
        let (newest, events) = self.api.recent_input(self.last_input);
        self.last_input = newest;
        for (dev, bytes) in events {
            if Some(dev) != self.in_dev {
                continue;
            }
            if self.watchdog.on_input(self.started.elapsed()) {
                self.log("device is back: connect and redraw");
                self.connect();
            }
            let mut messages = Vec::new();
            self.parser.push(&bytes, |m| messages.push(m.to_vec()));
            for m in messages {
                let Some(event) = protocol::decode(&m) else { continue };
                let cmd = self.status.update(&event).or_else(|| rules::command(&event));
                // Watchdog pings get a program reply every few seconds: log those only when something changes.
                if cmd.is_some() || !matches!(event, protocol::Event::Program(_)) {
                    self.log(format!("in  {} {event:?}", hex(&m)));
                }
                if let Some(cmd) = cmd {
                    self.run_command(cmd);
                }
            }
        }
    }

    fn run_command(&mut self, cmd: Command) {
        self.log(format!("cmd {cmd:?}"));
        let api = &self.api;
        match cmd {
            Command::SelectTrackBy(step) => {
                let current = api.selected_track().map(|t| api.info(t, c"IP_TRACKNUMBER") as i64 - 1).unwrap_or(-1);
                let last = api.track_count() as i64 - 1;
                if last >= 0 {
                    self.select((current + step as i64).clamp(0, last) as usize);
                }
            }
            Command::SelectTrack(i) => self.select(i),
            Command::ToggleArm => {
                if let Some(t) = api.selected_track() {
                    let armed = api.info(t, c"I_RECARM");
                    api.set_info(t, c"I_RECARM", 1.0 - armed);
                }
            }
            Command::Transport(button) => api.command(match button {
                TransportButton::Loop => 1068,   // Transport: Toggle repeat
                TransportButton::Stop => 1016,   // Transport: Stop
                TransportButton::Play => 1007,   // Transport: Play
                TransportButton::Record => 1013, // Transport: Record
                TransportButton::Tap => 1134,    // Transport: Tap tempo
            }),
            Command::SetVolumeFader(v) => {
                if let Some(t) = api.selected_track() {
                    // Reaper's fader curve: slider 0..1000 -> dB (+6 dB at the top), like its own mixer fader.
                    let db = api.0.SLIDER2DB(v as f64 / 127.0 * 1000.0);
                    api.set_info(t, c"D_VOL", if v == 0 { 0.0 } else { 10f64.powf(db / 20.0) });
                }
            }
            Command::SetKnob(knob, value) => {
                if let Some(t) = api.selected_track() {
                    unsafe { api.0.TrackFX_SetParamNormalized(t, 0, knob as c_int, value as f64 / 127.0) };
                }
            }
            Command::Redraw => self.connect(),
        }
        self.dirty.set(true);
    }

    fn select(&self, index: usize) {
        if let Some(t) = self.api.track(index) {
            unsafe {
                self.api.0.SetOnlyTrackSelected(t);
            }
            self.api.command(40913); // Track: Vertical scroll selected tracks into view
        }
    }

    fn flush(&mut self) {
        let Some(out) = self.out_dev else { return };
        let batch = self.queue.drain_batch(self.started.elapsed(), MAX_PER_TICK);
        for m in batch {
            self.api.send_midi(out, &m);
            self.log(format!("out {}", hex(&m)));
        }
    }
}

impl ControlSurface for Surface {
    fn run(&mut self) {
        if !self.ensure_devices() {
            return;
        }
        self.read_input();
        let recording = self.api.0.GetPlayState() & 4 != 0;
        match self.watchdog.tick(self.started.elapsed(), recording) {
            Some(watchdog::Action::Ping) => self.queue.push(Key::Once, protocol::request_program()),
            Some(watchdog::Action::ResetMidi) => {
                self.log("device silent: Reset all MIDI devices");
                self.api.command(41175);
            }
            None => {}
        }
        if self.dirty.replace(false) {
            for (key, msg) in rules::render(&self.state()) {
                self.queue.push(key, msg);
            }
        }
        self.flush();
    }

    fn set_track_list_change(&self) {
        self.dirty.set(true);
    }
    fn set_surface_volume(&self, _: reaper_medium::SetSurfaceVolumeArgs) {
        self.dirty.set(true);
    }
    fn set_surface_selected(&self, _: reaper_medium::SetSurfaceSelectedArgs) {
        self.dirty.set(true);
    }
    fn set_surface_rec_arm(&self, _: reaper_medium::SetSurfaceRecArmArgs) {
        self.dirty.set(true);
    }
    fn set_play_state(&self, _: reaper_medium::SetPlayStateArgs) {
        self.dirty.set(true);
    }
    fn set_repeat_state(&self, _: reaper_medium::SetRepeatStateArgs) {
        self.dirty.set(true);
    }
    fn set_track_title(&self, _: reaper_medium::SetTrackTitleArgs) {
        self.dirty.set(true);
    }
    fn ext_set_fx_param(&self, _: reaper_medium::ExtSetFxParamArgs) -> i32 {
        self.dirty.set(true);
        0
    }
    fn ext_set_fx_change(&self, _: reaper_medium::ExtSetFxChangeArgs) -> i32 {
        self.dirty.set(true);
        0
    }
}

