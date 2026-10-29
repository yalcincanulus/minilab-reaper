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
pub const RESET_EVERY: Duration = Duration::from_secs(10);

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
        if quiet {
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

    #[test]
    fn pings_after_silence_and_stays_quiet_while_answered() {
        let mut w = Watchdog::new(s(0.0));
        assert_eq!(w.tick(s(2.0), false), None);
        assert_eq!(w.tick(s(3.0), false), Some(Action::Ping));
        assert_eq!(w.tick(s(3.5), false), None);
        assert!(!w.on_input(s(3.6))); // the answer
        assert_eq!(w.tick(s(5.0), false), None);
        assert_eq!(w.tick(s(6.6), false), Some(Action::Ping));
    }

    #[test]
    fn lost_device_triggers_resets_until_it_answers() {
        let mut w = Watchdog::new(s(0.0));
        assert_eq!(w.tick(s(3.0), false), Some(Action::Ping));
        assert_eq!(w.tick(s(6.0), false), Some(Action::Ping));
        assert_eq!(w.tick(s(7.0), false), Some(Action::ResetMidi));
        assert!(w.is_lost());
        assert_eq!(w.tick(s(9.0), false), None);
        assert_eq!(w.tick(s(10.0), false), Some(Action::Ping));
        assert_eq!(w.tick(s(17.0), false), Some(Action::ResetMidi));
        assert!(w.on_input(s(18.0))); // back: reconnect
        assert!(!w.is_lost());
        assert!(!w.on_input(s(18.1)));
    }

    #[test]
    fn quiet_pauses_everything() {
        let mut w = Watchdog::new(s(0.0));
        assert_eq!(w.tick(s(5.0), true), None);
        assert_eq!(w.tick(s(60.0), true), None);
        assert!(!w.is_lost());
        assert_eq!(w.tick(s(62.0), false), None); // silence counts from the end of the quiet time
        assert_eq!(w.tick(s(63.0), false), Some(Action::Ping));
    }
}
