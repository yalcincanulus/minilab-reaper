//! MiniLab 3 support without Reaper or the device: the protocol, the paced output queue and (later) the rules
//! that turn Reaper's state into screen and LED output and device input into Reaper actions.

pub mod protocol;
pub mod queue;
pub mod rules;
