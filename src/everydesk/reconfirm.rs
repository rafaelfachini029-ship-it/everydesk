//! Periodic reconfirmation for contacts under `Policy::Reconfirm { every_min }`.
//!
//! Pure state machine; `Connection` owns one `ReconfirmState` and drives it from its `select!`
//! loop. The prompt never touches the stream: the session keeps running while the ring shows.

use hbb_common::tokio::time::{Duration, Instant};

/// How long the owner has to answer a reconfirmation before the session ends.
pub const RECONFIRM_RING_SECS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconfirmState {
    pub every: Duration,
    /// When the next prompt is due (meaningful while `asked_at` is `None`).
    pub next_at: Instant,
    /// When the pending prompt was shown, if one is pending.
    pub asked_at: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    Continue { next_at: Instant },
    Close,
}

fn every(every_min: u32) -> Duration {
    Duration::from_secs(u64::from(every_min.max(1)) * 60)
}

pub fn next_reconfirm_at(start: Instant, every_min: u32) -> Instant {
    start + every(every_min)
}

pub fn reconfirm_expired_at(asked: Instant) -> Instant {
    asked + Duration::from_secs(RECONFIRM_RING_SECS)
}

impl ReconfirmState {
    pub fn new(start: Instant, every_min: u32) -> Self {
        ReconfirmState {
            every: every(every_min),
            next_at: next_reconfirm_at(start, every_min),
            asked_at: None,
        }
    }

    /// When the next prompt fires, if none is pending.
    pub fn due_at(&self) -> Option<Instant> {
        self.asked_at.is_none().then_some(self.next_at)
    }

    /// When the pending prompt expires, if one is pending.
    pub fn expires_at(&self) -> Option<Instant> {
        self.asked_at.map(reconfirm_expired_at)
    }

    pub fn ask(&mut self, now: Instant) {
        self.asked_at = Some(now);
    }

    /// Applies a transition from `on_result`.
    pub fn apply(&mut self, t: Transition) {
        if let Transition::Continue { next_at } = t {
            self.asked_at = None;
            self.next_at = next_at;
        }
    }
}

/// The owner's answer `ok` arriving at `now`. A deny always closes. A confirm reschedules from
/// `now` only while a prompt is pending and unexpired; past the ring's end it is too late
/// (closes), and with nothing pending it changes nothing.
pub fn on_result(state: &ReconfirmState, ok: bool, now: Instant) -> Transition {
    if !ok {
        return Transition::Close;
    }
    match state.asked_at {
        Some(asked) if now >= reconfirm_expired_at(asked) => Transition::Close,
        Some(_) => Transition::Continue {
            next_at: now + state.every,
        },
        None => Transition::Continue {
            next_at: state.next_at,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconfirm_schedule_every_20_min() {
        let start = Instant::now();
        assert_eq!(
            next_reconfirm_at(start, 20),
            start + Duration::from_secs(1200)
        );
        let s = ReconfirmState::new(start, 20);
        assert_eq!(s.next_at, start + Duration::from_secs(1200));
        assert_eq!(s.asked_at, None);
        // "livre" values below a minute are clamped, never a busy loop.
        assert_eq!(next_reconfirm_at(start, 0), start + Duration::from_secs(60));
    }

    #[test]
    fn reconfirm_expires_after_60s() {
        let asked = Instant::now();
        assert_eq!(reconfirm_expired_at(asked), asked + Duration::from_secs(60));
        assert_eq!(RECONFIRM_RING_SECS, 60);
    }

    #[test]
    fn confirm_at_59s_reschedules_without_close() {
        let start = Instant::now();
        let mut s = ReconfirmState::new(start, 20);
        let asked = s.next_at;
        s.ask(asked);
        assert_eq!(s.asked_at, Some(asked));
        let now = asked + Duration::from_secs(59);
        let t = on_result(&s, true, now);
        assert_eq!(
            t,
            Transition::Continue {
                next_at: now + Duration::from_secs(1200)
            }
        );
        s.apply(t);
        assert_eq!(s.asked_at, None);
        assert_eq!(s.next_at, now + Duration::from_secs(1200));
    }

    #[test]
    fn deny_closes() {
        let start = Instant::now();
        let mut s = ReconfirmState::new(start, 45);
        s.ask(s.next_at);
        assert_eq!(on_result(&s, false, s.next_at), Transition::Close);
        // A deny with nothing pending still ends the session: the owner said no.
        let idle = ReconfirmState::new(start, 45);
        assert_eq!(on_result(&idle, false, start), Transition::Close);
    }

    #[test]
    fn late_or_unsolicited_confirm() {
        let start = Instant::now();
        let mut s = ReconfirmState::new(start, 20);
        let asked = s.next_at;
        s.ask(asked);
        // At or past the ring's end the timeout has won.
        assert_eq!(
            on_result(&s, true, asked + Duration::from_secs(60)),
            Transition::Close
        );
        // A confirm with nothing pending changes nothing.
        let idle = ReconfirmState::new(start, 20);
        assert_eq!(
            on_result(&idle, true, start + Duration::from_secs(5)),
            Transition::Continue {
                next_at: idle.next_at
            }
        );
    }
}
