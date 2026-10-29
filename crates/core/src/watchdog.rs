//! Notices when the device goes away (unplugged, powered off) and comes back.
//!
//! Through PipeWire's bridge, a replugged MiniLab gets new ports, and Reaper keeps its links to the old ones, so
//! nothing arrives in either direction. The device also restarts and forgets our connect. So: ask the device for
//! its program when it has been quiet for a while; when it stops answering, ask Reaper to reset its MIDI devices
//! (which relinks the ports) until it answers again; then connect and redraw.
//!
//! Time comes from the caller.

use std::time::Duration;

/// Ask after this much silence.
pub const PING_AFTER: Duration = Duration::from_secs(3);
/// Lost after this much silence (two unanswered pings).
pub const LOST_AFTER: Duration = Duration::from_secs(7);
/// While lost, reset Reaper's MIDI devices this often.
pub const RESET_EVERY: Duration = Duration::from_secs(4);
/// A gap this long between ticks means Reaper's main loop was frozen (a modal dialog); silence during it says
/// nothing about the device.
pub const FROZEN_AFTER: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Send [`request_program`](crate::protocol::request_program); the device answers in both programs.
    Ping,
    /// Run Reaper's "Reset all MIDI devices".
    ResetMidi,
}

#[derive(Debug, Default)]
pub struct Watchdog {
    last_input: Duration,
    last_ping: Option<Duration>,
    last_reset: Option<Duration>,
    last_tick: Option<Duration>,
    lost: bool,
}

impl Watchdog {
    pub fn new(now: Duration) -> Self {
        Watchdog { last_input: now, ..Default::default() }
    }

    pub fn is_lost(&self) -> bool {
        self.lost
    }

    /// Anything arrived from the device. Returns true when it was lost and is back: connect and redraw.
    pub fn on_input(&mut self, now: Duration) -> bool {
        self.last_input = now;
        self.last_ping = None;
        std::mem::replace(&mut self.lost, false)
    }

    /// Call regularly. `quiet` pauses the watchdog (e.g. while recording: no ping replies in a take, no MIDI reset
    /// in the middle of it); silence counts again from when it ends.
    pub fn tick(&mut self, now: Duration, quiet: bool) -> Option<Action> {
        let frozen = self.last_tick.is_some_and(|t| now.saturating_sub(t) >= FROZEN_AFTER);
        self.last_tick = Some(now);
        if quiet || frozen {
            self.last_input = now;
            self.last_ping = None;
            return None;
        }
        let silence = now.saturating_sub(self.last_input);
        if silence >= LOST_AFTER {
            self.lost = true;
        }
        if self.lost {
            if self.last_reset.is_none_or(|t| now.saturating_sub(t) >= RESET_EVERY) {
                self.last_reset = Some(now);
                self.last_ping = Some(now);
                return Some(Action::ResetMidi);
            }
        } else {
            self.last_reset = None;
        }
        if silence < PING_AFTER {
            return None;
        }
        if self.last_ping.is_none_or(|t| now.saturating_sub(t) >= PING_AFTER) {
            self.last_ping = Some(now);
            return Some(Action::Ping);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(n: f64) -> Duration {
        Duration::from_secs_f64(n)
    }

    /// Tick every 0.1 s from `from` to `to` (like Reaper's ~30 ms `run()`), collecting (time, action).
    fn run(w: &mut Watchdog, from: f64, to: f64, quiet: bool) -> Vec<(f64, Action)> {
        let mut out = Vec::new();
        let mut t = from;
        while t <= to + 1e-9 {
            if let Some(a) = w.tick(s(t), quiet) {
                out.push(((t * 10.0).round() / 10.0, a));
            }
            t += 0.1;
        }
        out
    }

    #[test]
    fn pings_after_silence_and_stays_quiet_while_answered() {
        let mut w = Watchdog::new(s(0.0));
        assert_eq!(run(&mut w, 0.0, 3.5, false), vec![(3.0, Action::Ping)]);
        assert!(!w.on_input(s(3.5))); // the answer
        assert_eq!(run(&mut w, 3.6, 6.4, false), vec![]);
        assert_eq!(run(&mut w, 6.5, 6.6, false), vec![(6.5, Action::Ping)]);
    }

    #[test]
    fn lost_device_triggers_resets_until_it_answers() {
        let mut w = Watchdog::new(s(0.0));
        let got = run(&mut w, 0.0, 11.0, false);
        // While lost it keeps pinging between resets, to notice the return sooner.
        let expected = [(3.0, Action::Ping), (6.0, Action::Ping), (7.0, Action::ResetMidi), (10.0, Action::Ping), (11.0, Action::ResetMidi)];
        assert_eq!(got, expected);
        assert!(w.is_lost());
        assert!(w.on_input(s(11.5))); // back: reconnect
        assert!(!w.is_lost());
        assert!(!w.on_input(s(11.6)));
    }

    #[test]
    fn a_frozen_main_loop_is_not_silence() {
        let mut w = Watchdog::new(s(0.0));
        assert_eq!(run(&mut w, 0.0, 0.5, false), vec![]);
        assert_eq!(w.tick(s(30.0), false), None); // Reaper was stuck in a dialog for 30 s
        assert!(!w.is_lost());
        // Silence counts again from 30 s: the first ping comes 3 s later.
        assert_eq!(run(&mut w, 30.1, 33.0, false), vec![(33.0, Action::Ping)]);
    }

    #[test]
    fn quiet_pauses_everything() {
        let mut w = Watchdog::new(s(0.0));
        assert_eq!(run(&mut w, 0.0, 60.0, true), vec![]);
        assert!(!w.is_lost());
        assert_eq!(run(&mut w, 60.1, 63.0, false), vec![(63.0, Action::Ping)]);
    }
}
