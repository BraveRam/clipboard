use parking_lot::Mutex;
use std::time::{Duration, Instant};

const TOGGLE_DEBOUNCE: Duration = Duration::from_millis(250);

/// Bound the main-thread queue even if callers bypass the desktop helper.
pub struct ToggleGate {
    state: Mutex<(bool, Option<Instant>)>,
}

impl ToggleGate {
    pub const fn new() -> Self {
        Self {
            state: Mutex::new((false, None)),
        }
    }

    pub fn begin(&self, now: Instant) -> bool {
        let mut state = self.state.lock();
        if state.0
            || state.1.is_some_and(|last| {
                now.saturating_duration_since(last) < TOGGLE_DEBOUNCE
            })
        {
            return false;
        }
        *state = (true, Some(now));
        true
    }

    pub fn finish(&self) {
        self.state.lock().0 = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drops_repeats_and_keeps_only_one_request_pending() {
        let gate = ToggleGate::new();
        let start = Instant::now();
        assert!(gate.begin(start));
        for offset in 1..1000 {
            assert!(!gate.begin(start + Duration::from_millis(offset)));
        }
        gate.finish();
        assert!(!gate.begin(start + Duration::from_millis(249)));
        assert!(gate.begin(start + Duration::from_millis(250)));
        gate.finish();
        assert!(gate.begin(start + Duration::from_millis(500)));
    }
}
