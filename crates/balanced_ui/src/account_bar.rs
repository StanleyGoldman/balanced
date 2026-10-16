//! The Account bar: the accounts in their fixed groups with their balances, group
//! headings that fold, and Net Worth in a footer.

use std::collections::HashSet;
use std::sync::Arc;

use balanced_core::AccountId;
use balanced_i18n::Localizer;
use eframe::egui::{
    self, CentralPanel, Color32, Event, Frame, Galley, Id, Panel, PanelState, Rect, Response,
    RichText, ScrollArea, Sense, Shape, Stroke, StrokeKind, TextStyle, TextWrapMode, WidgetInfo,
    WidgetText, WidgetType, pos2, vec2,
};

use crate::accounts_model::{AccountRow, AccountsModel, Section};
use crate::command::{AppCommand, CommandSender};
use crate::formatted_amount::FormattedAmount;
use crate::i18n::fl;
use crate::layout::Layout;
use crate::route::{OpenIn, Route};
use crate::theme::Theme;

/// Whether the Account bar shows its accounts or is folded to a narrow rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountBarFold {
    Open,
    Rail,
}

/// The Account bar's width, whether it is folded to a rail, and which of its sections
/// are folded. The saved [`Layout`] carries all three between runs.
#[derive(Debug)]
pub(crate) struct AccountBar {
    width: f32,
    fold: AccountBarFold,
    folded: HashSet<Section>,
    /// Whether the next frame gives the Account bar keyboard focus.
    focus_requested: bool,
    /// Whether one of the Account bar's widgets held keyboard focus in the last frame.
    has_focus: bool,
}

impl AccountBar {
    /// The Account bar's width on a first run, and the narrowest and widest it gets.
    pub(crate) const DEFAULT_WIDTH: f32 = 200.0;
    pub(crate) const MIN_WIDTH: f32 = 140.0;
    pub(crate) const MAX_WIDTH: f32 = 480.0;

    /// The Account bar as `layout` left it.
    pub(crate) fn new(layout: &Layout) -> Self {
        Self {
            width: layout.account_bar_width(),
            fold: layout.account_bar_fold(),
            folded: layout.folded_sections(),
            focus_requested: false,
            has_focus: false,
        }
    }

    /// Gives the Account bar keyboard focus when it is next drawn: the selected
    /// account's row, or else the first account's, or else the first heading. Folded to
    /// a rail, the button that opens it takes focus.
    pub(crate) fn request_focus(&mut self) {
        self.focus_requested = true;
    }

    /// Whether one of the Account bar's widgets held keyboard focus in the last frame.
    pub(crate) fn has_focus(&self) -> bool {
        self.has_focus
    }

    /// The Account bar's width while open, which it keeps while folded to a rail.
    pub(crate) fn width(&self) -> f32 {
        self.width
    }

    pub(crate) fn fold(&self) -> AccountBarFold {
        self.fold
    }

    /// The sections whose headings are folded.
    pub(crate) fn folded_sections(&self) -> &HashSet<Section> {
        &self.folded
    }

    /// Draws the Account bar from `model`, highlighting `selected`, or the rail it
    /// folds to. Clicking an account opens it in the current Register, and
    /// right-clicking it offers its commands. Clicking a heading folds or unfolds its
    /// section. Dragging the Account bar's edge resizes it.
    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        theme: &Theme,
        commands: &CommandSender,
        model: &AccountsModel,
        selected: Option<AccountId>,
    ) {
        let id = Id::new("account_bar");
        let rail = Panel::left("account_bar_rail")
            .frame(theme.chrome_frame())
            .resizable(false)
            .exact_size(RAIL_WIDTH);
        let bar = Panel::left(id)
            .frame(theme.chrome_frame())
            .default_size(self.width)
            .size_range(Self::MIN_WIDTH..=Self::MAX_WIDTH);
        let mut open = self.fold == AccountBarFold::Open;
        let mut fold_clicked = false;
        let mut focus = Focus::default();
        let frame = BarFrame {
            i18n,
            theme,
            commands,
            model,
            selected,
        };
        Panel::show_switched(ui, &mut open, rail, bar, |ui, expanded| {
            fold_clicked = if expanded {
                self.open_bar(ui, &frame, &mut focus)
            } else {
                Self::rail(ui, i18n, &mut focus)
            };
        });
        if std::mem::take(&mut self.focus_requested)
            && let Some((_, id)) = focus.target
        {
            ui.ctx().memory_mut(|memory| memory.request_focus(id));
        }
        self.has_focus = focus.held;
        if fold_clicked {
            open = !open;
        }
        self.fold = if open {
            AccountBarFold::Open
        } else {
            AccountBarFold::Rail
        };
        // The panel keeps its width before a drag until the drag ends.
        if let Some(state) = PanelState::load(ui.ctx(), id) {
            self.width = state.size().x;
        }
    }

    /// The open Account bar: the fold button, the sections and the Net Worth footer.
    /// Returns whether the fold button was clicked.
    fn open_bar(&mut self, ui: &mut egui::Ui, frame: &BarFrame<'_>, focus: &mut Focus) -> bool {
        let BarFrame {
            i18n, theme, model, ..
        } = *frame;
        let fold = Panel::top("account_bar_header")
            .frame(Frame::NONE)
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let button = rail_button(ui, fl!(i18n, "account-bar-fold"), Direction::Left);
                    focus.note(&button, Candidate::Last);
                    button.clicked()
                })
                .inner
            })
            .inner;
        Panel::bottom("net_worth")
            .frame(theme.footer_frame())
            .show(ui, |ui| Self::net_worth(ui, i18n, theme, model.net_worth()));
        CentralPanel::default().frame(Frame::NONE).show(ui, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                self.sections(ui, frame, focus);
            });
        });
        fold
    }

    /// The rail the Account bar folds to: only the button that opens it again.
    /// Returns whether that button was clicked.
    fn rail(ui: &mut egui::Ui, i18n: &Localizer, focus: &mut Focus) -> bool {
        ui.vertical_centered(|ui| {
            let button = rail_button(ui, fl!(i18n, "account-bar-unfold"), Direction::Right);
            focus.note(&button, Candidate::Last);
            button.clicked()
        })
        .inner
    }

    fn sections(&mut self, ui: &mut egui::Ui, frame: &BarFrame<'_>, focus: &mut Focus) {
        let BarFrame {
            i18n,
            theme,
            commands,
            model,
            selected,
        } = *frame;
        let mut jump = LetterJump::default();
        for section_rows in model.sections() {
            let section = section_rows.section();
            let fold = if self.folded.contains(&section) {
                Fold::Folded
            } else {
                Fold::Open
            };
            let heading = heading(ui, section, section.label(i18n), fold);
            focus.note(&heading, Candidate::Heading);
            jump.note_heading(&heading);
            if heading.clicked() {
                match fold {
                    Fold::Open => self.folded.insert(section),
                    Fold::Folded => self.folded.remove(&section),
                };
            }
            match fold {
                Fold::Folded => continue,
                Fold::Open => {}
            }
            for row in section_rows.rows() {
                let balance = model
                    .balance(row.id())
                    .expect("the model formats a balance for every account it lists");
                let is_selected = selected == Some(row.id());
                let response = account_row(ui, theme, row, balance, is_selected);
                let candidate = if is_selected {
                    Candidate::Selected
                } else {
                    Candidate::Account
                };
                focus.note(&response, candidate);
                jump.note_account(&response, row.name());
                if response.gained_focus() {
                    response.scroll_to_me(None);
                }
                if response.clicked() {
                    // TODO(#100): open in a new Register tab when the "Open an account
                    // in" Preference says so.
                    open(commands, row.id(), OpenIn::CurrentRegister);
                }
                response.context_menu(|ui| account_menu(ui, i18n, commands, row.id()));
            }
        }
        jump.act(ui.ctx());
    }

    fn net_worth(ui: &mut egui::Ui, i18n: &Localizer, theme: &Theme, net_worth: &FormattedAmount) {
        ui.horizontal(|ui| {
            ui.label(fl!(i18n, "account-bar-net-worth"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(net_worth.styled(theme));
            });
        });
    }
}

/// What every part of the Account bar is drawn with in one frame.
#[derive(Clone, Copy)]
struct BarFrame<'a> {
    i18n: &'a Localizer,
    theme: &'a Theme,
    commands: &'a CommandSender,
    model: &'a AccountsModel,
    /// The account the Main view shows, highlighted.
    selected: Option<AccountId>,
}

/// How strongly a widget is wanted as the Account bar's focus target, weakest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Candidate {
    /// A rail button, taken only when nothing else is shown.
    Last,
    Heading,
    Account,
    Selected,
}

/// What one frame of the Account bar learnt about keyboard focus: whether one of its
/// widgets holds it, and which widget takes it when the Account bar is given focus.
#[derive(Debug, Default)]
struct Focus {
    held: bool,
    /// The widget that takes focus, and how strongly it was wanted.
    target: Option<(Candidate, Id)>,
}

impl Focus {
    /// Notes a widget drawn in the Account bar. Of equally wanted widgets, the first
    /// drawn becomes the target.
    fn note(&mut self, response: &Response, candidate: Candidate) {
        self.held |= response.has_focus();
        if self
            .target
            .is_none_or(|(strongest, _)| candidate > strongest)
        {
            self.target = Some((candidate, response.id));
        }
    }
}

/// The accounts shown in one frame of the Account bar, in order, and where its focus
/// sits among them, so that typing a letter can jump to the next account starting
/// with it.
#[derive(Debug, Default)]
struct LetterJump<'a> {
    accounts: Vec<(Id, &'a str)>,
    /// The index in `accounts` the search for the next account starts from, while the
    /// Account bar's focus is on an account or a heading.
    start: Option<usize>,
}

impl<'a> LetterJump<'a> {
    fn note_heading(&mut self, response: &Response) {
        if response.has_focus() {
            self.start = Some(self.accounts.len());
        }
    }

    fn note_account(&mut self, response: &Response, name: &'a str) {
        if response.has_focus() {
            self.start = Some(self.accounts.len() + 1);
        }
        self.accounts.push((response.id, name));
    }

    /// Moves focus to the next account whose name starts with the text typed this
    /// frame, ignoring case and wrapping to the first account, while the focus is in the
    /// Account bar. The typed text is consumed.
    fn act(self, ctx: &egui::Context) {
        let Some(start) = self.start else {
            return;
        };
        let typed = ctx.input_mut(|input| {
            let mut typed = String::new();
            input.events.retain(|event| {
                let Event::Text(text) = event else {
                    return true;
                };
                typed.push_str(text);
                false
            });
            typed.to_lowercase()
        });
        if typed.is_empty() {
            return;
        }
        let (before, after) = self.accounts.split_at(start.min(self.accounts.len()));
        let next = after
            .iter()
            .chain(before)
            .find(|(_, name)| name.to_lowercase().starts_with(&typed));
        if let Some(&(id, _)) = next {
            ctx.memory_mut(|memory| memory.request_focus(id));
            ctx.request_repaint();
        }
    }
}

/// Opens `account`'s Register where `open_in` says.
fn open(commands: &CommandSender, account: AccountId, open_in: OpenIn) {
    commands.send(AppCommand::Navigate(Route::Account { account, open_in }));
}

/// An account's right-click menu.
fn account_menu(ui: &mut egui::Ui, i18n: &Localizer, commands: &CommandSender, account: AccountId) {
    if ui.button(fl!(i18n, "account-bar-open-register")).clicked() {
        open(commands, account, OpenIn::CurrentRegister);
    }
    if ui
        .button(fl!(i18n, "account-bar-open-in-new-tab"))
        .clicked()
    {
        open(commands, account, OpenIn::NewTab);
    }
}

/// The width of the rail the Account bar folds to.
const RAIL_WIDTH: f32 = 28.0;

/// Which way a rail button's triangle points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Left,
    Right,
}

/// A square button labelled `label` for AccessKit, showing a triangle pointing
/// `direction`. The triangle is painted, as the fold triangles are.
fn rail_button(ui: &mut egui::Ui, label: String, direction: Direction) -> Response {
    let size = ui.spacing().interact_size.y;
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, &label));
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        if response.hovered() || response.is_pointer_button_down_on() {
            ui.painter().rect_filled(rect, 0.0, visuals.weak_bg_fill);
        }
        let icon = Rect::from_center_size(rect.center(), vec2(size / 3.0, size / 2.0));
        let points = match direction {
            Direction::Left => vec![icon.right_top(), icon.left_center(), icon.right_bottom()],
            Direction::Right => vec![icon.left_top(), icon.right_center(), icon.left_bottom()],
        };
        ui.painter().add(Shape::convex_polygon(
            points,
            visuals.text_color(),
            Stroke::NONE,
        ));
    }
    response.on_hover_text(label)
}

/// Whether a section shows its accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fold {
    Open,
    Folded,
}

/// The width of the fold triangle's column, which also indents the account rows.
fn icon_column(ui: &egui::Ui) -> f32 {
    ui.spacing().icon_width + ui.spacing().item_spacing.x
}

/// Takes the full width of the Account bar, `height` tall, as one click target whose
/// id comes from `id_salt`, so its state stays with it when sections fold above it.
fn full_width_target(ui: &mut egui::Ui, id_salt: impl egui::AsIdSalt, height: f32) -> Response {
    let (_, rect) = ui.allocate_space(vec2(ui.available_width(), height));
    let id = ui.make_persistent_id(id_salt);
    ui.interact(rect, id, Sense::click())
}

/// Outlines the widget of `response` with `stroke` while it has keyboard focus.
fn focus_outline(ui: &egui::Ui, response: &Response, stroke: Stroke) {
    if response.has_focus() {
        ui.painter()
            .rect_stroke(response.rect, 0.0, stroke, StrokeKind::Inside);
    }
}

/// Paints `galley` at `left`, centred vertically in `row`.
fn paint_centred(ui: &egui::Ui, galley: Arc<Galley>, left: f32, row: Rect, color: Color32) {
    let top = row.center().y - galley.size().y / 2.0;
    ui.painter().galley(pos2(left, top), galley, color);
}

/// A section heading: a fold triangle and the section's name, the full width of the
/// Account bar.
fn heading(ui: &mut egui::Ui, section: Section, label: String, fold: Fold) -> Response {
    let response = full_width_target(
        ui,
        ("account_bar_heading", section),
        ui.spacing().interact_size.y,
    );
    let enabled = ui.is_enabled();
    let open = fold == Fold::Open;
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::CollapsingHeader, enabled, open, &label));
    let rect = response.rect;
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        if response.hovered() {
            ui.painter().rect_filled(rect, 0.0, visuals.weak_bg_fill);
        }
        focus_outline(ui, &response, visuals.bg_stroke);
        let icon_size = ui.spacing().icon_width / 2.0;
        let icon = Rect::from_center_size(
            pos2(rect.left() + ui.spacing().icon_width / 2.0, rect.center().y),
            vec2(icon_size, icon_size),
        );
        ui.painter()
            .add(fold_triangle(icon, fold, visuals.text_color()));
        let text_left = rect.left() + icon_column(ui);
        let galley = WidgetText::from(RichText::new(label).strong()).into_galley(
            ui,
            Some(TextWrapMode::Truncate),
            rect.right() - text_left,
            TextStyle::Body,
        );
        paint_centred(ui, galley, text_left, rect, visuals.text_color());
    }
    response
}

/// ▾ when the section is open and ▸ when it is folded, filling `icon`. The default
/// fonts lack both glyphs, so the triangle is painted.
fn fold_triangle(icon: Rect, fold: Fold, color: Color32) -> Shape {
    let points = match fold {
        Fold::Open => vec![icon.left_top(), icon.right_top(), icon.center_bottom()],
        Fold::Folded => vec![icon.left_top(), icon.right_center(), icon.left_bottom()],
    };
    Shape::convex_polygon(points, color, Stroke::NONE)
}

/// An account row: the name on the left and the balance on the right, indented under
/// its heading. The whole row is one click target, and its text can't be selected.
fn account_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    row: &AccountRow,
    balance: &FormattedAmount,
    selected: bool,
) -> Response {
    let response = full_width_target(ui, ("account_bar_row", row.id()), theme.row_height());
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        WidgetInfo::selected(WidgetType::SelectableLabel, enabled, selected, row.name())
    });
    let rect = response.rect;
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let indent = rect.left() + icon_column(ui);
    let gap = ui.spacing().item_spacing;
    if row.starts_sub_group() {
        let rule_y = rect.top() - gap.y / 2.0;
        ui.painter().add(theme.sub_group_rule(
            egui::Rangef::new(indent, rect.right()),
            rule_y,
            ui.pixels_per_point(),
        ));
    }
    let visuals = ui.style().interact_selectable(&response, selected);
    if selected || response.hovered() || response.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, 0.0, visuals.weak_bg_fill);
    }
    focus_outline(ui, &response, visuals.bg_stroke);
    let text_color = visuals.text_color();
    let balance_galley = WidgetText::from(balance.styled(theme)).into_galley(
        ui,
        Some(TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Body,
    );
    let balance_left = rect.right() - gap.x - balance_galley.size().x;
    let name_galley = WidgetText::from(row.name()).into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        (balance_left - gap.x - indent).max(0.0),
        TextStyle::Body,
    );
    paint_centred(ui, name_galley, indent, rect, text_color);
    paint_centred(ui, balance_galley, balance_left, rect, text_color);
    response
}
