//! The Register tabs: the Registers open under the Accounts tab, each with its own
//! state, and which of them is current.

use balanced_core::{AccountId, LedgerView};
use balanced_i18n::Localizer;
use eframe::egui::accesskit::Role;
use eframe::egui::{
    self, Rect, Sense, TextStyle, TextWrapMode, WidgetInfo, WidgetText, WidgetType, pos2, vec2,
};

use crate::i18n::fl;
use crate::register::Register;
use crate::route::OpenIn;

/// One open Register and the account it shows.
#[derive(Debug)]
struct RegisterTab {
    account: AccountId,
    register: Register,
}

impl RegisterTab {
    fn new(account: AccountId) -> Self {
        Self {
            account,
            register: Register::default(),
        }
    }
}

/// The open Register tabs, left to right, and the current one. None of it outlives the
/// run: every run starts with no tab open.
#[derive(Debug, Default)]
pub(crate) struct RegisterTabs {
    tabs: Vec<RegisterTab>,
    /// The index of the current tab. It is below `tabs.len()` whenever a tab is open.
    current: usize,
}

impl RegisterTabs {
    /// Opens `account`'s Register where `open_in` says and makes it current. An account
    /// that already has a tab doesn't get a second: its tab becomes current.
    pub(crate) fn open(&mut self, account: AccountId, open_in: OpenIn) {
        if let Some(index) = self.tabs.iter().position(|tab| tab.account == account) {
            self.current = index;
            return;
        }
        match (open_in, self.tabs.get_mut(self.current)) {
            (OpenIn::CurrentRegister, Some(tab)) => *tab = RegisterTab::new(account),
            (OpenIn::CurrentRegister, None) | (OpenIn::NewTab, _) => {
                self.tabs.push(RegisterTab::new(account));
                self.current = self.tabs.len() - 1;
            }
        }
    }

    /// Makes the tab at `index` current. An index past the last tab does nothing.
    fn select(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.current = index;
        }
    }

    /// Closes the tab at `index`. The tab to its right becomes current if it was, or
    /// the one to its left when it was the last.
    fn close(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        if index < self.current || self.current == self.tabs.len() {
            self.current = self.current.saturating_sub(1);
        }
    }

    /// Closes the current tab.
    pub(crate) fn close_current(&mut self) {
        self.close(self.current);
    }

    /// Makes the tab right of the current one current, wrapping to the first.
    pub(crate) fn next(&mut self) {
        if !self.tabs.is_empty() {
            self.current = (self.current + 1) % self.tabs.len();
        }
    }

    /// Makes the tab left of the current one current, wrapping to the last.
    pub(crate) fn previous(&mut self) {
        if !self.tabs.is_empty() {
            self.current = (self.current + self.tabs.len() - 1) % self.tabs.len();
        }
    }

    /// How many tabs are open.
    pub(crate) fn len(&self) -> usize {
        self.tabs.len()
    }

    /// The account in the current tab, or `None` while no tab is open.
    pub(crate) fn current_account(&self) -> Option<AccountId> {
        self.tabs.get(self.current).map(|tab| tab.account)
    }

    /// The current tab's Register, or `None` while no tab is open.
    pub(crate) fn current_register(&self) -> Option<&Register> {
        self.tabs.get(self.current).map(|tab| &tab.register)
    }

    /// The current tab's Register, or `None` while no tab is open.
    pub(crate) fn current_register_mut(&mut self) -> Option<&mut Register> {
        self.tabs.get_mut(self.current).map(|tab| &mut tab.register)
    }

    /// Draws the tab strip, one tab per open Register labelled with its account's name
    /// in `view`, and reports it to AccessKit as a tab list. A tab whose account is gone
    /// from `view` is left out. Clicking a tab makes it current, and clicking its ×
    /// closes it.
    pub(crate) fn show_strip(&mut self, ui: &mut egui::Ui, i18n: &Localizer, view: &LedgerView) {
        let mut select = None;
        let mut close = None;
        ui.horizontal(|ui| {
            ui.ctx()
                .accesskit_node_builder(ui.unique_id(), |node| node.set_role(Role::TabList));
            for (index, tab) in self.tabs.iter().enumerate() {
                let Some(summary) = view.account(tab.account) else {
                    continue;
                };
                let state = if index == self.current {
                    TabState::Current
                } else {
                    TabState::Other
                };
                let name = summary.account().name();
                match tab_widget(ui, i18n, tab.account, name, state) {
                    Some(TabPart::Tab) => select = Some(index),
                    Some(TabPart::Close) => close = Some(index),
                    None => {}
                }
            }
        });
        if let Some(index) = select {
            self.select(index);
        }
        if let Some(index) = close {
            self.close(index);
        }
    }
}

/// Whether a Register tab is the current one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TabState {
    Current,
    Other,
}

/// The part of a Register tab that was clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TabPart {
    Tab,
    Close,
}

/// One Register tab: the account's name, then a × that closes the tab. Returns the
/// part clicked this frame. The × is painted as two strokes.
fn tab_widget(
    ui: &mut egui::Ui,
    i18n: &Localizer,
    account: AccountId,
    name: &str,
    state: TabState,
) -> Option<TabPart> {
    let current = state == TabState::Current;
    let padding = ui.spacing().button_padding;
    let height = ui.spacing().interact_size.y;
    let close_size = height - padding.y * 2.0;
    let galley = WidgetText::from(name).into_galley(
        ui,
        Some(TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Body,
    );
    let width = padding.x + galley.size().x + padding.x + close_size + padding.y;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let tab = ui.interact(
        rect,
        ui.make_persistent_id(("register_tab", account)),
        Sense::click(),
    );
    let close_rect = Rect::from_min_size(
        pos2(
            rect.right() - padding.y - close_size,
            rect.top() + padding.y,
        ),
        vec2(close_size, close_size),
    );
    let close = ui.interact(
        close_rect,
        ui.make_persistent_id(("register_tab_close", account)),
        Sense::click(),
    );
    let enabled = ui.is_enabled();
    tab.widget_info(|| WidgetInfo::labeled(WidgetType::Other, enabled, name));
    ui.ctx().accesskit_node_builder(tab.id, |node| {
        node.set_role(Role::Tab);
        node.set_selected(current);
    });
    let close_label = fl!(i18n, "tab-register-close", account = name);
    close.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, &close_label));
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact_selectable(&tab, current);
        if current || tab.hovered() {
            ui.painter().rect_filled(rect, 0.0, visuals.weak_bg_fill);
        }
        let top = rect.center().y - galley.size().y / 2.0;
        ui.painter().galley(
            pos2(rect.left() + padding.x, top),
            galley,
            visuals.text_color(),
        );
        let close_visuals = ui.style().interact(&close);
        if close.hovered() || close.is_pointer_button_down_on() {
            ui.painter()
                .rect_filled(close_rect, 0.0, close_visuals.weak_bg_fill);
        }
        let cross = close_rect.shrink(close_size / 4.0);
        let stroke = visuals.fg_stroke;
        ui.painter()
            .line_segment([cross.left_top(), cross.right_bottom()], stroke);
        ui.painter()
            .line_segment([cross.right_top(), cross.left_bottom()], stroke);
    }
    if close.clicked() {
        Some(TabPart::Close)
    } else if tab.clicked() {
        Some(TabPart::Tab)
    } else {
        None
    }
}
