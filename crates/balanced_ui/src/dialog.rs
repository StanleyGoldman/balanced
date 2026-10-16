//! The modal helper: how every Dialog dims the Main window, takes Esc and Enter, keeps
//! inside the window and gives focus back when it closes.

use eframe::egui::{
    self, Button, Frame, Id, Key, Layout, Modal, Modifiers, Response, ScrollArea, TextStyle, Vec2,
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
        if ctx.memory(|memory| memory.top_modal_layer().is_some()) {
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
/// button and Cancel, and Back before them in a Dialog with steps. The default button
/// has first focus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LightDialog {
    id: Id,
    accept: String,
    cancel: String,
    back: Option<String>,
}

impl LightDialog {
    /// A light Dialog with `id`, whose default button reads `accept` and whose other
    /// button reads `cancel`.
    pub(crate) fn new(id: Id, accept: String, cancel: String) -> Self {
        Self {
            id,
            accept,
            cancel,
            back: None,
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
        } = self;
        let room =
            (ctx.content_rect().size() - Vec2::splat(2.0 * Theme::DIALOG_MARGIN)).max(Vec2::ZERO);
        let first_frame = !dialog.shown;
        dialog.shown = true;
        let frame = Frame::popup(&ctx.global_style());
        let modal = Modal::new(id).frame(frame).show(ctx, |ui| {
            let inner = (room - frame.total_margin().sum()).max(Vec2::ZERO);
            let width = Theme::LIGHT_DIALOG_WIDTH.min(inner.x);
            ui.set_width(width);
            let spacing = ui.spacing();
            let button_height = (ui.text_style_height(&TextStyle::Button)
                + 2.0 * spacing.button_padding.y)
                .max(spacing.interact_size.y);
            let body_height = inner.y - button_height - spacing.item_spacing.y;
            ScrollArea::vertical()
                .max_height(body_height.max(0.0))
                .min_scrolled_height(0.0)
                .auto_shrink([false, true])
                .show(ui, |ui| body(ui, &mut dialog.form));
            // Right to left, so the default button sits left of Cancel at the right edge,
            // and Back left of the default button.
            let (accept, cancel, back) = ui
                .with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    let cancel = ui.add(Button::new(cancel));
                    let accept = ui.add(Button::new(accept));
                    let back = back.map(|back| ui.add(Button::new(back)));
                    (accept, cancel, back)
                })
                .inner;
            if first_frame {
                accept.request_focus();
            }
            (accept, cancel, back)
        });
        let (accept, cancel, back) = &modal.inner;
        // Enter presses the default button, unless focus is on Cancel or Back, which
        // Enter presses instead.
        let other_button_focused =
            cancel.has_focus() || back.as_ref().is_some_and(Response::has_focus);
        let enter = !other_button_focused
            && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter));
        if accept.clicked() || enter {
            DialogOutcome::Accepted
        } else if back.as_ref().is_some_and(Response::clicked) {
            DialogOutcome::Back
        } else if cancel.clicked() || modal.should_close() {
            DialogOutcome::Cancelled
        } else {
            DialogOutcome::Open
        }
    }
}
