// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The modal helper: how every Dialog dims the Main window, takes Esc and Enter, keeps
//! inside the window and gives focus back when it closes.

use eframe::egui::{
    self, Align2, Area, Atom, Button, Color32, Frame, Id, Key, LayerId, Layout, Modal, Modifiers,
    Order, Popup, Rect, Response, ScrollArea, Sense, TextStyle, UiBuilder, Vec2, WidgetInfo,
    WidgetType, Window,
};

use crate::theme::Theme;

/// What became of a Dialog this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DialogOutcome {
    /// It stays open.
    Open,
    /// The user pressed its default button, or Enter.
    Accepted,
    /// The user pressed Back, which only a Dialog with steps has.
    Back,
    /// The user pressed Cancel or Esc, or clicked the dimmed window behind it.
    Cancelled,
}

/// An open Dialog's form state, with what the helper needs to know about it: whether
/// it has drawn yet, and which widget had keyboard focus when it opened. The host
/// window holds one as an `Option` that exists only while the Dialog is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenDialog<F> {
    pub(crate) form: F,
    /// The widget that gets keyboard focus back when the Dialog closes. A Dialog opened
    /// from a menu has none, because the click on the menu took focus away.
    return_focus: Option<Id>,
    /// Whether the Dialog has drawn a frame, after which its first focus is set.
    shown: bool,
}

impl<F> OpenDialog<F> {
    /// A Dialog holding `form`, opening now in `ctx`.
    pub(crate) fn new(ctx: &egui::Context, form: F) -> Self {
        Self {
            form,
            return_focus: ctx.memory(egui::Memory::focused),
            shown: false,
        }
    }

    /// Gives the default button keyboard focus again on the next frame, as when the
    /// Dialog opened.
    pub(crate) fn focus_default_button(&mut self) {
        self.shown = false;
    }

    /// Closes the Dialog, returning where keyboard focus goes. Restore it at the start
    /// of each frame until it is restored.
    #[must_use = "focus goes back only once the returned `FocusReturn` is restored"]
    pub(crate) fn close(self, ctx: &egui::Context) -> FocusReturn {
        // The next frame restores focus.
        ctx.request_repaint();
        FocusReturn(self.return_focus)
    }
}

/// Where keyboard focus goes once a Dialog has closed: to the widget that had it when
/// the Dialog opened, or nowhere.
///
/// egui keeps a closed modal's layer on top for one more frame, and takes focus from
/// any widget drawn below it. So focus waits until no modal layer is on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FocusReturn(Option<Id>);

impl FocusReturn {
    /// Restores focus, before any widget is drawn this frame. While a modal layer is
    /// still on top, it asks for another frame and returns itself, to try again then.
    #[must_use = "focus that can't be restored yet is returned to try again next frame"]
    pub(crate) fn restore(self, ctx: &egui::Context) -> Option<Self> {
        self.restore_under(ctx, None)
    }

    /// Restores focus to a widget in the heavy Dialog with id `heavy`, once a confirm
    /// opened over it has closed. Call it before the heavy Dialog's body is drawn. While
    /// another modal layer is on top of the heavy Dialog, it asks for another frame and
    /// returns itself, to try again then.
    #[must_use = "focus that can't be restored yet is returned to try again next frame"]
    pub(crate) fn restore_in(self, ctx: &egui::Context, heavy: Id) -> Option<Self> {
        self.restore_under(ctx, Some(heavy_backdrop(heavy)))
    }

    /// Restores focus once `layer`, or no layer, is the top modal layer.
    fn restore_under(self, ctx: &egui::Context, layer: Option<LayerId>) -> Option<Self> {
        if ctx.memory(egui::Memory::top_modal_layer) != layer {
            ctx.request_repaint();
            return Some(self);
        }
        // With no widget to go back to, focus stays nowhere: the click that opened
        // the Dialog from a menu, and the key or click that closed it, took it away.
        if let Some(id) = self.0 {
            ctx.memory_mut(|memory| memory.request_focus(id));
        }
        None
    }
}

/// A light Dialog: no title bar, a fixed width, its body above a row with its default
/// button and Cancel, and Back before them in a Dialog with steps. A notice has its
/// default button alone. The default button has first focus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LightDialog {
    id: Id,
    accept: String,
    /// Cancel's text, or `None` for a notice, which Esc still closes.
    cancel: Option<String>,
    back: Option<String>,
    backdrop: Color32,
}

impl LightDialog {
    /// A light Dialog with `id`, whose default button reads `accept` and whose other
    /// button reads `cancel`, dimming the window in `theme`'s backdrop.
    pub(crate) fn new(id: Id, accept: String, cancel: String, theme: &Theme) -> Self {
        Self {
            id,
            accept,
            cancel: Some(cancel),
            back: None,
            backdrop: theme.backdrop(),
        }
    }

    /// A notice with `id`, whose one button reads `accept`, dimming the window in
    /// `theme`'s backdrop.
    pub(crate) fn notice(id: Id, accept: String, theme: &Theme) -> Self {
        Self {
            id,
            accept,
            cancel: None,
            back: None,
            backdrop: theme.backdrop(),
        }
    }

    /// The same Dialog, with a Back button reading `back` left of its default button.
    pub(crate) fn with_back(self, back: String) -> Self {
        Self {
            back: Some(back),
            ..self
        }
    }

    /// Draws the Dialog over the dimmed Main window, with `body` drawn by the caller.
    /// The Dialog is never bigger than the window minus [`Theme::DIALOG_MARGIN`] on
    /// each side; a body too tall for that scrolls.
    pub(crate) fn show<F>(
        self,
        ctx: &egui::Context,
        dialog: &mut OpenDialog<F>,
        body: impl FnOnce(&mut egui::Ui, &mut F),
    ) -> DialogOutcome {
        let Self {
            id,
            accept,
            cancel,
            back,
            backdrop,
        } = self;
        let room =
            (ctx.content_rect().size() - Vec2::splat(2.0 * Theme::DIALOG_MARGIN)).max(Vec2::ZERO);
        let first_frame = !dialog.shown;
        dialog.shown = true;
        let frame = Frame::popup(&ctx.global_style());
        let modal = Modal::new(id)
            .frame(frame)
            .backdrop_color(backdrop)
            .show(ctx, |ui| {
                let inner = (room - frame.total_margin().sum()).max(Vec2::ZERO);
                let width = Theme::LIGHT_DIALOG_WIDTH.min(inner.x);
                ui.set_width(width);
                body_area(ui, inner.y, |ui| body(ui, &mut dialog.form));
                let (accept, cancel, back) = buttons(ui, accept, cancel, back);
                if first_frame {
                    accept.request_focus();
                }
                (accept, cancel, back)
            });
        let (accept, cancel, back) = &modal.inner;
        if accept.clicked()
            || (modal.is_top_modal && enter_pressed(ctx, cancel.as_ref(), back.as_ref()))
        {
            DialogOutcome::Accepted
        } else if back.as_ref().is_some_and(Response::clicked) {
            DialogOutcome::Back
        } else if cancel.as_ref().is_some_and(Response::clicked) || modal.should_close() {
            DialogOutcome::Cancelled
        } else {
            DialogOutcome::Open
        }
    }
}

/// A heavy Dialog: a title bar with ×, moved by its title bar and resized from its
/// edges, over the dimmed Main window. Its body sits above a row with its default
/// button and Cancel. It opens centred, and the default button has first focus
/// unless the body gives it to a field.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HeavyDialog {
    id: Id,
    title: String,
    accept: String,
    cancel: String,
    close: String,
    backdrop: Color32,
    size: HeavySize,
}

/// How big a heavy Dialog is.
#[derive(Debug, Clone, Copy, PartialEq)]
enum HeavySize {
    /// It opens [`Theme::HEAVY_DIALOG_WIDTH`] wide and as tall as its body.
    Default,
    /// It opens at this size.
    Opens(Vec2),
    /// It keeps its body this wide, or as wide as its title, and fits around it. It
    /// doesn't resize, and its body doesn't scroll.
    Fitted(f32),
}

impl HeavySize {
    /// How wide a fitted Dialog's body is: as asked, or `title_row` when the title is
    /// wider, and never wider than `room`. `None` for a Dialog that isn't fitted.
    fn fitted_width(self, title_row: f32, room: f32) -> Option<f32> {
        match self {
            Self::Fitted(width) => Some(width.max(title_row).min(room)),
            Self::Default | Self::Opens(_) => None,
        }
    }
}

impl HeavyDialog {
    /// A heavy Dialog with `id`, titled `title`, whose default button reads `accept`,
    /// whose other button reads `cancel` and whose × a screen reader reads as `close`,
    /// dimming the window in `theme`'s backdrop.
    pub(crate) fn new(
        id: Id,
        title: String,
        accept: String,
        cancel: String,
        close: String,
        theme: &Theme,
    ) -> Self {
        Self {
            id,
            title,
            accept,
            cancel,
            close,
            backdrop: theme.backdrop(),
            size: HeavySize::Default,
        }
    }

    /// The same Dialog, opening at `size` where the window has room.
    pub(crate) fn with_size(self, size: Vec2) -> Self {
        Self {
            size: HeavySize::Opens(size),
            ..self
        }
    }

    /// The same Dialog, fitted around a body `width` wide, or as wide as its title,
    /// where the window has room. It doesn't resize, and its body doesn't scroll, so
    /// keep the body small.
    pub(crate) fn fitted(self, width: f32) -> Self {
        Self {
            size: HeavySize::Fitted(width),
            ..self
        }
    }

    /// Draws the Dialog over the dimmed Main window, with `body` drawn by the caller.
    /// The Dialog moves and resizes only within the window minus
    /// [`Theme::DIALOG_MARGIN`] on each side; a body too tall for it scrolls.
    pub(crate) fn show<F>(
        self,
        ctx: &egui::Context,
        dialog: &mut OpenDialog<F>,
        body: impl FnOnce(&mut egui::Ui, &mut F),
    ) -> DialogOutcome {
        let Self {
            id,
            title,
            accept,
            cancel,
            close,
            backdrop: tint,
            size,
        } = self;
        let window = ctx.content_rect();
        let room = window.shrink(Theme::DIALOG_MARGIN);
        let first_frame = !dialog.shown;
        dialog.shown = true;
        // Windows' order: above the Main window's panels, and below popups and light
        // Dialogs, so a list or confirm the Dialog opens draws over it.
        let backdrop_layer = heavy_backdrop(id);
        let dialog_layer = LayerId::new(Order::Middle, id);
        // Widgets below the backdrop take no input or focus, as below an `egui::Modal`.
        let is_top = ctx.memory_mut(|memory| {
            memory.set_modal_layer(backdrop_layer);
            memory.top_modal_layer() == Some(backdrop_layer)
        });
        let popup_open = Popup::is_any_open(ctx);
        let backdrop = Area::new(backdrop_layer.id)
            .order(Order::Middle)
            .fixed_pos(window.min)
            .show(ctx, |ui| {
                ui.painter().rect_filled(window, 0.0, tint);
                ui.allocate_rect(window, Sense::click())
            })
            .inner;
        // The Dialog draws its own ×, labelled through `fl!`, where egui would draw
        // one. A gap as wide as egui's × follows the title, which keeps the title where
        // egui centres it beside its ×. The × sits in a slot as wide as that gap and as
        // tall as the title row.
        let style = ctx.global_style();
        let heading = TextStyle::Heading.resolve(&style);
        let (gap, close_slot, title_width) = ctx.fonts_mut(|fonts| {
            let gap = Vec2::splat(fonts.row_height(&heading));
            let title = fonts.layout_no_wrap(title.clone(), heading, Color32::PLACEHOLDER);
            let slot = Vec2::new(gap.x, title.size().y.max(gap.y));
            (gap, slot, title.size().x)
        });
        // The title, the gap after it and the ×, so a fitted Dialog shows its whole
        // title.
        let title_row = title_width + gap.x + style.spacing.item_spacing.x + close_slot.x;
        let mut heavy = Window::new((title, Atom::custom(id.with("close_gap"), gap)))
            .id(id)
            .order(Order::Middle)
            .collapsible(false)
            .resizable(true)
            .pivot(Align2::CENTER_CENTER)
            .max_size(room.size())
            .constrain_to(room);
        heavy = match size {
            HeavySize::Default => heavy.default_width(Theme::HEAVY_DIALOG_WIDTH.min(room.width())),
            HeavySize::Opens(size) => heavy.default_size(size.min(room.size())),
            HeavySize::Fitted(_) => heavy.auto_sized(),
        };
        if first_frame {
            heavy = heavy.current_pos(window.center());
        }
        let focused_before = ctx.memory(egui::Memory::focused);
        let shown = heavy.show(ctx, |ui| {
            let fitted_width = size.fitted_width(title_row, room.width());
            let (accept, cancel, _) = heavy_contents(ui, fitted_width, accept, cancel, |ui| {
                body(ui, &mut dialog.form);
            });
            let cancel = cancel.expect("a heavy Dialog has Cancel");
            let body_took_focus = ctx.memory(egui::Memory::focused) != focused_before;
            if first_frame && !body_took_focus {
                accept.request_focus();
            }
            (accept, cancel)
        });
        // Above the backdrop, which is above the Main window.
        ctx.move_to_top(backdrop_layer);
        ctx.move_to_top(dialog_layer);
        let Some(shown) = shown else {
            return DialogOutcome::Open;
        };
        let closed = close_button(
            ctx,
            id,
            dialog_layer,
            shown.response.rect,
            close_slot,
            &close,
        )
        .clicked();
        let Some((accept, cancel)) = shown.inner else {
            return DialogOutcome::Open;
        };
        // Esc and Enter belong to the top Dialog, such as a confirm opened over this one.
        let escape = || {
            is_top
                && !popup_open
                && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape))
        };
        if accept.clicked() || (is_top && enter_pressed(ctx, Some(&cancel), None)) {
            DialogOutcome::Accepted
        } else if cancel.clicked() || closed || backdrop.clicked() || escape() {
            DialogOutcome::Cancelled
        } else {
            DialogOutcome::Open
        }
    }
}

/// The × at the right of the title bar of the heavy Dialog with id `heavy`, drawn on
/// its `layer` where egui draws its own: centred in a `slot` at the top right corner
/// inside the frame of the window at `window`. A screen reader reads it as `label`.
fn close_button(
    ctx: &egui::Context,
    heavy: Id,
    layer: LayerId,
    window: Rect,
    slot: Vec2,
    label: &str,
) -> Response {
    let style = ctx.global_style();
    let frame = Frame::window(&style);
    let inset = frame.stroke.width;
    let corner = egui::pos2(
        window.right() - inset - f32::from(frame.inner_margin.right),
        window.top() + inset + f32::from(frame.inner_margin.top),
    );
    let slot = Rect::from_min_size(corner - Vec2::new(slot.x, 0.0), slot);
    let rect = Rect::from_center_size(slot.center(), Vec2::splat(style.spacing.icon_width));
    let ui = egui::Ui::new(
        ctx.clone(),
        heavy.with("close_ui"),
        UiBuilder::new()
            .layer_id(layer)
            .max_rect(rect)
            .accessibility_parent(heavy),
    );
    // It takes drags as well as clicks, so a drag that starts on it doesn't move the
    // Dialog by the title bar beneath it.
    let response = ui.interact(rect, heavy.with("close"), Sense::click_and_drag());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label));
    // Two strokes in the widget's foreground colour, as egui paints its ×.
    let visuals = ui.style().interact(&response);
    let cross = rect.shrink(2.0).expand(visuals.expansion);
    let stroke = visuals.fg_stroke;
    ui.painter()
        .line_segment([cross.left_top(), cross.right_bottom()], stroke);
    ui.painter()
        .line_segment([cross.right_top(), cross.left_bottom()], stroke);
    response
}

/// Draws a heavy Dialog's `body` above its default button reading `accept` and its
/// Cancel reading `cancel`. A fitted Dialog's body is `fitted_width` wide and doesn't
/// scroll; any other's body scrolls when it is too tall for the Dialog.
fn heavy_contents(
    ui: &mut egui::Ui,
    fitted_width: Option<f32>,
    accept: String,
    cancel: String,
    body: impl FnOnce(&mut egui::Ui),
) -> (Response, Option<Response>, Option<Response>) {
    if let Some(width) = fitted_width {
        ui.set_width(width);
        body(ui);
        // An auto-sized window offers all the room it may grow to, which the buttons'
        // row would centre itself in.
        let row = Vec2::new(ui.available_width(), button_height(ui));
        ui.allocate_ui(row, |ui| buttons(ui, accept, Some(cancel), None))
            .inner
    } else {
        let height = ui.available_height();
        body_area(ui, height, body);
        buttons(ui, accept, Some(cancel), None)
    }
}

/// The layer of the backdrop behind the heavy Dialog with id `heavy`, which is its
/// modal layer.
fn heavy_backdrop(heavy: Id) -> LayerId {
    LayerId::new(Order::Middle, heavy.with("backdrop"))
}

/// Draws a Dialog's `body` in `ui`, scrolling when it is taller than `height` less a
/// row of buttons below it.
fn body_area(ui: &mut egui::Ui, height: f32, body: impl FnOnce(&mut egui::Ui)) {
    let body_height = height - button_height(ui) - ui.spacing().item_spacing.y;
    ScrollArea::vertical()
        .max_height(body_height.max(0.0))
        .min_scrolled_height(0.0)
        .auto_shrink([false, true])
        .show(ui, body);
}

/// The height of a row of a Dialog's buttons in `ui`.
fn button_height(ui: &egui::Ui) -> f32 {
    let spacing = ui.spacing();
    (ui.text_style_height(&TextStyle::Button) + 2.0 * spacing.button_padding.y)
        .max(spacing.interact_size.y)
}

/// A Dialog's default button reading `accept`, any Cancel button reading `cancel` and
/// any Back button reading `back`, at the right edge of `ui`.
fn buttons(
    ui: &mut egui::Ui,
    accept: String,
    cancel: Option<String>,
    back: Option<String>,
) -> (Response, Option<Response>, Option<Response>) {
    // Right to left, so the default button sits left of Cancel at the right edge, and
    // Back left of the default button.
    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
        let cancel = cancel.map(|cancel| ui.add(Button::new(cancel)));
        let accept = ui.add(Button::new(accept));
        let back = back.map(|back| ui.add(Button::new(back)));
        (accept, cancel, back)
    })
    .inner
}

/// Whether Enter pressed a Dialog's default button: Enter presses it unless focus is
/// on `cancel` or `back`, which Enter presses instead.
fn enter_pressed(ctx: &egui::Context, cancel: Option<&Response>, back: Option<&Response>) -> bool {
    let other_button_focused =
        cancel.is_some_and(Response::has_focus) || back.is_some_and(Response::has_focus);
    !other_button_focused && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter))
}

#[cfg(test)]
mod tests {
    use eframe::egui::accesskit::Role;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable as _;

    use super::*;

    /// A heavy Dialog with a confirm open over it, and what became of each.
    struct Stacked {
        heavy: OpenDialog<()>,
        confirm: OpenDialog<()>,
        accepted: bool,
        confirmed: bool,
    }

    /// A heavy Dialog with a confirm drawn over it every frame.
    fn stacked() -> Harness<'static, Stacked> {
        let ctx = egui::Context::default();
        let stacked = Stacked {
            heavy: OpenDialog::new(&ctx, ()),
            confirm: OpenDialog::new(&ctx, ()),
            accepted: false,
            confirmed: false,
        };
        let mut harness = Harness::new_ui_state(
            |ui, stacked: &mut Stacked| {
                let ctx = ui.ctx().clone();
                let theme = Theme::default();
                let heavy = HeavyDialog::new(
                    Id::new("heavy"),
                    "Heavy".to_owned(),
                    "OK".to_owned(),
                    "Cancel".to_owned(),
                    "Close dialog".to_owned(),
                    &theme,
                );
                let outcome = heavy.show(&ctx, &mut stacked.heavy, |ui, ()| {
                    ui.label("Behind");
                });
                stacked.accepted |= outcome == DialogOutcome::Accepted;
                let confirm = LightDialog::new(
                    Id::new("confirm"),
                    "Yes".to_owned(),
                    "No".to_owned(),
                    &theme,
                );
                let outcome = confirm.show(&ctx, &mut stacked.confirm, |ui, ()| {
                    ui.label("Sure?");
                });
                stacked.confirmed |= outcome == DialogOutcome::Accepted;
            },
            stacked,
        );
        harness.run();
        harness
    }

    #[test]
    fn a_confirm_opened_from_a_heavy_dialog_draws_above_it() {
        let mut harness = stacked();

        // Both open centred, so the click on Yes lands where the heavy Dialog is too.
        harness.get_by_role_and_label(Role::Button, "Yes").click();
        harness.run();

        assert!(harness.state().confirmed);
    }

    #[test]
    fn enter_presses_the_confirm_over_a_heavy_dialog_not_the_dialog() {
        let mut harness = stacked();

        harness.key_press(Key::Enter);
        harness.run();

        let state = harness.state();
        assert_eq!((state.confirmed, state.accepted), (true, false));
    }
}
