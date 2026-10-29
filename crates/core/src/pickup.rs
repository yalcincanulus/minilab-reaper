//! Fader pickup.
//!
//! The faders are absolute, and the value they control can change elsewhere (another track selected, the mouse in
//! Reaper). Applying the fader position right away would make the value jump. Instead a fader takes over only once
//! it reaches the current value: within [`CATCH`] of it, or crossing it between two moves.
//!
//! Positions are 0..=127 on both sides; the caller maps Reaper values (volume curve, pan) to positions.

/// How close a fader must come to the current value to take over.
pub const CATCH: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    /// The fader controls the value: apply this position.
    Apply(u8),
    /// Not caught yet. `up` tells which way to move to catch it.
    Waiting { up: bool },
}

#[derive(Debug, Default, Clone)]
struct Fader {
    caught: bool,
    /// The last position we applied, to notice changes made elsewhere.
    applied: Option<u8>,
    /// The fader's previous position, to notice crossing the target between two moves.
    previous: Option<u8>,
}

#[derive(Debug, Default, Clone)]
pub struct Pickup {
    faders: [Fader; 4],
}

impl Pickup {
    /// The fader moved to `position`; `target` is where the value it controls is now.
    pub fn on_move(&mut self, index: usize, position: u8, target: u8) -> Move {
        let f = &mut self.faders[index];
        let previous = f.previous.replace(position);
        if !f.caught {
            let near = position.abs_diff(target) <= CATCH;
            let crossed = previous.is_some_and(|p| (p < target) != (position < target) && p != position);
            f.caught = near || crossed;
        }
        if f.caught {
            f.applied = Some(position);
            Move::Apply(position)
        } else {
            Move::Waiting { up: position < target }
        }
    }

    /// Call with the current target positions on every redraw. A target that moved away from what the fader last
    /// applied was changed elsewhere: the fader must catch it again.
    pub fn sync(&mut self, index: usize, target: Option<u8>) {
        let f = &mut self.faders[index];
        let moved_elsewhere = match (target, f.applied) {
            (Some(t), Some(a)) => t.abs_diff(a) > CATCH,
            (None, _) => true,
            _ => false,
        };
        if f.caught && moved_elsewhere {
            f.caught = false;
            f.applied = None;
        }
    }

    /// The faders now control something else (another track): catch everything again.
    pub fn release_all(&mut self) {
        for f in &mut self.faders {
            f.caught = false;
            f.applied = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_until_near_then_follows() {
        let mut p = Pickup::default();
        assert_eq!(p.on_move(0, 10, 100), Move::Waiting { up: true });
        assert_eq!(p.on_move(0, 60, 100), Move::Waiting { up: true });
        assert_eq!(p.on_move(0, 98, 100), Move::Apply(98));
        assert_eq!(p.on_move(0, 40, 100), Move::Apply(40)); // caught: follows anywhere
    }

    #[test]
    fn catches_when_crossing_between_moves() {
        let mut p = Pickup::default();
        assert_eq!(p.on_move(1, 120, 64), Move::Waiting { up: false });
        assert_eq!(p.on_move(1, 20, 64), Move::Apply(20)); // a fast move jumped over 64
    }

    #[test]
    fn change_elsewhere_releases_the_fader() {
        let mut p = Pickup::default();
        assert_eq!(p.on_move(0, 64, 64), Move::Apply(64));
        p.sync(0, Some(65)); // our own change, rounded: still caught
        assert_eq!(p.on_move(0, 70, 65), Move::Apply(70));
        p.sync(0, Some(20)); // the mouse moved it in Reaper
        assert_eq!(p.on_move(0, 72, 20), Move::Waiting { up: false });
    }

    #[test]
    fn release_all_on_track_change() {
        let mut p = Pickup::default();
        p.on_move(3, 64, 64);
        p.release_all();
        assert_eq!(p.on_move(3, 80, 64), Move::Waiting { up: false });
    }

    #[test]
    fn faders_are_independent() {
        let mut p = Pickup::default();
        assert_eq!(p.on_move(0, 64, 64), Move::Apply(64));
        assert_eq!(p.on_move(1, 0, 64), Move::Waiting { up: true });
    }
}
