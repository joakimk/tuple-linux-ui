//! Boot-time contact refresh.
//!
//! The first `state` the daemon streams after it starts can be signed in with an
//! empty contact list, and no `contacts` event follows once the list has loaded.
//! While signed in with no contacts, poll `tuple state --raw` with backoff and
//! adopt the first snapshot that has contacts.

use std::time::Duration;

use crate::model::Phase;
use crate::tuple::FollowEvent;

use super::Store;

/// Delays before each poll while signed in with no contacts; the last one repeats.
pub const REFRESH_DELAYS: [Duration; 5] =
    [Duration::from_secs(1), Duration::from_secs(2), Duration::from_secs(5), Duration::from_secs(10), Duration::from_secs(30)];

impl Store {
    /// Start polling if the current state is signed in without contacts (no-op otherwise).
    pub(super) fn maybe_refresh_contacts(&self) {
        if !self.needs_contacts() || self.inner.borrow().refreshing {
            return;
        }
        self.inner.borrow_mut().refreshing = true;
        let store = self.clone();
        glib::spawn_future_local(async move {
            for attempt in 0.. {
                glib::timeout_future(REFRESH_DELAYS[attempt.min(REFRESH_DELAYS.len() - 1)]).await;
                if !store.needs_contacts() {
                    break;
                }
                let cli = store.cli();
                let args = ["state", "--raw", "--format", "json"].iter().map(|a| a.to_string()).collect();
                let result = store.runtime.spawn(async move { cli.run(args).await }).await;
                let snapshot = result.ok().and_then(|r| r.json());
                let loaded = snapshot.as_ref().and_then(|s| s.get("contacts")).and_then(|c| c.as_array()).is_some_and(|c| !c.is_empty());
                if loaded && store.needs_contacts() {
                    store.apply(FollowEvent::Event { kind: "state".into(), data: snapshot.expect("checked") });
                }
                if !store.needs_contacts() {
                    break;
                }
            }
            store.inner.borrow_mut().refreshing = false;
        });
    }

    fn needs_contacts(&self) -> bool {
        let state = self.state();
        state.phase() == Phase::Ready && state.contacts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use crate::model::Phase;
    use crate::test_support::{fake_store, pump, raw_idle, with_context, FakeTuple};

    fn state_call() -> Vec<String> {
        ["state", "--raw", "--format", "json"].iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn polls_state_until_contacts_arrive_after_daemon_boot() {
        with_context(|context| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let fake = FakeTuple::new();
            // The daemon's first state after boot: signed in, contacts not loaded yet.
            let mut booting = raw_idle();
            booting["contacts"] = serde_json::json!([]);
            fake.set_follow_state(&booting);
            let (store, _results) = fake_store(&fake, &runtime);
            store.start();
            assert!(pump(context, || store.state().phase() == Phase::Ready));
            assert!(store.state().contacts.is_empty());
            assert!(pump(context, || store.state().contacts.len() == 24), "contacts never refreshed");
            assert!(fake.calls().contains(&state_call()));
        });
    }

    #[test]
    fn does_not_poll_when_contacts_are_present() {
        with_context(|context| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let fake = FakeTuple::new();
            let (store, _results) = fake_store(&fake, &runtime);
            store.start();
            assert!(pump(context, || store.state().contacts.len() == 24));
            let settled = std::time::Instant::now() + std::time::Duration::from_millis(1500);
            pump(context, || std::time::Instant::now() > settled);
            assert!(!fake.calls().contains(&state_call()));
        });
    }
}
