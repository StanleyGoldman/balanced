//! The segmented date field that edits a Register Date cell.
//!
//! The field shows the day, month and year in the formatting locale's order. Typing
//! starts the date over from the first segment. Two digits, or a first digit no
//! segment of that kind starts with (`3` in a month), move to the next segment, and a
//! typed separator moves on early. A full date typed or pasted at once fills every
//! segment, and so do eight digits typed with no separators (`20261005`) when they
//! make a date in ISO order.
//!
//! While nothing has been typed, shortcut keys move the date: `t` today, `+`/`-` and
//! ↑/↓ a day, `m`/`h` the month's first and last day, `y`/`r` the year's, PgUp/PgDn a
//! month. A 📅 button, or Alt+↓, opens a calendar whose days pick on pointer press.

use balanced_i18n::{DateParseError, DateSegment, Localizer};
use eframe::egui::accesskit::Role;
use eframe::egui::{
    self, Align2, Area, Event, EventFilter, Frame, Id, Key, Label, Modifiers, Order, Rect,
    Response, Sense, StrokeKind, TextStyle, WidgetInfo, WidgetText, WidgetType, pos2, vec2,
};
use icu_calendar::types::Weekday;
use icu_calendar::{Date, Iso};
use jiff::ToSpan as _;

use crate::clock::{to_civil, to_iso};
use crate::i18n::fl;

/// The separators a typed date may use, besides spaces.
const SEPARATORS: &str = "/.-";

/// How many digits a date typed with no separators has: `YYYYMMDD`.
const DIGITS_ONLY_LENGTH: usize = 8;

/// The days of the week, Monday first, as ICU4X numbers them.
const WEEKDAYS: [Weekday; 7] = [
    Weekday::Monday,
    Weekday::Tuesday,
    Weekday::Wednesday,
    Weekday::Thursday,
    Weekday::Friday,
    Weekday::Saturday,
    Weekday::Sunday,
];

/// The space between the calendar's day buttons, and the width of each.
const CALENDAR_SPACING: f32 = 2.0;
const CALENDAR_DAY_WIDTH: f32 = 28.0;

/// The date field's state while its cell is edited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DateEdit {
    /// The cell's saved date, which the edit changes only when committed.
    saved: Date<Iso>,
    /// The date the field holds while nothing has been typed: the saved date, or where
    /// a shortcut or the calendar moved it.
    date: Date<Iso>,
    /// The segments typed, once anything has been. Shortcuts act only while this is
    /// `None`.
    typed: Option<Typed>,
    /// The first day of the month the calendar shows, while it is open.
    calendar: Option<Date<Iso>>,
    /// Whether the last commit was refused, so the format message shows.
    rejected: bool,
    /// Whether the pointer went down on the 📅 button or the calendar and hasn't come
    /// up. egui takes focus from the field on the release, so the field takes it back.
    holding_focus: bool,
}

/// The digits typed into each segment, in the locale's order, and where typing goes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Typed {
    segments: [String; 3],
    /// The index of the segment typing goes to.
    slot: usize,
    /// Whether the next digit replaces the segment's digits, as it does in a segment
    /// just moved to.
    fresh: bool,
    /// Whether typing filled the segment before this one and moved on, so a separator
    /// typed next is taken as already typed.
    advanced: bool,
    /// Every digit typed since the edit began, while nothing but digits has been typed.
    digits: Option<String>,
}

impl Typed {
    /// No segments filled, typing at the first.
    fn empty() -> Self {
        Self {
            segments: Default::default(),
            slot: 0,
            fresh: false,
            advanced: false,
            digits: Some(String::new()),
        }
    }

    /// Every segment filled from `date`, typing at the last.
    fn filled(i18n: &Localizer, date: Date<Iso>) -> Self {
        Self {
            segments: segments_of(i18n, date),
            slot: 2,
            fresh: false,
            advanced: false,
            digits: None,
        }
    }

    /// Moves typing to the segment `slot`, whose digits the next digit replaces.
    fn move_to(&mut self, slot: usize) {
        self.slot = slot;
        self.fresh = true;
        self.advanced = false;
        self.digits = None;
    }

    /// Types `digit` into the current segment, then moves on when the segment can
    /// take no more.
    fn type_digit(&mut self, kind: DateSegment, digit: char) {
        let segment = &mut self.segments[self.slot];
        if self.fresh {
            segment.clear();
            self.fresh = false;
        }
        self.advanced = false;
        if let Some(digits) = &mut self.digits {
            digits.push(digit);
        }
        if segment.len() >= width(kind) {
            // The last segment is full: the digit only counts towards eight digits.
            return;
        }
        segment.push(digit);
        if is_full(kind, segment) && self.slot < 2 {
            self.slot += 1;
            self.fresh = true;
            self.advanced = true;
        }
    }

    /// Moves to the next segment, unless typing just moved there.
    fn type_separator(&mut self) {
        self.digits = None;
        if std::mem::take(&mut self.advanced) {
            return;
        }
        if self.slot < 2 {
            self.move_to(self.slot + 1);
        }
    }

    /// Deletes the last digit of the current segment, or of the one before it when the
    /// current one is empty.
    fn backspace(&mut self) {
        self.digits = None;
        self.advanced = false;
        self.fresh = false;
        if self.segments[self.slot].is_empty() && self.slot > 0 {
            self.slot -= 1;
        }
        self.segments[self.slot].pop();
    }
}

/// The most digits a segment of `kind` takes.
fn width(kind: DateSegment) -> usize {
    match kind {
        DateSegment::Day | DateSegment::Month => 2,
        DateSegment::Year => 4,
    }
}

/// Whether a segment of `kind` holding `digits` can take no more: it has its width, or
/// its first digit is too large for a second one to follow (`4` as a day, `2` as a
/// month).
fn is_full(kind: DateSegment, digits: &str) -> bool {
    let largest_first_digit = match kind {
        DateSegment::Day => '3',
        DateSegment::Month => '1',
        DateSegment::Year => '9',
    };
    let mut chars = digits.chars();
    match (chars.next(), chars.next()) {
        (Some(first), None) => first > largest_first_digit,
        _ => digits.len() >= width(kind),
    }
}

/// `date`'s day, month and year in the formatting locale's order, with the locale's
/// zero padding.
fn segments_of(i18n: &Localizer, date: Date<Iso>) -> [String; 3] {
    let layout = i18n.date_segments();
    let padded = layout.zero_padded();
    layout.order().map(|segment| match segment {
        DateSegment::Day if padded => format!("{:02}", date.day_of_month().0),
        DateSegment::Day => date.day_of_month().0.to_string(),
        DateSegment::Month if padded => format!("{:02}", date.month().ordinal),
        DateSegment::Month => date.month().ordinal.to_string(),
        DateSegment::Year => format!("{:04}", date.year().extended_year()),
    })
}

/// A shortcut that moves the date of an unedited field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shortcut {
    Today,
    Days(i8),
    Months(i8),
    MonthStart,
    MonthEnd,
    YearStart,
    YearEnd,
}

impl Shortcut {
    /// The shortcut typed as `text`, when it is one.
    fn typed(text: &str) -> Option<Self> {
        match text {
            "t" | "T" => Some(Self::Today),
            "+" | "=" => Some(Self::Days(1)),
            "-" => Some(Self::Days(-1)),
            "m" | "M" => Some(Self::MonthStart),
            "h" | "H" => Some(Self::MonthEnd),
            "y" | "Y" => Some(Self::YearStart),
            "r" | "R" => Some(Self::YearEnd),
            _ => None,
        }
    }

    /// The shortcut on `key`, pressed with no modifiers, when it is one.
    fn key(key: Key) -> Option<Self> {
        #[expect(clippy::wildcard_enum_match_arm, reason = "foreign enum")]
        match key {
            Key::ArrowUp => Some(Self::Days(1)),
            Key::ArrowDown => Some(Self::Days(-1)),
            Key::PageUp => Some(Self::Months(-1)),
            Key::PageDown => Some(Self::Months(1)),
            _ => None,
        }
    }

    /// Where the shortcut moves `date`. Returns `date` unchanged when the result would
    /// fall outside the years -9999 to 9999.
    fn apply(self, date: Date<Iso>, today: Date<Iso>) -> Date<Iso> {
        let Some(civil) = to_civil(date) else {
            return date;
        };
        let moved = match self {
            Self::Today => return today,
            Self::Days(days) => civil.checked_add(i64::from(days).days()).ok(),
            Self::Months(months) => civil.checked_add(i64::from(months).months()).ok(),
            Self::MonthStart => Some(civil.first_of_month()),
            Self::MonthEnd => Some(civil.last_of_month()),
            Self::YearStart => Some(civil.first_of_year()),
            Self::YearEnd => Some(civil.last_of_year()),
        };
        moved.map_or(date, to_iso)
    }
}

/// How a commit of the field ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Commit {
    /// The field holds the saved date: nothing to save.
    Unchanged,
    /// The field holds this new date.
    Changed(Date<Iso>),
    /// The field holds no date. The format message shows until the next keystroke.
    Rejected,
}

impl DateEdit {
    /// The field over a cell whose saved date is `saved`, unedited.
    pub(crate) fn new(saved: Date<Iso>) -> Self {
        Self {
            saved,
            date: saved,
            typed: None,
            calendar: None,
            rejected: false,
            holding_focus: false,
        }
    }

    /// Reads the field's date for saving. A refused date shows the format message.
    pub(crate) fn commit(&mut self, i18n: &Localizer) -> Commit {
        match self.value(i18n) {
            Ok(date) if date == self.saved => Commit::Unchanged,
            Ok(date) => Commit::Changed(date),
            Err(_) => {
                self.rejected = true;
                Commit::Rejected
            }
        }
    }

    /// Whether the last commit was refused.
    pub(crate) fn is_rejected(&self) -> bool {
        self.rejected
    }

    /// Closes the calendar. Returns whether it was open.
    pub(crate) fn close_calendar(&mut self) -> bool {
        self.calendar.take().is_some()
    }

    /// The date the field holds.
    fn value(&self, i18n: &Localizer) -> Result<Date<Iso>, DateParseError> {
        match &self.typed {
            None => Ok(self.date),
            Some(typed) => i18n.parse_date(&typed.segments.join("/")),
        }
    }

    /// The field's text, as shown and as a screen reader reads it.
    fn text(&self, i18n: &Localizer) -> String {
        match &self.typed {
            None => i18n.format_date(self.date),
            Some(typed) => typed.segments.join(i18n.date_segments().separator()),
        }
    }

    /// Acts on this frame's keys and typing, which the field takes from the input.
    fn handle_input(&mut self, ctx: &egui::Context, i18n: &Localizer, today: Date<Iso>) {
        let order = i18n.date_segments().order();
        let events = ctx.input_mut(|input| std::mem::take(&mut input.events));
        let mut kept = Vec::with_capacity(events.len());
        for event in events {
            #[expect(clippy::wildcard_enum_match_arm, reason = "foreign enum")]
            let taken = match &event {
                Event::Key {
                    key: Key::ArrowDown,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.alt => {
                    self.toggle_calendar(i18n, today);
                    true
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                    ..
                } => self.press(*key, order, today),
                Event::Text(text) | Event::Paste(text) => {
                    self.type_text(i18n, order, today, text);
                    true
                }
                _ => false,
            };
            if !taken {
                kept.push(event);
            }
        }
        ctx.input_mut(|input| input.events = kept);
    }

    /// Acts on `key`, pressed with no modifiers. Returns whether the field took it.
    fn press(&mut self, key: Key, order: [DateSegment; 3], today: Date<Iso>) -> bool {
        if let Some(shortcut) = Shortcut::key(key) {
            if self.typed.is_none() {
                self.shortcut(shortcut, today);
            }
            return true;
        }
        #[expect(clippy::wildcard_enum_match_arm, reason = "foreign enum")]
        match key {
            Key::ArrowLeft => {
                if let Some(typed) = &mut self.typed {
                    typed.move_to(typed.slot.saturating_sub(1));
                }
                true
            }
            Key::ArrowRight => {
                if let Some(typed) = &mut self.typed {
                    typed.move_to((typed.slot + 1).min(order.len() - 1));
                }
                true
            }
            Key::Backspace => {
                self.rejected = false;
                self.typed.get_or_insert_with(Typed::empty).backspace();
                true
            }
            _ => false,
        }
    }

    /// Moves the field's date by `shortcut`.
    fn shortcut(&mut self, shortcut: Shortcut, today: Date<Iso>) {
        self.rejected = false;
        self.date = shortcut.apply(self.date, today);
    }

    /// Types `text`: a whole date typed or pasted at once fills the segments, and
    /// anything else is typed a character at a time.
    fn type_text(
        &mut self,
        i18n: &Localizer,
        order: [DateSegment; 3],
        today: Date<Iso>,
        text: &str,
    ) {
        self.rejected = false;
        if self.typed.is_none()
            && let Some(shortcut) = Shortcut::typed(text)
        {
            self.shortcut(shortcut, today);
            return;
        }
        if text.chars().count() > 1
            && let Ok(date) = i18n.parse_date(text)
        {
            self.typed = Some(Typed::filled(i18n, date));
            return;
        }
        for c in text.chars() {
            if c.is_ascii_digit() {
                let typed = self.typed.get_or_insert_with(Typed::empty);
                typed.type_digit(order[typed.slot], c);
                if let Some(digits) = &typed.digits
                    && digits.len() == DIGITS_ONLY_LENGTH
                    && let Ok(date) = i18n.parse_date(digits)
                {
                    self.typed = Some(Typed::filled(i18n, date));
                }
            } else if (c.is_whitespace() || SEPARATORS.contains(c))
                && let Some(typed) = &mut self.typed
            {
                typed.type_separator();
            }
        }
    }

    /// Opens the calendar on the month of the field's date, or of today when the field
    /// holds no date, or closes it.
    fn toggle_calendar(&mut self, i18n: &Localizer, today: Date<Iso>) {
        if self.calendar.take().is_none() {
            let shown = self.value(i18n).unwrap_or(today);
            self.calendar = Some(first_of_month(shown));
        }
    }

    /// Sets the field to `date` picked in the calendar, and closes it.
    fn pick(&mut self, date: Date<Iso>) {
        self.date = date;
        self.typed = None;
        self.calendar = None;
        self.rejected = false;
    }

    /// Draws the field over `rect`, with its 📅 button on the right, its calendar and
    /// its format message below. Keys and typing go to it while it has focus.
    pub(crate) fn show(
        &mut self,
        ui: &egui::Ui,
        i18n: &Localizer,
        today: Date<Iso>,
        id: Id,
        rect: Rect,
    ) -> Response {
        let button_width = rect.height();
        let button =
            Rect::from_x_y_ranges(rect.right() - button_width..=rect.right(), rect.y_range());
        let field = Rect::from_x_y_ranges(rect.left()..=button.left(), rect.y_range());
        let response = ui.interact(field, id, Sense::click());
        if response.clicked() {
            response.request_focus();
        }
        if response.has_focus() {
            ui.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    id,
                    EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    },
                );
            });
            self.handle_input(ui.ctx(), i18n, today);
        }
        let text = self.text(i18n);
        response.widget_info(|| WidgetInfo::text_edit(ui.is_enabled(), &text, &text, ""));
        self.paint(ui, i18n, field, response.has_focus());
        let calendar_button = calendar_button(ui, i18n, id, button);
        self.holding_focus |= calendar_button.is_pointer_button_down_on();
        if calendar_button.clicked() {
            self.toggle_calendar(i18n, today);
        }
        if let Some(month) = self.calendar {
            let shown = Calendar {
                id: id.with("date_calendar"),
                month,
                selected: self.value(i18n).ok(),
                today,
                below: rect,
            };
            match shown.show(ui.ctx(), i18n) {
                CalendarAction::None => {}
                CalendarAction::Month(month) => self.calendar = Some(month),
                CalendarAction::Pick(date) => self.pick(date),
            }
            self.holding_focus |= shown.pressed(ui.ctx());
        } else if self.rejected {
            bubble(ui.ctx(), id, rect, &format_message(i18n, today));
        }
        if self.holding_focus {
            ui.memory_mut(|memory| memory.request_focus(id));
            self.holding_focus = ui.input(|input| input.pointer.any_down());
        }
        response
    }

    /// Paints the field's text in `rect`: the whole date selected while nothing has
    /// been typed, so typing replaces it, or the typed segments with the current one
    /// selected and the empty ones as placeholders.
    fn paint(&self, ui: &egui::Ui, i18n: &Localizer, rect: Rect, focused: bool) {
        if !ui.is_rect_visible(rect) {
            return;
        }
        let painter = ui.painter_at(rect);
        let visuals = ui.visuals();
        let font = TextStyle::Body.resolve(ui.style());
        let text_colour = visuals.text_color();
        let mut x = rect.left();
        let mut place = |text: String, colour, selected: bool| {
            let galley = painter.layout_no_wrap(text, font.clone(), colour);
            let placed = Rect::from_min_size(
                pos2(x, rect.center().y - galley.size().y / 2.0),
                galley.size(),
            );
            if selected && focused {
                painter.rect_filled(placed, 0.0, visuals.selection.bg_fill);
            }
            painter.galley(placed.min, galley, colour);
            x = placed.right();
        };
        match &self.typed {
            None => place(i18n.format_date(self.date), text_colour, true),
            Some(typed) => {
                let layout = i18n.date_segments();
                let placeholders = placeholders(i18n);
                for (slot, segment) in typed.segments.iter().enumerate() {
                    let selected = slot == typed.slot;
                    if segment.is_empty() {
                        let placeholder = placeholders[slot].clone();
                        place(placeholder, visuals.weak_text_color(), selected);
                    } else {
                        place(segment.clone(), text_colour, selected);
                    }
                    if slot < 2 {
                        place(
                            layout.separator().to_owned(),
                            visuals.weak_text_color(),
                            false,
                        );
                    }
                }
            }
        }
        if focused {
            painter.rect_stroke(rect, 0.0, visuals.selection.stroke, StrokeKind::Inside);
        }
    }
}

/// The first day of `date`'s month.
fn first_of_month(date: Date<Iso>) -> Date<Iso> {
    to_civil(date).map_or(date, |civil| to_iso(civil.first_of_month()))
}

/// The letters standing for each segment in the formatting locale's order:
/// `M`, `D`, `YYYY` in `en-US`; `DD`, `MM`, `YYYY` where days and months are padded.
fn placeholders(i18n: &Localizer) -> [String; 3] {
    let layout = i18n.date_segments();
    let repeat = if layout.zero_padded() { 2 } else { 1 };
    layout.order().map(|segment| match segment {
        DateSegment::Day => fl!(i18n, "field-date-day-letter").repeat(repeat),
        DateSegment::Month => fl!(i18n, "field-date-month-letter").repeat(repeat),
        DateSegment::Year => fl!(i18n, "field-date-year-letter").repeat(4),
    })
}

/// The message a refused date shows: the locale's date pattern, with today as an
/// example.
fn format_message(i18n: &Localizer, today: Date<Iso>) -> String {
    let pattern = placeholders(i18n).join(i18n.date_segments().separator());
    fl!(
        i18n,
        "field-date-invalid",
        pattern = pattern,
        example = i18n.format_date(today)
    )
}

/// The 📅 button in `rect`, at the right of the field. It never takes focus.
fn calendar_button(ui: &egui::Ui, i18n: &Localizer, field: Id, rect: Rect) -> Response {
    let response = ui
        .interact(rect, field.with("date_calendar_button"), Sense::CLICK)
        .on_hover_text(fl!(i18n, "field-date-calendar", "tooltip"));
    let label = fl!(i18n, "field-date-calendar");
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), &label));
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        if response.hovered() || response.is_pointer_button_down_on() {
            ui.painter().rect_filled(rect, 0.0, visuals.weak_bg_fill);
        }
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "📅",
            TextStyle::Body.resolve(ui.style()),
            visuals.text_color(),
        );
    }
    response
}

/// Shows `message` in a bubble under `rect`.
fn bubble(ctx: &egui::Context, field: Id, rect: Rect, message: &str) {
    Area::new(field.with("date_bubble"))
        .order(Order::Tooltip)
        .fixed_pos(rect.left_bottom())
        .interactable(false)
        .show(ctx, |ui| {
            Frame::popup(ui.style()).show(ui, |ui| {
                ui.add(Label::new(WidgetText::from(message)).extend());
            });
        });
}

/// What a press in the calendar did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CalendarAction {
    None,
    /// The calendar moved to the month starting on this day.
    Month(Date<Iso>),
    /// The user picked this day.
    Pick(Date<Iso>),
}

/// The calendar under a date field, showing one month.
struct Calendar {
    id: Id,
    /// The month's first day.
    month: Date<Iso>,
    /// The field's date, which the calendar marks.
    selected: Option<Date<Iso>>,
    today: Date<Iso>,
    /// The cell the calendar opens under.
    below: Rect,
}

impl Calendar {
    /// Draws the calendar: the month and year between ◀ and ▶, the weekdays from the
    /// locale's first, and the month's days, which pick on pointer press.
    fn show(&self, ctx: &egui::Context, i18n: &Localizer) -> CalendarAction {
        let mut action = CalendarAction::None;
        let Some(month) = to_civil(self.month) else {
            return action;
        };
        Area::new(self.id)
            .order(Order::Foreground)
            .fixed_pos(self.below.left_bottom())
            .show(ctx, |ui| {
                Frame::popup(ui.style()).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(CALENDAR_SPACING, CALENDAR_SPACING);
                    ui.horizontal(|ui| {
                        let previous = month.checked_sub(1.month()).ok();
                        if nav_button(ui, "◀", &fl!(i18n, "field-date-previous-month"))
                            && let Some(previous) = previous
                        {
                            action = CalendarAction::Month(to_iso(previous));
                        }
                        ui.add_sized(
                            vec2(CALENDAR_DAY_WIDTH * 5.0, ui.spacing().interact_size.y),
                            Label::new(i18n.format_month_year(self.month)),
                        );
                        let next = month.checked_add(1.month()).ok();
                        if nav_button(ui, "▶", &fl!(i18n, "field-date-next-month"))
                            && let Some(next) = next
                        {
                            action = CalendarAction::Month(to_iso(next));
                        }
                    });
                    if let Some(picked) = self.grid(ui, i18n, month) {
                        action = CalendarAction::Pick(picked);
                    }
                });
            });
        action
    }

    /// Whether the pointer was pressed on the calendar this frame.
    fn pressed(&self, ctx: &egui::Context) -> bool {
        let rect = ctx.memory(|memory| memory.area_rect(self.id));
        ctx.input(|input| {
            input.pointer.any_pressed()
                && input
                    .pointer
                    .interact_pos()
                    .zip(rect)
                    .is_some_and(|(pos, rect)| rect.contains(pos))
        })
    }

    /// The weekday headings and the month's days. Returns the day pressed, if any.
    fn grid(
        &self,
        ui: &mut egui::Ui,
        i18n: &Localizer,
        month: jiff::civil::Date,
    ) -> Option<Date<Iso>> {
        let first_weekday = i18n.first_weekday();
        let start = WEEKDAYS
            .iter()
            .position(|&weekday| weekday == first_weekday)
            .unwrap_or_default();
        let week: Vec<Weekday> = (0..7)
            .map(|offset| WEEKDAYS[(start + offset) % 7])
            .collect();
        let size = vec2(CALENDAR_DAY_WIDTH, ui.spacing().interact_size.y);
        ui.horizontal(|ui| {
            for &weekday in &week {
                let name = i18n.format_weekday(weekday);
                let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
                response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, &name));
                ui.ctx()
                    .accesskit_node_builder(response.id, |node| node.set_role(Role::ColumnHeader));
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    &name,
                    TextStyle::Small.resolve(ui.style()),
                    ui.visuals().weak_text_color(),
                );
            }
        });
        let first = to_iso(month);
        let lead = week
            .iter()
            .position(|&weekday| weekday == first.weekday())
            .unwrap_or_default();
        let days = usize::try_from(month.days_in_month()).unwrap_or_default();
        let mut picked = None;
        for row in 0..(lead + days).div_ceil(7) {
            ui.horizontal(|ui| {
                for column in 0..7 {
                    let place = row * 7 + column;
                    let Some(day) = place.checked_sub(lead).filter(|&day| day < days) else {
                        ui.allocate_exact_size(size, Sense::hover());
                        continue;
                    };
                    let date = month
                        .checked_add(i64::try_from(day).unwrap_or_default().days())
                        .map_or(first, to_iso);
                    if self.day(ui, i18n, date, size) {
                        picked = Some(date);
                    }
                }
            });
        }
        picked
    }

    /// One day of the month. Returns whether the pointer was pressed on it.
    fn day(&self, ui: &mut egui::Ui, i18n: &Localizer, date: Date<Iso>, size: egui::Vec2) -> bool {
        let (rect, response) = ui.allocate_exact_size(size, Sense::CLICK);
        let label = i18n.format_date(date);
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &label));
        let visuals = ui.style().interact(&response);
        if self.selected == Some(date) {
            ui.painter()
                .rect_filled(rect, 0.0, ui.visuals().selection.bg_fill);
        } else if response.hovered() {
            ui.painter().rect_filled(rect, 0.0, visuals.weak_bg_fill);
        }
        if date == self.today {
            ui.painter()
                .rect_stroke(rect, 0.0, visuals.fg_stroke, StrokeKind::Inside);
        }
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            date.day_of_month().0.to_string(),
            TextStyle::Body.resolve(ui.style()),
            visuals.text_color(),
        );
        response.is_pointer_button_down_on()
    }
}

/// A ◀ or ▶ button that moves the calendar a month. It never takes focus.
fn nav_button(ui: &mut egui::Ui, glyph: &str, label: &str) -> bool {
    let size = vec2(CALENDAR_DAY_WIDTH, ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_exact_size(size, Sense::CLICK);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
    let visuals = ui.style().interact(&response);
    if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, visuals.weak_bg_fill);
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        TextStyle::Body.resolve(ui.style()),
        visuals.text_color(),
    );
    response.clicked()
}
