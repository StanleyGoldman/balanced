//! Keyboard navigation of the Menu bar. egui's menus have none, so it is built on top
//! of them. F10 or a lone Alt opens the first menu and puts focus on its first enabled item. While a
//! menu is open, ← and → open the menu beside it, and ↑ and ↓ move focus through its
//! enabled items, all wrapping around.

use eframe::egui::{self, Id, Key, Modifiers, Popup, Response};

/// What the Menu bar's keyboard navigation remembers between frames.
#[derive(Debug, Default)]
pub(crate) struct MenuNav {
    /// The popup ids of the Menu bar's menus, left to right, as last drawn.
    menus: Vec<Id>,
    /// The open menu's enabled items, top to bottom, as last drawn.
    items: Vec<Id>,
    /// Arrow key presses taken from egui's focus navigation while a menu was open, to
    /// act on in the next frame.
    arrows: Vec<Arrow>,
    /// The modifiers held at the end of the last frame.
    held: Modifiers,
    /// Whether Alt went down on its own with nothing pressed since.
    lone_alt: bool,
    /// Whether the menu the keyboard opened still has to put focus on its first item.
    focus_first: bool,
}

/// One frame of the Menu bar's drawing: its menus and the open menu's enabled items.
#[derive(Debug, Default)]
pub(crate) struct MenuFrame {
    menus: Vec<Id>,
    items: Vec<Id>,
}

impl MenuFrame {
    /// Notes a menu's button, left to right.
    pub(crate) fn note_menu(&mut self, button: &Response) {
        self.menus.push(Popup::default_response_id(button));
    }

    /// Notes an item of the open menu, top to bottom. Disabled items can't take focus.
    pub(crate) fn note_item(&mut self, item: &Response) {
        if item.enabled() {
            self.items.push(item.id);
        }
    }
}

impl MenuNav {
    /// Takes the arrow key presses out of `raw_input` while a menu is open, before
    /// egui's focus navigation moves focus out of the menu with them.
    pub(crate) fn take_arrows(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if self.open_menu(ctx).is_none() {
            return;
        }
        raw_input.events.retain(|event| {
            let egui::Event::Key {
                key,
                pressed,
                modifiers,
                ..
            } = event
            else {
                return true;
            };
            let Some(arrow) = Arrow::of(*key).filter(|_| modifiers.is_none()) else {
                return true;
            };
            if *pressed {
                self.arrows.push(arrow);
            }
            false
        });
    }

    /// The index of the open menu, if one is.
    fn open_menu(&self, ctx: &egui::Context) -> Option<usize> {
        self.menus.iter().position(|&id| Popup::is_id_open(ctx, id))
    }

    /// Acts on this frame's menu keys, before the Menu bar is drawn.
    pub(crate) fn keys(&mut self, ctx: &egui::Context) {
        let f10 = ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::F10));
        let alt_released = ctx.input(|input| {
            input
                .events
                .iter()
                .any(|event| self.alt_released_alone(event))
        });
        if f10 || alt_released {
            self.open(ctx, 0);
        }
        let arrows = std::mem::take(&mut self.arrows);
        for arrow in arrows {
            let Some(menu) = self.open_menu(ctx) else {
                return;
            };
            match arrow {
                Arrow::Left => self.open(ctx, wrap(menu, -1, self.menus.len())),
                Arrow::Right => self.open(ctx, wrap(menu, 1, self.menus.len())),
                Arrow::Up => self.move_focus(ctx, -1),
                Arrow::Down => self.move_focus(ctx, 1),
            }
        }
    }

    /// Follows Alt through `event`, and returns whether the event released an Alt that
    /// was pressed and released with nothing between: no key, click or other modifier.
    fn alt_released_alone(&mut self, event: &egui::Event) -> bool {
        let mut released = false;
        if let egui::Event::ModifiersChanged(modifiers) = *event {
            released = self.lone_alt && modifiers.is_none();
            self.lone_alt = modifiers == Modifiers::ALT && self.held.is_none();
            self.held = modifiers;
        } else if matches!(
            event,
            egui::Event::Key { pressed: true, .. }
                | egui::Event::PointerButton { pressed: true, .. }
        ) {
            self.lone_alt = false;
        }
        released
    }

    /// Opens the menu at `index`, closing any other, with focus on its first item.
    fn open(&mut self, ctx: &egui::Context, index: usize) {
        if let Some(&menu) = self.menus.get(index) {
            Popup::open_id(ctx, menu);
            self.items.clear();
            self.focus_first = true;
        }
    }

    /// Moves focus `step` items through the open menu, wrapping around. With no item
    /// focused, ↓ focuses the first item and ↑ the last.
    fn move_focus(&self, ctx: &egui::Context, step: isize) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        let focused = ctx.memory(egui::Memory::focused);
        let index = match self.items.iter().position(|&id| Some(id) == focused) {
            Some(index) => wrap(index, step, len),
            None if step < 0 => len - 1,
            None => 0,
        };
        let id = self.items[index];
        ctx.memory_mut(|memory| memory.request_focus(id));
    }

    /// Takes in what `frame` drew of the Menu bar, after it is drawn.
    pub(crate) fn drawn(&mut self, ctx: &egui::Context, frame: MenuFrame) {
        if self.focus_first
            && let Some(&first) = frame.items.first()
        {
            ctx.memory_mut(|memory| memory.request_focus(first));
            self.focus_first = false;
        }
        self.menus = frame.menus;
        self.items = frame.items;
    }
}

/// An arrow key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arrow {
    Up,
    Down,
    Left,
    Right,
}

impl Arrow {
    /// The arrow `key` is, if it is one.
    fn of(key: Key) -> Option<Self> {
        #[expect(clippy::wildcard_enum_match_arm, reason = "foreign enum")]
        match key {
            Key::ArrowUp => Some(Self::Up),
            Key::ArrowDown => Some(Self::Down),
            Key::ArrowLeft => Some(Self::Left),
            Key::ArrowRight => Some(Self::Right),
            _ => None,
        }
    }
}

/// `index` moved `step` places through `len` places, wrapping around at both ends.
fn wrap(index: usize, step: isize, len: usize) -> usize {
    (index + len).wrapping_add_signed(step) % len
}
