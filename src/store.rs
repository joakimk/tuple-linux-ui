//! The store owns the live `tuple state follow` stream and dispatches CLI actions.
//!
//! It lives on the GTK main thread (`Rc`), subscribers get the new [`AppState`]
//! after every change, and every action is a `tuple` invocation that runs on the
//! tokio runtime and reports back through `results`.

use std::cell::{Ref, RefCell};
use std::rc::Rc;

use async_channel::{Receiver, Sender};

use crate::model::{AppState, Phase};
use crate::tuple::{CliResult, FollowEvent, TupleCli};

pub type Listener = Box<dyn Fn(&AppState)>;

/// Outcome of one dispatched action: the label passed to `dispatch` and the CLI result.
#[derive(Debug, Clone)]
pub struct ActionResult {
    pub label: String,
    pub result: CliResult,
}

#[derive(Clone)]
pub struct Store {
    inner: Rc<RefCell<Inner>>,
    runtime: tokio::runtime::Handle,
}

struct Inner {
    cli: TupleCli,
    state: AppState,
    listeners: Vec<Listener>,
    results: Sender<ActionResult>,
}

impl Store {
    pub fn new(cli: TupleCli, runtime: tokio::runtime::Handle) -> (Store, Receiver<ActionResult>) {
        let (results, receiver) = async_channel::unbounded();
        let inner = Inner { cli, state: AppState::default(), listeners: Vec::new(), results };
        (Store { inner: Rc::new(RefCell::new(inner)), runtime }, receiver)
    }

    pub fn state(&self) -> Ref<'_, AppState> {
        Ref::map(self.inner.borrow(), |inner| &inner.state)
    }

    pub fn cli(&self) -> TupleCli {
        self.inner.borrow().cli.clone()
    }

    pub fn runtime(&self) -> &tokio::runtime::Handle {
        &self.runtime
    }

    pub fn subscribe(&self, listener: impl Fn(&AppState) + 'static) {
        self.inner.borrow_mut().listeners.push(Box::new(listener));
    }

    /// Start following daemon state. Events are applied on the calling (GTK) thread.
    pub fn start(&self) {
        let events = self.cli().follow(&self.runtime);
        let store = self.clone();
        glib::spawn_future_local(async move {
            while let Ok(event) = events.recv().await {
                store.apply(event);
            }
        });
    }

    pub fn apply(&self, event: FollowEvent) {
        let next = {
            let current = self.state();
            match event {
                FollowEvent::Event { kind, data } => current.apply_event(&kind, data),
                FollowEvent::Unavailable(message) => AppState::unavailable(message),
                FollowEvent::Ended { error, had_state } => {
                    if had_state || current.phase() == Phase::Ready {
                        AppState::connecting(error)
                    } else {
                        AppState::unavailable(error)
                    }
                }
            }
        };
        self.set_state(next);
    }

    pub fn set_state(&self, state: AppState) {
        self.inner.borrow_mut().state = state;
        let inner = self.inner.borrow();
        for listener in &inner.listeners {
            listener(&inner.state);
        }
    }

    /// Run `tuple <args>` asynchronously; the result arrives on the `results` channel.
    pub fn dispatch(&self, label: &str, args: Vec<String>) {
        let (cli, results) = {
            let inner = self.inner.borrow();
            (inner.cli.clone(), inner.results.clone())
        };
        let label = label.to_string();
        self.runtime.spawn(async move {
            let result = cli.run(args).await;
            let _ = results.send(ActionResult { label, result }).await;
        });
    }

    fn run(&self, label: &str, args: &[&str]) {
        self.dispatch(label, args.iter().map(|a| a.to_string()).collect());
    }

    // ------------------------------------------------------------- calls
    pub fn call_person(&self, person: &str) {
        self.run("call", &["call", "start", person, "--format", "json"]);
    }

    pub fn join(&self, target: &str, switch: bool) {
        let mut args = vec!["call", "join", target];
        if switch {
            args.push("--switch");
        }
        args.extend(["--format", "json"]);
        self.run("join", &args);
    }

    pub fn join_room(&self, slug: &str, switch: bool) {
        let mut args = vec!["rooms", "join", slug];
        if switch {
            args.push("--switch");
        }
        args.extend(["--format", "json"]);
        self.run("join_room", &args);
    }

    pub fn accept(&self, call_id: &str) {
        self.run("accept", &["call", "accept", "--call", call_id]);
    }

    pub fn leave(&self, call_id: &str) {
        self.run("leave", &["call", "leave", "--call", call_id]);
    }

    pub fn toggle_mute(&self) {
        let muted = match &self.state().call {
            Some(call) => call.muted,
            None => return,
        };
        let verb = if muted { "unmute" } else { "mute" };
        self.run(verb, &["call", verb, "--call", "current"]);
    }

    pub fn toggle_share(&self) {
        let sharing = match &self.state().call {
            Some(call) => call.sharing,
            None => return,
        };
        if sharing {
            self.run("stop_sharing", &["unshare"]);
        } else {
            self.run("share_screen", &["share"]);
        }
    }

    pub fn toggle_capture(&self) {
        let capturing = match &self.state().call {
            Some(call) => call.transcribing,
            None => return,
        };
        let verb = if capturing { "stop" } else { "start" };
        self.run("capture", &["capture", verb]);
    }

    pub fn react(&self, sound: &str) {
        self.run("react", &["call", "react", sound]);
    }

    pub fn add_participant(&self, person: &str) {
        self.run("add_participant", &["call", "participants", "add", person]);
    }

    pub fn remove_participant(&self, person: &str) {
        self.run("remove_participant", &["call", "participants", "remove", person]);
    }

    // ------------------------------------------------- contacts and rooms
    pub fn favorite_contact(&self, person: &str, favorited: bool) {
        let verb = if favorited { "favorite" } else { "unfavorite" };
        self.run("favorite_contact", &["contacts", verb, person]);
    }

    pub fn remove_contact(&self, person: &str) {
        self.run("remove_contact", &["contacts", "remove", person]);
    }

    pub fn invite(&self, email: &str) {
        self.run("invite", &["contacts", "invite", email]);
    }

    pub fn favorite_room(&self, slug: &str, favorited: bool) {
        let optimistic = self.state().with_room_favorited(slug, favorited);
        self.set_state(optimistic);
        let verb = if favorited { "favorite" } else { "unfavorite" };
        self.run("favorite_room", &["rooms", verb, slug]);
    }

    pub fn create_room(&self, name: &str) {
        self.run("create_room", &["rooms", "create", name, "--format", "json"]);
    }

    // ------------------------------------------------------------ session
    pub fn login(&self) {
        self.run("login", &["login"]);
    }

    pub fn auth(&self, code: &str) {
        self.run("auth", &["auth", code]);
    }

    pub fn logout(&self) {
        self.run("logout", &["logout"]);
    }

    pub fn start_daemon(&self) {
        self.run("start_daemon", &["start"]);
    }

    pub fn set_setting(&self, name: &str, value: &str) {
        self.dispatch(&format!("set:{name}"), vec!["set".into(), name.into(), value.into()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{fake_tuple_path, FakeTuple};
    use std::time::Duration;

    fn store_for(fake: &FakeTuple, runtime: &tokio::runtime::Runtime) -> (Store, Receiver<ActionResult>) {
        let cli = TupleCli { executable: fake_tuple_path().to_string_lossy().into_owned(), env: fake.env() };
        Store::new(cli, runtime.handle().clone())
    }

    fn pump(context: &glib::MainContext, until: impl Fn() -> bool) -> bool {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            context.iteration(false);
            if until() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    /// Each test gets its own main context so `glib::spawn_future_local` targets it.
    fn with_context(test: impl FnOnce(&glib::MainContext)) {
        let context = glib::MainContext::new();
        context.with_thread_default(|| test(&context)).unwrap();
    }

    #[test]
    fn follows_state_and_notifies_listeners() {
        with_context(|context| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let fake = FakeTuple::new();
            let (store, _results) = store_for(&fake, &runtime);
            let seen = Rc::new(RefCell::new(Vec::new()));
            let sink = seen.clone();
            store.subscribe(move |state| sink.borrow_mut().push(state.phase()));
            store.start();
            assert!(pump(context, || seen.borrow().last() == Some(&Phase::Ready)));
            assert_eq!(store.state().user.as_ref().unwrap().full_name, "Jo Example");

            fake.emit("current_call", serde_json::json!({"id": "abc", "local": {"audio_enabled": true, "connection_state": "connected"}}));
            assert!(pump(context, || store.state().call.is_some()));
            assert!(!store.state().call.as_ref().unwrap().muted);
            store.toggle_mute();
            assert!(pump(context, || fake.calls().contains(&vec!["call".to_string(), "mute".into(), "--call".into(), "current".into()])));
        });
    }

    #[test]
    fn dispatch_reports_results_and_failures_become_unavailable() {
        with_context(|context| {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let fake = FakeTuple::new();
            let (store, results) = store_for(&fake, &runtime);
            store.call_person("ada@example.com");
            let outcome = runtime.block_on(async { tokio::time::timeout(Duration::from_secs(5), results.recv()).await.unwrap().unwrap() });
            assert_eq!(outcome.label, "call");
            assert!(outcome.result.ok());
            assert!(fake.calls().contains(&vec!["call".to_string(), "start".into(), "ada@example.com".into(), "--format".into(), "json".into()]));

            fake.fail_with("dial unix: no such file");
            store.start();
            assert!(pump(context, || store.state().phase() == Phase::Unavailable));
            assert!(store.state().error.contains("no such file"));
        });
    }
}
