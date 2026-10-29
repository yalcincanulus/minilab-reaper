//! Paced, keyed output queue.
//!
//! The device loses messages when ~200 arrive with no gap; a 1 ms gap was enough in tests (docs/minilab3-protocol.md,
//! pacing). Each message has a key (one per LED, one for the screen, ...):
//! - a message equal to the last one sent under its key is dropped, so callers can redraw freely;
//! - a newer message replaces a queued one with the same key, so fast changes collapse to the latest.
//!
//! Time comes from the caller, so the queue is testable and works with any clock.

use std::collections::{HashMap, VecDeque};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Connect,
    Screen,
    Led(u8),
    /// Parameter popup for the control with this CC.
    Parameter(u8),
    Knob(u8),
    /// Never deduplicated or replaced.
    Once,
}

pub const DEFAULT_GAP: Duration = Duration::from_millis(2);

pub struct OutQueue {
    gap: Duration,
    order: VecDeque<Key>,
    pending: HashMap<Key, Vec<u8>>,
    once: VecDeque<Vec<u8>>,
    sent: HashMap<Key, Vec<u8>>,
    last_send: Option<Duration>,
}

impl Default for OutQueue {
    fn default() -> Self {
        Self::new(DEFAULT_GAP)
    }
}

impl OutQueue {
    pub fn new(gap: Duration) -> Self {
        OutQueue {
            gap,
            order: VecDeque::new(),
            pending: HashMap::new(),
            once: VecDeque::new(),
            sent: HashMap::new(),
            last_send: None,
        }
    }

    pub fn push(&mut self, key: Key, msg: Vec<u8>) {
        if key == Key::Once {
            self.order.push_back(Key::Once);
            self.once.push_back(msg);
            return;
        }
        if self.sent.get(&key) == Some(&msg) {
            // Back to what the device already shows: cancel a pending change.
            if self.pending.remove(&key).is_some() {
                self.order.retain(|k| *k != key);
            }
            return;
        }
        if self.pending.insert(key, msg).is_none() {
            self.order.push_back(key);
        }
    }

    /// The device lost its state (reconnect, program change): send everything again on the next push.
    pub fn forget_sent(&mut self) {
        self.sent.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// Next message to send at time `now`, if the gap since the last one has passed.
    pub fn pop(&mut self, now: Duration) -> Option<Vec<u8>> {
        if let Some(last) = self.last_send {
            if now < last + self.gap {
                return None;
            }
        }
        let key = self.order.pop_front()?;
        let msg = if key == Key::Once {
            self.once.pop_front()?
        } else {
            let msg = self.pending.remove(&key)?;
            self.sent.insert(key, msg.clone());
            msg
        };
        self.last_send = Some(now);
        Some(msg)
    }

    /// Everything that may be sent by `now`, for callers that can only send in batches (e.g. once per Reaper
    /// timer tick). Spacing within the batch is the caller's job; `max` bounds the burst.
    pub fn drain_batch(&mut self, now: Duration, max: usize) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while out.len() < max {
            self.last_send = None; // a batch ignores the gap; `max` keeps it well under the device's buffer
            match self.pop(now) {
                Some(m) => out.push(m),
                None => break,
            }
        }
        if !out.is_empty() {
            self.last_send = Some(now);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn paces_messages() {
        let mut q = OutQueue::new(ms(2));
        q.push(Key::Led(1), vec![1]);
        q.push(Key::Led(2), vec![2]);
        assert_eq!(q.pop(ms(0)), Some(vec![1]));
        assert_eq!(q.pop(ms(1)), None);
        assert_eq!(q.pop(ms(2)), Some(vec![2]));
        assert_eq!(q.pop(ms(10)), None);
    }

    #[test]
    fn newer_message_replaces_pending_one_in_place() {
        let mut q = OutQueue::new(ms(0));
        q.push(Key::Screen, vec![1]);
        q.push(Key::Led(5), vec![2]);
        q.push(Key::Screen, vec![3]);
        assert_eq!(q.pop(ms(0)), Some(vec![3]));
        assert_eq!(q.pop(ms(0)), Some(vec![2]));
        assert!(q.is_empty());
    }

    #[test]
    fn unchanged_message_is_not_resent() {
        let mut q = OutQueue::new(ms(0));
        q.push(Key::Screen, vec![1]);
        q.pop(ms(0));
        q.push(Key::Screen, vec![1]);
        assert!(q.is_empty());
        q.forget_sent();
        q.push(Key::Screen, vec![1]);
        assert_eq!(q.pop(ms(0)), Some(vec![1]));
    }

    #[test]
    fn change_and_back_cancels_pending() {
        let mut q = OutQueue::new(ms(0));
        q.push(Key::Led(1), vec![1]);
        q.pop(ms(0));
        q.push(Key::Led(1), vec![2]);
        q.push(Key::Led(1), vec![1]);
        assert!(q.is_empty());
    }

    #[test]
    fn once_messages_keep_order_and_duplicates() {
        let mut q = OutQueue::new(ms(0));
        q.push(Key::Once, vec![9]);
        q.push(Key::Led(1), vec![1]);
        q.push(Key::Once, vec![9]);
        let got: Vec<_> = std::iter::from_fn(|| q.pop(ms(0))).collect();
        assert_eq!(got, vec![vec![9], vec![1], vec![9]]);
    }

    #[test]
    fn batch_is_bounded() {
        let mut q = OutQueue::new(ms(2));
        for i in 0..20 {
            q.push(Key::Led(i), vec![i]);
        }
        assert_eq!(q.drain_batch(ms(0), 8).len(), 8);
        assert_eq!(q.drain_batch(ms(1), 8).len(), 8);
        assert_eq!(q.drain_batch(ms(2), 8).len(), 4);
    }
}
