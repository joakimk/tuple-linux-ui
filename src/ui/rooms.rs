//! Rooms tab: the pinned personal room card, then team rooms with favorites first.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;

use super::{avatar, clear, confirm, icon_button, padded_column, prompt, scrolled, section};
use crate::avatars::AvatarLoader;
use crate::model::{Room, RoomKind};
use crate::store::Store;
use crate::viewmodel::occupancy_text;

pub struct RoomsView {
    pub root: gtk::Box,
    list: gtk::Box,
    store: Store,
    loader: AvatarLoader,
    rendered: RefCell<Option<(Option<Room>, Vec<Room>, Option<String>)>>,
}

impl RoomsView {
    pub fn new(store: Store, loader: AvatarLoader) -> Rc<RoomsView> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        top.set_margin_top(8);
        top.set_margin_start(12);
        top.set_margin_end(12);
        let title = gtk::Label::new(Some("Rooms"));
        title.add_css_class("title-3");
        title.set_xalign(0.0);
        title.set_hexpand(true);
        top.append(&title);
        let create = icon_button("list-add-symbolic", "Create a team room");
        top.append(&create);
        root.append(&top);
        let list = padded_column();
        root.append(&scrolled(&list));

        let view = Rc::new(RoomsView { root, list, store, loader, rendered: RefCell::new(None) });
        create.connect_clicked({
            let view = view.clone();
            move |button| {
                let store = view.store.clone();
                prompt(button, "Create room", "Room name", "Create", move |name| store.create_room(&name));
            }
        });
        view.store.subscribe({
            let view = view.clone();
            move |_| view.render()
        });
        view.render();
        view
    }

    fn current_room_slug(&self) -> Option<String> {
        self.store.state().call.as_ref().and_then(|c| c.room.as_ref()).map(|r| r.slug.clone())
    }

    fn render(&self) {
        let state = self.store.state();
        let current = self.current_room_slug();
        let key = (state.personal_room.clone(), state.team_rooms.clone(), current.clone());
        if self.rendered.borrow().as_ref() == Some(&key) {
            return;
        }
        *self.rendered.borrow_mut() = Some(key);
        clear(&self.list);
        if let Some(personal) = &state.personal_room {
            let (container, list) = section("");
            list.append(&self.row(personal, current.as_deref()));
            self.list.append(&container);
        }
        let rooms = state.sorted_team_rooms();
        if rooms.is_empty() {
            let label = gtk::Label::new(Some("No team rooms yet. Create one with +."));
            label.add_css_class("dim-label");
            label.set_margin_top(24);
            self.list.append(&label);
            return;
        }
        let (container, list) = section("Team rooms");
        for room in rooms {
            list.append(&self.row(room, current.as_deref()));
        }
        self.list.append(&container);
    }

    fn row(&self, room: &Room, current: Option<&str>) -> adw::ActionRow {
        let row = adw::ActionRow::new();
        row.set_title(&glib::markup_escape_text(&room.display_name()));
        row.set_subtitle(&occupancy_text(room));
        row.set_tooltip_text(Some(&room.url));
        if room.kind == RoomKind::Personal {
            row.add_prefix(&gtk::Image::from_icon_name("user-home-symbolic"));
        }
        if !room.members.is_empty() {
            let avatars = gtk::Box::new(gtk::Orientation::Horizontal, 2);
            avatars.set_valign(gtk::Align::Center);
            for member in room.members.iter().take(4) {
                avatars.append(&avatar(member, 24, None, &self.loader));
            }
            row.add_suffix(&avatars);
        }
        let copy = icon_button("edit-copy-symbolic", "Copy room link");
        copy.connect_clicked({
            let url = room.url.clone();
            move |button| button.clipboard().set_text(&url)
        });
        row.add_suffix(&copy);
        if room.kind == RoomKind::Team {
            let star = icon_button(
                if room.favorited { "starred-symbolic" } else { "non-starred-symbolic" },
                if room.favorited { "Unfavorite room" } else { "Favorite room" },
            );
            star.connect_clicked({
                let store = self.store.clone();
                let slug = room.slug.clone();
                let favorited = room.favorited;
                move |_| store.favorite_room(&slug, !favorited)
            });
            row.add_suffix(&star);
        }
        let in_this_room = current == Some(room.slug.as_str());
        let button = gtk::Button::with_label(if in_this_room { "Leave" } else { "Join" });
        button.set_valign(gtk::Align::Center);
        if in_this_room {
            button.add_css_class("destructive-action");
            button.connect_clicked({
                let store = self.store.clone();
                move |_| store.leave("current")
            });
        } else {
            if room.occupied() {
                button.add_css_class("suggested-action");
            }
            button.connect_clicked({
                let store = self.store.clone();
                let room = room.clone();
                move |button| join_room(&store, &room, button)
            });
        }
        row.add_suffix(&button);
        row
    }
}

/// Join a room; confirm first when it means leaving another call. The personal
/// room also copies its link, like the Mac card does.
pub fn join_room(store: &Store, room: &Room, parent: &impl IsA<gtk::Widget>) {
    let in_other_call = {
        let state = store.state();
        state.in_call() && state.call.as_ref().and_then(|c| c.room.as_ref()).map(|r| r.slug.as_str()) != Some(room.slug.as_str())
    };
    if room.kind == RoomKind::Personal {
        parent.clipboard().set_text(&room.url);
    }
    if in_other_call {
        let store = store.clone();
        let slug = room.slug.clone();
        confirm(parent, "Switch rooms?", &format!("You're already in a call. Leave it and join {}?", room.display_name()), "Switch", move || {
            store.join_room(&slug, true)
        });
    } else {
        store.join_room(&room.slug, false);
    }
}
