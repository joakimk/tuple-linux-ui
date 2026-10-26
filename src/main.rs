//! tuple-ui: a native Linux client for Tuple pair programming, built on the `tuple` CLI.

mod autostart;
mod avatars;
mod instance;
mod model;
mod notify;
mod prefs;
mod store;
#[cfg(test)]
mod test_support;
mod tray;
mod tuple;
mod ui;
mod viewmodel;

use std::cell::RefCell;
use std::rc::Rc;

use clap::Parser;
use adw::prelude::*;

use crate::avatars::AvatarLoader;
use crate::prefs::Prefs;
use crate::store::Store;
use crate::tray::{TrayCommand, TraySnapshot};
use crate::tuple::TupleCli;
use crate::ui::window::MainWindow;

pub const APP_ID: &str = "dev.joakimk.TupleUi";

#[derive(Parser, Debug)]
#[command(name = "tuple-ui", version, about = "Tuple desktop client for Linux")]
struct Args {
    /// Start hidden in the tray
    #[arg(long)]
    tray: bool,
    /// Show the window of the running instance (or start one)
    #[arg(long)]
    show: bool,
    /// Show or hide the window of the running instance
    #[arg(long = "toggle-window")]
    toggle_window: bool,
    /// Mute or unmute the current call in the running instance
    #[arg(long = "toggle-mute")]
    toggle_mute: bool,
    /// Quit the running instance
    #[arg(long)]
    quit: bool,
}

fn main() -> glib::ExitCode {
    let args = Args::parse();
    let socket = instance::default_socket_path();
    let command = [(args.show, "show"), (args.toggle_window, "toggle-window"), (args.toggle_mute, "toggle-mute"), (args.quit, "quit")]
        .into_iter()
        .find(|(set, _)| *set)
        .map(|(_, name)| name);
    if let Some(command) = command {
        if instance::send_to_running_instance(command, &socket) {
            return glib::ExitCode::SUCCESS;
        }
        if command == "toggle-mute" || command == "quit" {
            eprintln!("tuple-ui is not running");
            return glib::ExitCode::FAILURE;
        }
        // --show / --toggle-window without a running instance: just launch.
    }

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let handle = runtime.handle().clone();
    let start_hidden = args.tray;

    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| activate(app, handle.clone(), start_hidden));
    // GTK parses its own flags; we already handled ours.
    app.run_with_args::<String>(&[])
}

fn activate(app: &adw::Application, runtime: tokio::runtime::Handle, start_hidden: bool) {
    if let Some(window) = app.active_window() {
        window.present();
        return;
    }
    ui::install_css();
    let prefs = Rc::new(RefCell::new(Prefs::load()));
    let (store, results) = Store::new(TupleCli::default(), runtime.clone());
    let loader = AvatarLoader::new(runtime.clone());
    let main = MainWindow::new(app, store.clone(), prefs.clone(), loader);
    let hold = app.hold();
    std::mem::forget(hold); // keep running while the window is hidden in the tray

    wire_actions(app, &main, &store);
    wire_results(&main, results);
    wire_notifications(&main, &store, &prefs);
    wire_tray(app, &main, &store);
    wire_instance_socket(app, &main, &store);

    store.start();
    let show = !start_hidden && !prefs.borrow().start_in_tray && prefs.borrow().show_window_on_start;
    if show {
        main.present();
    }
    // Development aid: TUPLE_UI_OPEN=settings|shortcuts opens that dialog right away.
    match std::env::var("TUPLE_UI_OPEN").as_deref() {
        Ok("settings") => main.open_settings(),
        Ok("shortcuts") => main.open_shortcuts(),
        _ => {}
    }
}

fn wire_actions(app: &adw::Application, main: &Rc<MainWindow>, store: &Store) {
    let tab = gio::SimpleAction::new("tab", Some(glib::VariantTy::STRING));
    tab.connect_activate({
        let main = main.clone();
        move |_, parameter| {
            if let Some(name) = parameter.and_then(|p| p.get::<String>()) {
                main.show_tab(&name);
            }
        }
    });
    app.add_action(&tab);
    let simple: Vec<(&str, Box<dyn Fn()>)> = vec![
        ("search", Box::new({ let main = main.clone(); move || main.focus_search() })),
        ("mute", Box::new({ let store = store.clone(); move || store.toggle_mute() })),
        ("share", Box::new({ let store = store.clone(); move || store.toggle_share() })),
        ("capture", Box::new({ let store = store.clone(); move || store.toggle_capture() })),
        ("leave", Box::new({ let store = store.clone(); move || if store.state().in_call() { store.leave("current") } })),
        ("settings", Box::new({ let main = main.clone(); move || main.open_settings() })),
        ("shortcuts", Box::new({ let main = main.clone(); move || main.open_shortcuts() })),
        ("copy-link", Box::new({ let main = main.clone(); move || main.copy_personal_link() })),
        ("sign-out", Box::new({ let main = main.clone(); move || main.sign_out() })),
        ("hide", Box::new({ let main = main.clone(); move || main.window.set_visible(false) })),
        ("quit", Box::new({ let app = app.clone(); move || app.quit() })),
    ];
    for (name, handler) in simple {
        let action = gio::SimpleAction::new(name, None);
        action.connect_activate(move |_, _| handler());
        app.add_action(&action);
    }
    for (action, accel, _label) in ui::shortcuts::SHORTCUTS {
        app.set_accels_for_action(action, &[accel]);
    }
}

fn wire_results(main: &Rc<MainWindow>, results: async_channel::Receiver<store::ActionResult>) {
    let main = main.clone();
    glib::spawn_future_local(async move {
        while let Ok(result) = results.recv().await {
            main.on_result(&result);
            if !result.result.ok() && !main.window.is_visible() {
                notify::Notifier::notice(&format!("Tuple: {} failed", result.label.replace('_', " ")), &result.result.error());
            }
        }
    });
}

fn wire_notifications(main: &Rc<MainWindow>, store: &Store, prefs: &Rc<RefCell<Prefs>>) {
    let (notifier, actions) = notify::Notifier::new();
    let notifier = Rc::new(RefCell::new(notifier));
    store.subscribe({
        let prefs = prefs.clone();
        move |state| notifier.borrow_mut().on_state(state, &prefs.borrow())
    });
    let store = store.clone();
    let main = main.clone();
    glib::spawn_future_local(async move {
        while let Ok(action) = actions.recv().await {
            // A notification may outlive its invite; only act on calls still ringing.
            let still_ringing = |id: &str| store.state().invites.iter().any(|i| viewmodel::invite_call_id(i) == id);
            match action {
                notify::NotificationAction::Accept(id) if still_ringing(&id) => {
                    store.accept(&id);
                    main.present();
                }
                notify::NotificationAction::Decline(id) if still_ringing(&id) => store.leave(&id),
                notify::NotificationAction::Accept(_) | notify::NotificationAction::Decline(_) => {}
                notify::NotificationAction::CallBack(target) => store.call_person(&target),
            }
        }
    });
}

fn wire_tray(app: &adw::Application, main: &Rc<MainWindow>, store: &Store) {
    let (commands, receiver) = async_channel::unbounded::<TrayCommand>();
    let (ready, ready_rx) = async_channel::bounded::<ksni::Handle<tray::TupleTray>>(1);
    store.runtime().spawn(async move {
        match tray::spawn(commands).await {
            Ok(handle) => {
                let _ = ready.send(handle).await;
            }
            Err(err) => eprintln!("tuple-ui: no system tray available ({err})"),
        }
    });
    {
        let store = store.clone();
        glib::spawn_future_local(async move {
            let Ok(handle) = ready_rx.recv().await else { return };
            let runtime = store.runtime().clone();
            let push = move |state: &model::AppState| {
                let snapshot = TraySnapshot::from_state(state);
                let handle = handle.clone();
                runtime.spawn(async move {
                    handle.update(|tray| tray.snapshot = snapshot).await;
                });
            };
            push(&store.state());
            store.subscribe(push);
        });
    }
    let main = main.clone();
    let store = store.clone();
    let app = app.clone();
    glib::spawn_future_local(async move {
        while let Ok(command) = receiver.recv().await {
            match command {
                TrayCommand::ToggleWindow => main.toggle(),
                TrayCommand::ToggleMute => store.toggle_mute(),
                TrayCommand::ToggleShare => store.toggle_share(),
                TrayCommand::Leave => store.leave("current"),
                TrayCommand::CallPerson(email) => store.call_person(&email),
                TrayCommand::JoinRoom(slug) => store.join_room(&slug, false),
                TrayCommand::Settings => {
                    main.present();
                    main.open_settings();
                }
                TrayCommand::Quit => app.quit(),
            }
        }
    });
}

fn wire_instance_socket(app: &adw::Application, main: &Rc<MainWindow>, store: &Store) {
    let commands = instance::listen(instance::default_socket_path(), store.runtime());
    let main = main.clone();
    let store = store.clone();
    let app = app.clone();
    glib::spawn_future_local(async move {
        while let Ok(command) = commands.recv().await {
            match command.as_str() {
                "show" => main.present(),
                "toggle-window" => main.toggle(),
                "toggle-mute" => store.toggle_mute(),
                "quit" => app.quit(),
                other => eprintln!("tuple-ui: unknown command {other}"),
            }
        }
    });
}
