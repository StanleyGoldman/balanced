//! PROTOTYPE, throwaway: theme and density for the shell layout picked in the first session.
//!
//! Question: what theme and density give Quicken's feel in a modern-flat style? Fonts and sizes,
//! row heights, spacing, alternating Register rows, colours, light and dark, display scales.
//!
//! The layout is fixed (the shell-layout prototype's variant D); the variants A–D here are style
//! presets. Density is the Preference ("dense rows", off by default), not a theme property.
//!
//! Run:
//!   cargo run -p balanced_ui --example theming_density_prototype -- \
//!     [--variant a|b|c|d] [--log <dir>/events.jsonl] [--control <dir>/control.txt]
//!
//! The PROTOTYPE dock on the right holds the variant switcher and toggles; F4 collapses it.
//! A–D or ← / → switch variant, 1–0 flip toggles, F2 writes a note into the log, F3 drops a marker.
//!
//! Fixed-seed stub data, English literals. None of this is production code.

#![allow(
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::pedantic,
    clippy::too_many_lines,
    clippy::wildcard_enum_match_arm,
    clippy::unwrap_used,
    clippy::str_to_string,
    clippy::disallowed_methods,
    clippy::disallowed_macros,
    clippy::cast_precision_loss,
    clippy::let_underscore_must_use,
    clippy::iter_over_hash_type,
    missing_debug_implementations,
    unreachable_pub
)]

use std::collections::HashMap;

use balanced_prototype_harness::{Args, Harness};
use eframe::egui;
use egui::{Align, Button, CentralPanel, Id, Layout, Panel, RichText, Sense, Sides, Ui};
use egui_extras::{Column, TableBuilder};
use serde_json::json;

// ---------------------------------------------------------------------------------------------
// Stub data: fixed seed, realistic volume

const SEED: u64 = 0x00ba_1a9c_ed00;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
    Banking,
    Credit,
    Investing,
    PropertyDebt,
}

impl Group {
    const ALL: [Self; 4] = [
        Self::Banking,
        Self::Credit,
        Self::Investing,
        Self::PropertyDebt,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Banking => "Banking",
            Self::Credit => "Credit Cards",
            Self::Investing => "Investing",
            Self::PropertyDebt => "Property & Debt",
        }
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Checking,
    Savings,
    Credit,
    Investment,
    Asset,
    Loan,
}

/// (name, group, kind, transactions, opening balance in cents)
const ACCOUNT_SPECS: &[(&str, Group, Kind, usize, i64)] = &[
    (
        "Everyday Checking",
        Group::Banking,
        Kind::Checking,
        5_000,
        250_000,
    ),
    (
        "Joint Checking",
        Group::Banking,
        Kind::Checking,
        3_200,
        400_000,
    ),
    (
        "Bills Checking",
        Group::Banking,
        Kind::Checking,
        1_800,
        150_000,
    ),
    ("Cash", Group::Banking, Kind::Checking, 600, 20_000),
    (
        "High-Yield Savings",
        Group::Banking,
        Kind::Savings,
        900,
        1_000_000,
    ),
    (
        "Emergency Fund",
        Group::Banking,
        Kind::Savings,
        500,
        1_500_000,
    ),
    (
        "Holiday Savings",
        Group::Banking,
        Kind::Savings,
        500,
        50_000,
    ),
    ("Kids' Savings", Group::Banking, Kind::Savings, 500, 30_000),
    ("Visa Rewards", Group::Credit, Kind::Credit, 2_500, 0),
    ("Mastercard", Group::Credit, Kind::Credit, 1_400, 0),
    ("Store Card", Group::Credit, Kind::Credit, 500, 0),
    ("Travel Card", Group::Credit, Kind::Credit, 800, 0),
    (
        "Brokerage",
        Group::Investing,
        Kind::Investment,
        1_200,
        2_000_000,
    ),
    ("401(k)", Group::Investing, Kind::Investment, 900, 3_000_000),
    ("Roth IRA", Group::Investing, Kind::Investment, 600, 800_000),
    (
        "Traditional IRA",
        Group::Investing,
        Kind::Investment,
        500,
        1_200_000,
    ),
    ("HSA", Group::Investing, Kind::Investment, 500, 100_000),
    (
        "529 College Fund",
        Group::Investing,
        Kind::Investment,
        500,
        200_000,
    ),
    ("House", Group::PropertyDebt, Kind::Asset, 500, 35_000_000),
    ("Car", Group::PropertyDebt, Kind::Asset, 500, 2_800_000),
    (
        "Mortgage",
        Group::PropertyDebt,
        Kind::Loan,
        500,
        -28_000_000,
    ),
    ("Car Loan", Group::PropertyDebt, Kind::Loan, 500, -2_400_000),
    (
        "Student Loan",
        Group::PropertyDebt,
        Kind::Loan,
        500,
        -3_500_000,
    ),
    (
        "Personal Loan",
        Group::PropertyDebt,
        Kind::Loan,
        500,
        -600_000,
    ),
];

const SPENDING: &[(&str, &str, i64)] = &[
    ("Grocery Mart", "Food:Groceries", 18_000),
    ("Corner Bakery", "Food:Groceries", 2_500),
    ("Coffee Corner", "Food:Dining", 1_200),
    ("Noodle House", "Food:Dining", 6_000),
    ("City Power", "Utilities:Electric", 14_000),
    ("Water Works", "Utilities:Water", 6_000),
    ("FiberNet", "Utilities:Internet", 7_000),
    ("Gas & Go", "Auto:Fuel", 6_500),
    ("Quick Lube", "Auto:Service", 9_000),
    ("Book Barn", "Leisure:Books", 3_500),
    ("Cinema 8", "Leisure:Movies", 3_000),
    ("Pharmacy Plus", "Medical:Prescriptions", 4_000),
    ("Hardware Haus", "Home:Repairs", 12_000),
    ("Online Market", "Shopping:General", 8_000),
];

struct Txn {
    day: i64,
    num: Option<u32>,
    payee: &'static str,
    category: &'static str,
    amount: i64,
    balance: i64,
    cleared: bool,
}

struct Account {
    name: &'static str,
    group: Group,
    txns: Vec<Txn>,
    balance: i64,
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    /// Roughly `typical` cents, ±60%.
    fn around(&mut self, typical: i64) -> i64 {
        let spread = (typical * 6 / 5).max(1) as u64;
        (typical * 2 / 5 + self.below(spread) as i64).max(1)
    }
}

/// Days since 1970-01-01 of 2016-01-01; the stub history runs about ten years from there.
const FIRST_DAY: i64 = 16_801;
const HISTORY_DAYS: i64 = 3_900;

fn generate() -> Vec<Account> {
    let mut rng = Lcg(SEED);
    ACCOUNT_SPECS
        .iter()
        .map(|&(name, group, kind, n, opening)| {
            let mut balance = opening;
            let mut check = 1000;
            let mut txns = Vec::with_capacity(n + 1);
            txns.push(Txn {
                day: FIRST_DAY,
                num: None,
                payee: "Opening Balance",
                category: "[Equity]",
                amount: opening,
                balance,
                cleared: true,
            });
            for i in 0..n {
                let day = FIRST_DAY + 1 + (i as i64 * HISTORY_DAYS) / n as i64;
                let roll = rng.below(100);
                let (payee, category, amount) = match kind {
                    Kind::Checking if roll < 5 => {
                        ("Paycheck", "Income:Salary", rng.around(280_000))
                    }
                    Kind::Checking if roll < 8 => {
                        ("Landlord", "Housing:Rent", -rng.around(150_000))
                    }
                    Kind::Checking if roll < 11 => (
                        "Transfer to Visa Rewards",
                        "[Visa Rewards]",
                        -rng.around(60_000),
                    ),
                    Kind::Checking => {
                        let (p, c, typical) = SPENDING[rng.below(SPENDING.len() as u64) as usize];
                        (p, c, -rng.around(typical))
                    }
                    Kind::Savings if roll < 30 => ("Interest", "Income:Interest", rng.around(800)),
                    Kind::Savings if roll < 85 => (
                        "Transfer from Checking",
                        "[Everyday Checking]",
                        rng.around(30_000),
                    ),
                    Kind::Savings => (
                        "Transfer to Checking",
                        "[Everyday Checking]",
                        -rng.around(40_000),
                    ),
                    Kind::Credit if roll < 8 => (
                        "Payment — Thank You",
                        "[Everyday Checking]",
                        rng.around(80_000),
                    ),
                    Kind::Credit => {
                        let (p, c, typical) = SPENDING[rng.below(SPENDING.len() as u64) as usize];
                        (p, c, -rng.around(typical))
                    }
                    Kind::Investment if roll < 40 => {
                        ("Contribution", "[Everyday Checking]", rng.around(50_000))
                    }
                    Kind::Investment if roll < 75 => {
                        ("Market Gain", "Investments:Unrealized", rng.around(60_000))
                    }
                    Kind::Investment => {
                        ("Market Loss", "Investments:Unrealized", -rng.around(45_000))
                    }
                    Kind::Asset if roll < 60 => {
                        ("Revaluation", "Property:Value Change", rng.around(20_000))
                    }
                    Kind::Asset => ("Depreciation", "Property:Value Change", -rng.around(15_000)),
                    Kind::Loan if roll < 90 => (
                        "Principal Payment",
                        "[Everyday Checking]",
                        rng.around(-opening * 8 / (10 * n as i64)),
                    ),
                    Kind::Loan => (
                        "Interest Charged",
                        "Interest:Loans",
                        -rng.around(-opening / (20 * n as i64)),
                    ),
                };
                balance += amount;
                let num = if matches!(kind, Kind::Checking) && amount < 0 && rng.below(6) == 0 {
                    check += 1;
                    Some(check)
                } else {
                    None
                };
                txns.push(Txn {
                    day,
                    num,
                    payee,
                    category,
                    amount,
                    balance,
                    cleared: day < FIRST_DAY + HISTORY_DAYS - 30,
                });
            }
            Account {
                name,
                group,
                txns,
                balance,
            }
        })
        .collect()
}

fn fmt_cents(cents: i64) -> String {
    let neg = cents < 0;
    let abs = cents.unsigned_abs();
    let whole = (abs / 100).to_string();
    let mut grouped = String::new();
    for (i, ch) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{}{grouped}.{:02}", if neg { "-" } else { "" }, abs % 100)
}

/// Howard Hinnant's `civil_from_days`.
fn fmt_day(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y}-{m:02}-{d:02}")
}

/// Quicken 2012-style top-level tabs. Charts/reports are out of scope, so these are placeholders.
const TABS: &[&str] = &[
    "Home",
    "Bills",
    "Spending",
    "Planning",
    "Investing",
    "Property & Debt",
];

// ---------------------------------------------------------------------------------------------
// Variants and toggles

#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav {
    Tab(usize),
    Account(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    A,
    B,
    C,
    D,
}

impl Variant {
    const ALL: [Self; 4] = [Self::A, Self::B, Self::C, Self::D];

    fn key(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
            Self::C => "c",
            Self::D => "d",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|v| v.key().eq_ignore_ascii_case(key))
    }

    fn label(self) -> &'static str {
        match self {
            Self::A => "A · Quicken-modern (13px, blue-grey)",
            Self::B => "B · Neutral flat (14px, grey)",
            Self::C => "C · Airy (15px, white, teal)",
            Self::D => "D · Ledger (13px, green-bar)",
        }
    }

    fn step(self, delta: isize) -> Self {
        let n = Self::ALL.len() as isize;
        let i = Self::ALL.iter().position(|v| *v == self).unwrap_or(0) as isize;
        Self::ALL[(i + delta).rem_euclid(n) as usize]
    }
}

/// The key that flips each entry of `TOGGLES`, in order, while no text field has focus.
const TOGGLE_KEYS: [(egui::Key, &str); 10] = [
    (egui::Key::Num1, "1"),
    (egui::Key::Num2, "2"),
    (egui::Key::Num3, "3"),
    (egui::Key::Num4, "4"),
    (egui::Key::Num5, "5"),
    (egui::Key::Num6, "6"),
    (egui::Key::Num7, "7"),
    (egui::Key::Num8, "8"),
    (egui::Key::Num9, "9"),
    (egui::Key::Num0, "0"),
];

/// The key that picks each variant, in `Variant::ALL` order.
const VARIANT_KEYS: [(egui::Key, &str); 4] = [
    (egui::Key::A, "A"),
    (egui::Key::B, "B"),
    (egui::Key::C, "C"),
    (egui::Key::D, "D"),
];

/// (key, label). Every toggle applies to every variant. `scale`, `font` and `grid` cycle
/// through their values instead of flipping.
const TOGGLES: &[(&str, &str)] = &[
    ("dark", "Dark theme"),
    ("dense", "Dense rows (the Preference)"),
    ("zebra", "Alternating rows"),
    ("scale", "Display scale"),
    ("font", "Font"),
    ("mono_money", "Monospace money digits"),
    ("grid", "Gridlines"),
    ("colour_negatives", "Colour negative amounts"),
    (
        "solid_selection",
        "Selection: solid accent (off: soft tint)",
    ),
    ("tinted_chrome", "Tinted chrome (off: flat like the body)"),
];

const SCALES: [u32; 4] = [100, 125, 150, 200];

/// Left inset for the Register's first column, so its text doesn't touch the edge.
const FIRST_COL_INSET: f32 = 6.0;

/// (key, label) for the gridline cycle: rules between rows, between columns, or both.
const GRIDS: [(&str, &str); 4] = [
    ("off", "off"),
    ("rows", "horizontal"),
    ("columns", "vertical"),
    ("both", "both"),
];

fn is_cycle(key: &str) -> bool {
    matches!(key, "scale" | "font" | "grid")
}

/// (key, label, system font file). `None` is egui's bundled default.
const FONTS: [(&str, &str, Option<&str>); 3] = [
    ("default", "egui default", None),
    (
        "liberation",
        "Liberation Sans",
        Some("/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf"),
    ),
    (
        "dejavu",
        "DejaVu Sans",
        Some("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
    ),
];

struct Toggles {
    dark: bool,
    dense: bool,
    zebra: bool,
    /// Index into `SCALES`.
    scale: usize,
    /// Index into `FONTS`.
    font: usize,
    mono_money: bool,
    /// Index into `GRIDS`.
    grid: usize,
    colour_negatives: bool,
    solid_selection: bool,
    tinted_chrome: bool,
    // Fixed by the shell-layout decisions; not in the dock.
    status_bar: bool,
    account_bar: bool,
    group_totals: bool,
    net_worth_footer: bool,
    register_tabs: bool,
}

impl Toggles {
    fn get_mut(&mut self, key: &str) -> Option<&mut bool> {
        Some(match key {
            "dark" => &mut self.dark,
            "dense" => &mut self.dense,
            "zebra" => &mut self.zebra,
            "mono_money" => &mut self.mono_money,
            "colour_negatives" => &mut self.colour_negatives,
            "solid_selection" => &mut self.solid_selection,
            "tinted_chrome" => &mut self.tinted_chrome,
            "status_bar" => &mut self.status_bar,
            "account_bar" => &mut self.account_bar,
            "group_totals" => &mut self.group_totals,
            "net_worth_footer" => &mut self.net_worth_footer,
            "register_tabs" => &mut self.register_tabs,
            _ => return None,
        })
    }

    /// The current value as logged: a bool, or the name of the scale or font.
    fn describe(&mut self, key: &str) -> serde_json::Value {
        match key {
            "scale" => json!(SCALES[self.scale]),
            "font" => json!(FONTS[self.font].0),
            "grid" => json!(GRIDS[self.grid].0),
            _ => json!(self.get_mut(key).map(|v| *v)),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The look: per-variant sizes and palettes, applied to egui's style whenever an input changes.

/// Sizes that differ between variants. Density picks `row` or `row_dense`.
#[derive(Clone, Copy)]
struct Look {
    body: f32,
    row: f32,
    row_dense: f32,
    /// Vertical gap between stacked widgets: normal, dense.
    gap: (f32, f32),
    radius: u8,
    /// Draw a hairline under every Register row even when gridlines are off.
    hairlines: bool,
}

fn look(v: Variant) -> Look {
    match v {
        Variant::A => Look {
            body: 13.0,
            row: 20.0,
            row_dense: 16.0,
            gap: (4.0, 2.0),
            radius: 0,
            hairlines: false,
        },
        Variant::B => Look {
            body: 14.0,
            row: 24.0,
            row_dense: 18.0,
            gap: (5.0, 3.0),
            radius: 3,
            hairlines: false,
        },
        Variant::C => Look {
            body: 15.0,
            row: 30.0,
            row_dense: 22.0,
            gap: (7.0, 4.0),
            radius: 6,
            hairlines: true,
        },
        Variant::D => Look {
            body: 13.0,
            row: 22.0,
            row_dense: 17.0,
            gap: (4.0, 2.0),
            radius: 2,
            hairlines: false,
        },
    }
}

#[derive(Clone, Copy)]
struct Palette {
    chrome: egui::Color32,
    body: egui::Color32,
    header: egui::Color32,
    zebra: egui::Color32,
    grid: egui::Color32,
    accent: egui::Color32,
    soft: egui::Color32,
    text: egui::Color32,
    negative: egui::Color32,
}

fn palette(v: Variant, dark: bool) -> Palette {
    let c = |r, g, b| egui::Color32::from_rgb(r, g, b);
    let rows = match (v, dark) {
        // chrome, body, header, zebra, grid, accent, soft selection, text, negative
        (Variant::A, false) => [
            (220, 228, 238),
            (255, 255, 255),
            (205, 215, 230),
            (238, 243, 250),
            (200, 208, 220),
            (31, 94, 168),
            (200, 218, 240),
            (28, 32, 38),
            (190, 30, 30),
        ],
        (Variant::B, false) => [
            (236, 237, 239),
            (252, 252, 252),
            (228, 229, 232),
            (244, 244, 246),
            (218, 219, 222),
            (38, 110, 210),
            (209, 224, 247),
            (30, 30, 32),
            (200, 40, 40),
        ],
        (Variant::C, false) => [
            (247, 248, 249),
            (255, 255, 255),
            (240, 243, 244),
            (249, 250, 251),
            (230, 233, 235),
            (0, 128, 128),
            (210, 238, 236),
            (33, 37, 41),
            (196, 48, 48),
        ],
        (Variant::D, false) => [
            (230, 236, 228),
            (252, 254, 250),
            (210, 224, 206),
            (226, 240, 222),
            (190, 206, 186),
            (46, 125, 50),
            (190, 222, 190),
            (24, 34, 24),
            (176, 32, 32),
        ],
        (Variant::A, true) => [
            (30, 36, 46),
            (22, 26, 34),
            (40, 50, 66),
            (28, 34, 44),
            (52, 60, 74),
            (96, 160, 235),
            (40, 70, 110),
            (222, 228, 236),
            (255, 110, 110),
        ],
        (Variant::B, true) => [
            (36, 37, 40),
            (28, 29, 31),
            (46, 47, 51),
            (34, 35, 38),
            (58, 59, 63),
            (95, 150, 240),
            (45, 70, 110),
            (226, 226, 228),
            (255, 115, 115),
        ],
        (Variant::C, true) => [
            (30, 32, 34),
            (22, 24, 26),
            (36, 38, 41),
            (27, 29, 31),
            (44, 46, 49),
            (70, 200, 190),
            (30, 80, 80),
            (225, 228, 230),
            (255, 120, 120),
        ],
        (Variant::D, true) => [
            (30, 38, 30),
            (22, 28, 22),
            (38, 52, 38),
            (30, 42, 30),
            (50, 64, 50),
            (110, 190, 110),
            (40, 80, 42),
            (220, 232, 220),
            (255, 115, 115),
        ],
    };
    let [
        chrome,
        body,
        header,
        zebra,
        grid,
        accent,
        soft,
        text,
        negative,
    ] = rows.map(|(r, g, b)| c(r, g, b));
    Palette {
        chrome,
        body,
        header,
        zebra,
        grid,
        accent,
        soft,
        text,
        negative,
    }
}

/// How money is drawn; shared by the Account bar and the Register.
#[derive(Clone, Copy)]
struct MoneyStyle {
    colour: bool,
    mono: bool,
    negative: egui::Color32,
}

impl MoneyStyle {
    fn text(self, cents: i64) -> RichText {
        let rt = RichText::new(fmt_cents(cents));
        let rt = if self.mono { rt.monospace() } else { rt };
        if cents < 0 && self.colour {
            rt.color(self.negative)
        } else {
            rt
        }
    }
}

/// The inputs `Proto::sync_look` compares against what it last applied.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Applied {
    variant: Variant,
    dark: bool,
    dense: bool,
    solid_selection: bool,
    font: usize,
    scale: usize,
}

fn text_styles(body: f32) -> std::collections::BTreeMap<egui::TextStyle, egui::FontId> {
    use egui::{
        FontFamily::{Monospace, Proportional},
        FontId, TextStyle,
    };
    [
        (TextStyle::Small, FontId::new(body - 2.0, Proportional)),
        (TextStyle::Body, FontId::new(body, Proportional)),
        (TextStyle::Button, FontId::new(body, Proportional)),
        (TextStyle::Heading, FontId::new(body + 5.0, Proportional)),
        (TextStyle::Monospace, FontId::new(body, Monospace)),
    ]
    .into()
}

fn build_style(look: Look, pal: Palette, dark: bool, dense: bool, solid: bool) -> egui::Style {
    let mut style = egui::Style {
        visuals: if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        },
        ..Default::default()
    };
    style.interaction.selectable_labels = false;
    style.text_styles = text_styles(look.body);
    let row = if dense { look.row_dense } else { look.row };
    let gap = if dense { look.gap.1 } else { look.gap.0 };
    style.spacing.interact_size.y = row;
    style.spacing.item_spacing = egui::vec2(8.0, gap);
    style.spacing.button_padding = egui::vec2(if dense { 6.0 } else { 8.0 }, gap.min(3.0));
    style.spacing.menu_margin = egui::Margin::same(if dense { 4 } else { 6 });
    let radius = egui::CornerRadius::same(look.radius);
    let v = &mut style.visuals;
    v.panel_fill = pal.body;
    v.window_fill = pal.body;
    v.extreme_bg_color = pal.body;
    v.faint_bg_color = pal.zebra;
    v.error_fg_color = pal.negative;
    v.hyperlink_color = pal.accent;
    v.window_corner_radius = egui::CornerRadius::same(look.radius.saturating_add(2));
    v.menu_corner_radius = radius;
    v.window_stroke = egui::Stroke::new(1.0, pal.grid);
    let (sel_bg, sel_fg) = if solid {
        (pal.accent, egui::Color32::WHITE)
    } else {
        (pal.soft, pal.text)
    };
    v.selection.bg_fill = sel_bg;
    v.selection.stroke = egui::Stroke::new(1.0, sel_fg);
    let hover = pal.soft.gamma_multiply(0.6);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = radius;
        w.fg_stroke.color = pal.text;
    }
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, pal.grid);
    v.widgets.inactive.weak_bg_fill = pal.header;
    v.widgets.inactive.bg_fill = pal.header;
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, pal.grid);
    v.widgets.hovered.weak_bg_fill = hover;
    v.widgets.hovered.bg_fill = hover;
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, pal.accent);
    v.widgets.active.weak_bg_fill = pal.soft;
    v.widgets.active.bg_fill = pal.soft;
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0, pal.accent);
    v.widgets.open.weak_bg_fill = pal.soft;
    style
}

// ---------------------------------------------------------------------------------------------
// App state

struct Proto {
    h: Harness,
    accounts: Vec<Account>,
    variant: Variant,
    t: Toggles,
    nav: Nav,
    collapsed: [bool; 4],
    filter: String,
    /// Variant C: open register tabs (account indices), in order.
    open_registers: Vec<usize>,
    /// Selected Register row per account; shared across variants.
    selected_row: HashMap<usize, usize>,
    jump_to_row: Option<usize>,
    /// The register the Accounts tab (variant D) returns to.
    last_account: usize,
    /// Variant D folded the Account bar on leaving Accounts, so returning unfolds it.
    d_auto_folded: bool,
    /// The docked prototype controls (F4).
    dock_open: bool,
    /// Last action, shown in the Status bar so every click is visible.
    status: String,
    /// What the style, fonts and zoom were last built from; `None` before the first frame.
    applied: Option<Applied>,
    pal: Palette,
}

impl Proto {
    fn new(args: &Args) -> Self {
        let variant = args
            .variant
            .as_deref()
            .and_then(Variant::from_key)
            .unwrap_or(Variant::A);
        let mut h = Harness::new(args);
        h.event("variant", json!({ "to": variant.key(), "via": "start" }));
        Self {
            h,
            accounts: generate(),
            variant,
            t: Toggles {
                dark: false,
                dense: false,
                zebra: true,
                scale: 0,
                font: 0,
                mono_money: false,
                grid: 0,
                colour_negatives: true,
                solid_selection: false,
                tinted_chrome: true,
                status_bar: true,
                account_bar: true,
                group_totals: true,
                net_worth_footer: true,
                register_tabs: false,
            },
            nav: Nav::Account(0),
            collapsed: [false; 4],
            filter: String::new(),
            open_registers: vec![0, 8],
            selected_row: HashMap::new(),
            jump_to_row: None,
            last_account: 0,
            d_auto_folded: false,
            dock_open: true,
            status: "Ready".to_owned(),
            applied: None,
            pal: palette(variant, false),
        }
    }

    fn did(&mut self, what: impl Into<String>) {
        self.status = what.into();
    }

    fn emit(&mut self, command: &str, source: &str) {
        self.h.event(
            "command",
            json!({ "would_emit": command, "source": source }),
        );
        self.did(format!("{source} → {command}"));
    }

    fn set_variant(&mut self, to: Variant, via: &str) {
        if to == self.variant {
            return;
        }
        self.h.event(
            "variant",
            json!({ "from": self.variant.key(), "to": to.key(), "via": via }),
        );
        self.variant = to;
        self.h.screenshot(format!("variant-{}", to.key()));
    }

    fn set_toggle(&mut self, key: &str, value: Option<bool>, via: &str) {
        if is_cycle(key) {
            let (len, cur) = match key {
                "scale" => (SCALES.len(), self.t.scale),
                "font" => (FONTS.len(), self.t.font),
                _ => (GRIDS.len(), self.t.grid),
            };
            self.set_choice(key, (cur + 1) % len, via);
            return;
        }
        let Some(slot) = self.t.get_mut(key) else {
            self.h.event("control_unknown", json!({ "toggle": key }));
            return;
        };
        let new = value.unwrap_or(!*slot);
        if *slot == new && via != "click" && via != "menu" {
            return;
        }
        *slot = new;
        self.h
            .event("toggle", json!({ "key": key, "value": new, "via": via }));
        self.h
            .screenshot(format!("toggle-{key}-{}", if new { "on" } else { "off" }));
    }

    /// Select entry `index` of the `scale`, `font` or `grid` cycle.
    fn set_choice(&mut self, key: &str, index: usize, via: &str) {
        let name = match key {
            "scale" => {
                self.t.scale = index;
                SCALES[index].to_string()
            }
            "font" => {
                self.t.font = index;
                FONTS[index].0.to_owned()
            }
            _ => {
                self.t.grid = index;
                GRIDS[index].0.to_owned()
            }
        };
        let value = self.t.describe(key);
        self.h
            .event("toggle", json!({ "key": key, "value": value, "via": via }));
        self.h.screenshot(format!("toggle-{key}-{name}"));
    }

    /// Rebuild the style, fonts and zoom when one of their inputs changed since the last frame.
    fn sync_look(&mut self, ctx: &egui::Context) {
        let want = Applied {
            variant: self.variant,
            dark: self.t.dark,
            dense: self.t.dense,
            solid_selection: self.t.solid_selection,
            font: self.t.font,
            scale: self.t.scale,
        };
        let had = self.applied;
        if had == Some(want) {
            return;
        }
        self.applied = Some(want);
        self.pal = palette(want.variant, want.dark);
        let theme = if want.dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        let style = build_style(
            look(want.variant),
            self.pal,
            want.dark,
            want.dense,
            want.solid_selection,
        );
        ctx.set_style_of(egui::Theme::Dark, style.clone());
        ctx.set_style_of(egui::Theme::Light, style);
        ctx.set_theme(theme);
        if had.is_none_or(|h| h.font != want.font) {
            self.load_font(ctx, want.font);
        }
        if had.is_none_or(|h| h.scale != want.scale) {
            ctx.set_zoom_factor(SCALES[want.scale] as f32 / 100.0);
        }
        self.h.event(
            "look_applied",
            json!({
                "variant": want.variant.key(),
                "dark": want.dark,
                "dense": want.dense,
                "font": FONTS[want.font].0,
                "scale": SCALES[want.scale],
                "body_px": look(want.variant).body,
                "row_px": if want.dense { look(want.variant).row_dense } else { look(want.variant).row },
                "pixels_per_point": ctx.pixels_per_point(),
            }),
        );
    }

    /// Put the chosen system font first in the proportional family; egui's own stay as fallback.
    fn load_font(&mut self, ctx: &egui::Context, index: usize) {
        let mut defs = egui::FontDefinitions::default();
        if let Some(path) = FONTS[index].2 {
            match std::fs::read(path) {
                Ok(bytes) => {
                    defs.font_data.insert(
                        "proto_font".to_owned(),
                        std::sync::Arc::new(egui::FontData::from_owned(bytes)),
                    );
                    if let Some(list) = defs.families.get_mut(&egui::FontFamily::Proportional) {
                        list.insert(0, "proto_font".to_owned());
                    }
                }
                Err(err) => self.h.event(
                    "font_missing",
                    json!({ "font": FONTS[index].0, "error": err.to_string() }),
                ),
            }
        }
        ctx.set_fonts(defs);
    }

    fn chrome_frame(&self, ui: &Ui) -> egui::Frame {
        let fill = if self.t.tinted_chrome {
            self.pal.chrome
        } else {
            self.pal.body
        };
        egui::Frame::side_top_panel(ui.style())
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, self.pal.grid))
    }

    fn body_frame(&self, ui: &Ui) -> egui::Frame {
        egui::Frame::central_panel(ui.style()).fill(self.pal.body)
    }

    fn money(&self) -> MoneyStyle {
        MoneyStyle {
            colour: self.t.colour_negatives,
            mono: self.t.mono_money,
            negative: self.pal.negative,
        }
    }

    fn open_account(&mut self, i: usize, via: &str) {
        self.nav = Nav::Account(i);
        self.last_account = i;
        if !self.open_registers.contains(&i) {
            self.open_registers.push(i);
        }
        self.h.event(
            "navigate",
            json!({ "to": self.accounts[i].name, "kind": "register", "via": via }),
        );
        self.did(format!("Opened register: {}", self.accounts[i].name));
    }

    fn open_tab(&mut self, t: usize, via: &str) {
        self.nav = Nav::Tab(t);
        self.h.event(
            "navigate",
            json!({ "to": TABS[t], "kind": "tab", "via": via }),
        );
        self.did(format!("Tab → {}", TABS[t]));
    }

    fn apply_control(&mut self, ctx: &egui::Context, cmd: &str) {
        let (verb, rest) = cmd
            .split_once(' ')
            .map_or((cmd, ""), |(v, r)| (v, r.trim()));
        match verb {
            "variant" => match Variant::from_key(rest) {
                Some(v) => self.set_variant(v, "control"),
                None => self.h.event("control_unknown", json!({ "command": cmd })),
            },
            "toggle" => {
                let (key, value) = rest
                    .split_once(' ')
                    .map_or((rest, None), |(k, v)| (k, Some(v.trim() == "on")));
                self.set_toggle(key, value, "control");
            }
            "account" => {
                let found = rest
                    .parse::<usize>()
                    .ok()
                    .filter(|i| *i < self.accounts.len())
                    .or_else(|| {
                        let needle = rest.to_lowercase();
                        self.accounts
                            .iter()
                            .position(|a| a.name.to_lowercase().contains(&needle))
                    });
                match found {
                    Some(i) => self.open_account(i, "control"),
                    None => self.h.event("control_unknown", json!({ "command": cmd })),
                }
            }
            "tab" => {
                let found = rest
                    .parse::<usize>()
                    .ok()
                    .filter(|i| *i < TABS.len())
                    .or_else(|| {
                        let needle = rest.to_lowercase();
                        TABS.iter().position(|t| t.to_lowercase().contains(&needle))
                    });
                match found {
                    Some(t) => self.open_tab(t, "control"),
                    None => self.h.event("control_unknown", json!({ "command": cmd })),
                }
            }
            "row" => match (rest.parse::<usize>(), self.nav) {
                (Ok(r), Nav::Account(i)) => {
                    self.jump_to_row = Some(r);
                    self.selected_row.insert(i, r);
                }
                _ => self.h.event("control_unknown", json!({ "command": cmd })),
            },
            "scale" => match SCALES.iter().position(|s| s.to_string() == rest) {
                Some(i) => self.set_choice("scale", i, "control"),
                None => self.h.event("control_unknown", json!({ "command": cmd })),
            },
            "font" => match FONTS.iter().position(|f| f.0 == rest) {
                Some(i) => self.set_choice("font", i, "control"),
                None => self.h.event("control_unknown", json!({ "command": cmd })),
            },
            "grid" => match GRIDS.iter().position(|g| g.0 == rest) {
                Some(i) => self.set_choice("grid", i, "control"),
                None => self.h.event("control_unknown", json!({ "command": cmd })),
            },
            "reload" => {
                self.accounts = generate();
                self.h.event("reload", json!({ "seed": SEED }));
            }
            "screenshot" => self
                .h
                .screenshot(if rest.is_empty() { "control" } else { rest }),
            "dock" => self.set_dock(rest != "off", "control"),
            "quit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            _ => self.h.event("control_unknown", json!({ "command": cmd })),
        }
    }

    fn group_total(&self, group: Group) -> i64 {
        self.accounts
            .iter()
            .filter(|a| a.group == group)
            .map(|a| a.balance)
            .sum()
    }

    fn net_worth(&self) -> i64 {
        self.accounts.iter().map(|a| a.balance).sum()
    }

    fn nav_title(&self) -> &'static str {
        match self.nav {
            Nav::Account(i) => self.accounts[i].name,
            Nav::Tab(t) => TABS[t],
        }
    }
}

impl eframe::App for Proto {
    fn on_exit(&mut self) {
        self.h.event("session_end", json!({}));
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.sync_look(&ctx);
        if let Some(cmd) = self.h.begin_frame(&ctx) {
            self.apply_control(&ctx, &cmd);
        }
        if !ctx.egui_wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                self.set_variant(self.variant.step(-1), "key");
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                self.set_variant(self.variant.step(1), "key");
            }
            for (v, (k, _)) in Variant::ALL.into_iter().zip(VARIANT_KEYS) {
                if ctx.input(|i| i.key_pressed(k) && i.modifiers.is_none()) {
                    self.set_variant(v, "key");
                }
            }
            for (&(key, _), (k, _)) in TOGGLES.iter().zip(TOGGLE_KEYS) {
                if ctx.input(|i| i.key_pressed(k) && i.modifiers.is_none()) {
                    self.set_toggle(key, None, "key");
                }
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F4)) {
            self.set_dock(!self.dock_open, "key");
        }
        self.dock(ui);
        self.shell(ui);

        self.h.end_frame(&ctx);
    }
}

// ---------------------------------------------------------------------------------------------
// Shared pieces (the menus are part of the question, so all variants share one proposal)

fn command_name(label: &str) -> String {
    label
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect()
}

impl Proto {
    fn item(&mut self, ui: &mut Ui, path: &str, label: &str, shortcut: &str, kind: &str) {
        let button = if shortcut.is_empty() {
            Button::new(label)
        } else {
            Button::new(label).shortcut_text(shortcut)
        };
        let r = ui.add(button);
        self.h.track(&r, format!("menu: {path} → {label}"));
        if r.clicked() {
            self.emit(
                &format!("{kind}::{}", command_name(label)),
                &format!("menu {path}"),
            );
            ui.close();
        }
    }

    fn menu_toggle(&mut self, ui: &mut Ui, key: &str, label: &str) {
        let mut v = *self.t.get_mut(key).unwrap();
        let r = ui.checkbox(&mut v, label);
        self.h.track(&r, format!("menu: View → {label}"));
        if r.changed() {
            self.set_toggle(key, Some(v), "menu");
        }
    }

    fn menus(&mut self, ui: &mut Ui) {
        let app = "AppCommand";
        let dom = "DomainCommand";
        ui.menu_button("File", |ui| {
            self.item(ui, "File", "New Data File…", "", app);
            self.item(ui, "File", "Open Data File…", "Ctrl+O", app);
            ui.menu_button("Open Recent", |ui| {
                self.item(ui, "File ▸ Open Recent", "household.balanced", "", app);
            });
            ui.separator();
            self.item(ui, "File", "Back Up…", "Ctrl+B", app);
            self.item(ui, "File", "Restore Backup…", "", app);
            ui.separator();
            ui.menu_button("Import", |ui| {
                self.item(ui, "File ▸ Import", "QIF File…", "", app);
                self.item(ui, "File ▸ Import", "OFX / QFX File…", "", app);
                self.item(ui, "File ▸ Import", "CSV File…", "", app);
            });
            ui.menu_button("Export", |ui| {
                self.item(ui, "File ▸ Export", "QIF File…", "", app);
                self.item(ui, "File ▸ Export", "CSV File…", "", app);
            });
            ui.separator();
            self.item(ui, "File", "Print Register…", "Ctrl+P", app);
            ui.separator();
            self.item(ui, "File", "Exit", "Alt+F4", app);
        });
        ui.menu_button("Edit", |ui| {
            self.item(ui, "Edit", "Undo", "Ctrl+Z", app);
            self.item(ui, "Edit", "Redo", "Ctrl+Y", app);
            ui.separator();
            self.item(ui, "Edit", "Cut", "Ctrl+X", app);
            self.item(ui, "Edit", "Copy", "Ctrl+C", app);
            self.item(ui, "Edit", "Paste", "Ctrl+V", app);
            ui.separator();
            ui.menu_button("Transaction", |ui| {
                self.item(ui, "Edit ▸ Transaction", "New", "Ctrl+N", app);
                self.item(ui, "Edit ▸ Transaction", "Delete", "Ctrl+D", dom);
                self.item(ui, "Edit ▸ Transaction", "Split…", "Ctrl+S", app);
                self.item(ui, "Edit ▸ Transaction", "Mark Cleared", "Ctrl+M", dom);
            });
            ui.separator();
            self.item(ui, "Edit", "Find…", "Ctrl+F", app);
            self.item(ui, "Edit", "Find and Replace…", "Ctrl+H", app);
        });
        ui.menu_button("View", |ui| {
            self.menu_toggle(ui, "account_bar", "Account Bar");
            self.menu_toggle(ui, "status_bar", "Status Bar");
            ui.separator();
            ui.menu_button("Go To", |ui| {
                for (t, tab) in TABS.iter().enumerate() {
                    let r = ui.button(*tab);
                    self.h.track(&r, format!("menu: View ▸ Go To → {tab}"));
                    if r.clicked() {
                        self.open_tab(t, "menu");
                        ui.close();
                    }
                }
            });
            self.item(ui, "View", "Back", "Alt+←", app);
            self.item(ui, "View", "Forward", "Alt+→", app);
            ui.separator();
            self.menu_toggle(ui, "dark", "Dark Theme");
            self.item(ui, "View", "Zoom In", "Ctrl++", app);
            self.item(ui, "View", "Zoom Out", "Ctrl+-", app);
        });
        ui.menu_button("Tools", |ui| {
            self.item(ui, "Tools", "Account List…", "Ctrl+A", app);
            self.item(ui, "Tools", "Category List…", "Ctrl+Shift+C", app);
            self.item(ui, "Tools", "Payee List…", "", app);
            self.item(ui, "Tools", "Scheduled Transactions…", "Ctrl+J", app);
            ui.separator();
            self.item(ui, "Tools", "Reconcile Account…", "Ctrl+R", app);
            self.item(ui, "Tools", "Calculator", "", app);
            ui.separator();
            self.item(ui, "Tools", "Preferences…", "Ctrl+,", app);
        });
        ui.menu_button("Help", |ui| {
            self.item(ui, "Help", "Balanced Help", "F1", app);
            self.item(ui, "Help", "Keyboard Shortcuts", "", app);
            ui.separator();
            self.item(ui, "Help", "About Balanced", "", app);
        });
    }

    fn account_context_menu(&mut self, ui: &mut Ui, i: usize) {
        let name = self.accounts[i].name;
        let path = format!("account menu ({name})");
        for (label, cmd) in [
            ("Open Register", "AppCommand::OpenRegister"),
            (
                "Open in Detached Window",
                "AppCommand::OpenDetachedRegister",
            ),
            ("Reconcile…", "AppCommand::OpenReconcileDialog"),
            ("Edit Account Details…", "AppCommand::OpenEditAccountDialog"),
            ("Hide from Account Bar", "DomainCommand::HideAccount"),
            ("Delete Account…", "AppCommand::OpenDeleteAccountDialog"),
        ] {
            if label == "Delete Account…" {
                ui.separator();
            }
            let r = ui.button(label);
            self.h.track(&r, format!("{path} → {label}"));
            if r.clicked() {
                if label == "Open Register" {
                    self.open_account(i, "context menu");
                }
                self.emit(&format!("{cmd} {{ account: {name:?} }}"), &path);
                ui.close();
            }
        }
    }

    fn money_label(ui: &mut Ui, money: MoneyStyle, cents: i64) -> egui::Response {
        ui.label(money.text(cents))
    }

    /// One account in the Account bar: the whole row is the click target and the highlight.
    fn account_row(
        ui: &mut Ui,
        money: MoneyStyle,
        selected: bool,
        name: &str,
        balance: i64,
    ) -> egui::Response {
        let height = ui.spacing().interact_size.y;
        let (rect, row) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::click());
        let v = ui.visuals();
        if selected {
            ui.painter().rect_filled(rect, 2.0, v.selection.bg_fill);
        } else if row.hovered() {
            ui.painter()
                .rect_filled(rect, 2.0, v.widgets.hovered.weak_bg_fill);
        }
        let font = egui::TextStyle::Body.resolve(ui.style());
        let text = if selected {
            v.selection.stroke.color
        } else {
            v.text_color()
        };
        let money_colour = if balance < 0 && money.colour {
            v.error_fg_color
        } else {
            text
        };
        let money_font = if money.mono {
            egui::TextStyle::Monospace.resolve(ui.style())
        } else {
            font.clone()
        };
        let pad = egui::vec2(14.0, 0.0);
        ui.painter().text(
            rect.left_center() + pad,
            egui::Align2::LEFT_CENTER,
            name,
            font.clone(),
            text,
        );
        ui.painter().text(
            rect.right_center() - egui::vec2(2.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            fmt_cents(balance),
            money_font,
            money_colour,
        );
        row
    }

    /// The Account bar body: groups with totals, accounts with balances, right-click menus.
    fn account_groups(&mut self, ui: &mut Ui) {
        let gap = if self.t.dense { 2.0 } else { 8.0 };
        let money = self.money();
        let needle = self.filter.to_lowercase();
        for (g, group) in Group::ALL.into_iter().enumerate() {
            let total = self.group_total(group);
            let show_total = self.t.group_totals;
            let arrow = if self.collapsed[g] { "⏵" } else { "⏷" };
            let (header, ()) = Sides::new().show(
                ui,
                |ui| {
                    ui.add(
                        Button::new(RichText::new(format!("{arrow} {}", group.name())).strong())
                            .frame(false),
                    )
                },
                |ui| {
                    if show_total {
                        Self::money_label(ui, money, total);
                    }
                },
            );
            self.h
                .track(&header, format!("account group: {}", group.name()));
            if header.clicked() {
                self.collapsed[g] = !self.collapsed[g];
                self.h.event(
                    "group_collapse",
                    json!({ "group": group.name(), "collapsed": self.collapsed[g] }),
                );
            }
            header.context_menu(|ui| {
                for label in [
                    "Add Account to Group…",
                    "Collapse All Groups",
                    "Expand All Groups",
                ] {
                    let r = ui.button(label);
                    self.h
                        .track(&r, format!("group menu ({}) → {label}", group.name()));
                    if r.clicked() {
                        match label {
                            "Collapse All Groups" => self.collapsed = [true; 4],
                            "Expand All Groups" => self.collapsed = [false; 4],
                            _ => self.emit("AppCommand::OpenNewAccountDialog", "group menu"),
                        }
                        ui.close();
                    }
                }
            });
            if self.collapsed[g] {
                ui.add_space(gap);
                continue;
            }
            let rows: Vec<(usize, &'static str, i64)> = self
                .accounts
                .iter()
                .enumerate()
                .filter(|(_, a)| {
                    a.group == group
                        && (needle.is_empty() || a.name.to_lowercase().contains(&needle))
                })
                .map(|(i, a)| (i, a.name, a.balance))
                .collect();
            for (i, name, balance) in rows {
                let selected = self.nav == Nav::Account(i);
                let row = Self::account_row(ui, money, selected, name, balance);
                self.h.track(&row, format!("account bar: {name}"));
                if row.clicked() {
                    self.open_account(i, "account bar");
                }
                row.context_menu(|ui| self.account_context_menu(ui, i));
            }
            ui.add_space(gap);
        }
    }

    fn net_worth_row(&mut self, ui: &mut Ui) {
        let nw = self.net_worth();
        Sides::new().show(
            ui,
            |ui| ui.label(RichText::new("Net Worth").strong()),
            |ui| ui.label(RichText::new(fmt_cents(nw)).strong()),
        );
        let r = ui.button("➕ Add Account…");
        self.h.track(&r, "account bar footer: Add Account");
        if r.clicked() {
            self.emit("AppCommand::OpenNewAccountDialog", "account bar footer");
        }
    }

    /// The Account bar as a resizable left panel, with the optional Net Worth footer.
    fn account_bar(&mut self, ui: &mut Ui, id: &str, top: impl FnOnce(&mut Self, &mut Ui)) {
        let chrome = self.chrome_frame(ui);
        let bar = Panel::left(Id::new(id))
            .resizable(true)
            .default_size(270.0)
            .min_size(180.0)
            .frame(chrome)
            .show(ui, |ui| {
                if self.t.net_worth_footer {
                    Panel::bottom(Id::new((id, "footer")))
                        .frame(chrome.inner_margin(egui::Margin::symmetric(8, 6)))
                        .show(ui, |ui| self.net_worth_row(ui));
                }
                CentralPanel::default()
                    .frame(egui::Frame::new())
                    .show(ui, |ui| {
                        top(self, ui);
                        egui::ScrollArea::vertical().show(ui, |ui| self.account_groups(ui));
                    });
            });
        self.h.region(bar.response.rect, "account bar");
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        if !self.t.status_bar {
            return;
        }
        let right = if self.t.net_worth_footer {
            format!("{} accounts", self.accounts.len())
        } else {
            format!(
                "Net Worth {}   ·   {} accounts",
                fmt_cents(self.net_worth()),
                self.accounts.len()
            )
        };
        let status = self.status.clone();
        let chrome = self.chrome_frame(ui);
        let bar = Panel::bottom("status_bar").frame(chrome).show(ui, |ui| {
            Sides::new().show(
                ui,
                |ui| ui.label(status),
                |ui| ui.label(RichText::new(right).weak()),
            );
        });
        self.h.region(bar.response.rect, "status bar");
    }

    /// What the Main view shows for the current navigation.
    fn main_view_body(&mut self, ui: &mut Ui) {
        match self.nav {
            Nav::Account(i) => self.register(ui, i),
            Nav::Tab(t) => {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.heading(TABS[t]);
                    ui.label(
                        RichText::new("Placeholder: the tab's own view is a later feature map.")
                            .weak(),
                    );
                });
            }
        }
    }

    /// A rough Register stub; the real grid is "Prototype the Register grid".
    /// Payee and Category share the spare width; below the minimum the table scrolls sideways.
    fn register(&mut self, ui: &mut Ui, account: usize) {
        let out = egui::ScrollArea::horizontal()
            .id_salt(("register_h", account))
            .show(ui, |ui| self.register_table(ui, account));
        self.h.region(out.inner_rect, "register");
    }

    fn register_table(&mut self, ui: &mut Ui, account: usize) {
        // The scroll viewport: header and grid painters reach past their cells but not past this.
        let view = ui.clip_rect();
        let lk = look(self.variant);
        let row_h = if self.t.dense { lk.row_dense } else { lk.row };
        let pal = self.pal;
        let money = self.money();
        let grid_rows = matches!(GRIDS[self.t.grid].0, "rows" | "both");
        let grid_cols = matches!(GRIDS[self.t.grid].0, "columns" | "both");
        let hair = lk.hairlines;
        let zebra = self.t.zebra;
        let n = self.accounts[account].txns.len();
        let selected = self.selected_row.get(&account).copied();
        let mut builder = TableBuilder::new(ui)
            .id_salt(("register", account))
            .striped(zebra)
            // Not resizable: its drag handles draw column lines that would mask the gridlines toggle.
            .resizable(false)
            .sense(Sense::click())
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::exact(88.0))
            .column(Column::exact(48.0))
            .column(Column::remainder().at_least(90.0).clip(true))
            .column(Column::remainder().at_least(90.0).clip(true))
            .column(Column::exact(24.0))
            .column(Column::exact(96.0))
            .column(Column::exact(96.0))
            .column(Column::exact(104.0));
        if let Some(r) = self.jump_to_row.take() {
            builder = builder.scroll_to_row(r.min(n.saturating_sub(1)), Some(Align::Center));
        }
        let mut clicked: Option<(usize, &'static str)> = None;
        let mut names: Vec<(egui::Response, String)> = Vec::new();
        let out = builder
            .header(row_h + 2.0, |mut h| {
                for title in [
                    "Date", "Num", "Payee", "Category", "Clr", "Payment", "Deposit", "Balance",
                ] {
                    h.col(|ui| {
                        // The cell's painter is clipped to the cell, which leaves gaps at the
                        // column spacing; paint the band through an unclipped layer painter.
                        let pad = ui.spacing().item_spacing / 2.0;
                        let bg = ui.max_rect().expand2(pad);
                        ui.ctx()
                            .layer_painter(ui.layer_id())
                            .with_clip_rect(bg.intersect(view))
                            .rect_filled(bg, 0.0, pal.header);
                        if matches!(title, "Payment" | "Deposit" | "Balance") {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.strong(title);
                            });
                        } else {
                            if title == "Date" {
                                ui.add_space(FIRST_COL_INSET);
                            }
                            ui.strong(title);
                        }
                    });
                }
            })
            .body(|body| {
                let txns = &self.accounts[account].txns;
                body.rows(row_h, n, |mut row| {
                    let r = row.index();
                    let t = &txns[r];
                    row.set_selected(selected == Some(r));
                    let cells: [(&'static str, String, bool); 8] = [
                        ("date", fmt_day(t.day), false),
                        (
                            "num",
                            t.num.map(|n| n.to_string()).unwrap_or_default(),
                            false,
                        ),
                        ("payee", t.payee.to_owned(), false),
                        ("category", t.category.to_owned(), false),
                        (
                            "cleared",
                            if t.cleared {
                                "c".to_owned()
                            } else {
                                String::new()
                            },
                            false,
                        ),
                        (
                            "payment",
                            if t.amount < 0 {
                                fmt_cents(-t.amount)
                            } else {
                                String::new()
                            },
                            false,
                        ),
                        (
                            "deposit",
                            if t.amount >= 0 {
                                fmt_cents(t.amount)
                            } else {
                                String::new()
                            },
                            false,
                        ),
                        ("balance", fmt_cents(t.balance), t.balance < 0),
                    ];
                    let is_selected = selected == Some(r);
                    for (col, text, negative) in cells {
                        let right = matches!(col, "payment" | "deposit" | "balance");
                        let (_, resp) = row.col(|ui| {
                            let rect = ui.max_rect();
                            let mut rt = RichText::new(&text);
                            if right && money.mono {
                                rt = rt.monospace();
                            }
                            if is_selected {
                                rt = rt.color(ui.visuals().selection.stroke.color);
                            } else if negative && money.colour {
                                rt = rt.color(pal.negative);
                            }
                            if right {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(rt);
                                });
                            } else {
                                if col == "date" {
                                    ui.add_space(FIRST_COL_INSET);
                                }
                                ui.label(rt);
                            }
                            // The cell's painter clips the gaps between cells, where the lines
                            // belong, so widen its clip by half the spacing. Row rules go on the
                            // top edge: the next row's stripe would cover a bottom one.
                            let line = egui::Stroke::new(1.0, pal.grid);
                            let pad = ui.spacing().item_spacing / 2.0;
                            let painter = ui
                                .ctx()
                                .layer_painter(ui.layer_id())
                                // One extra point, or a line on the gap's edge is clipped away.
                                .with_clip_rect(
                                    ui.clip_rect().expand2(pad + egui::Vec2::splat(1.0)).intersect(view),
                                );
                            // Snap to a pixel centre: at fractional scales an unsnapped 1 pt line
                            // straddles two pixels and antialiases almost to nothing.
                            let ppp = ui.ctx().pixels_per_point();
                            let snap = |v: f32| ((v * ppp).floor() + 0.5) / ppp;
                            let line = egui::Stroke::new(1.0 / ppp, line.color);
                            // Each cell rules its own left edge: a rule on the right edge would be
                            // covered by the next cell's stripe, which egui_extras paints across
                            // the gap.
                            if grid_cols && col != "date" {
                                painter.vline(
                                    snap(rect.left() - pad.x),
                                    rect.y_range().expand(pad.y),
                                    line,
                                );
                            }
                            if grid_rows || hair {
                                painter.hline(
                                    rect.x_range().expand(pad.x),
                                    snap(rect.top() - pad.y),
                                    line,
                                );
                            }
                        });
                        if resp.clicked() {
                            clicked = Some((r, col));
                        }
                        names.push((resp, format!("{col} cell, row {r}")));
                    }
                    row.response().context_menu(|ui| {
                        for label in [
                            "Edit",
                            "Split…",
                            "Mark Cleared",
                            "Duplicate",
                            "Go to Transfer",
                            "Delete",
                        ] {
                            if label == "Delete" {
                                ui.separator();
                            }
                            if ui.button(label).clicked() {
                                clicked = Some((r, label));
                                ui.close();
                            }
                        }
                    });
                });
            });
        for (resp, name) in &names {
            self.h.track(resp, name.clone());
        }
        self.h.scroll("register", out.state.offset.y, row_h + 3.0);
        if let Some((r, what)) = clicked {
            self.selected_row.insert(account, r);
            if what.chars().next().is_some_and(char::is_uppercase) {
                let cmd = format!(
                    "{}::{} {{ row: {r} }}",
                    if what == "Delete" || what == "Mark Cleared" {
                        "DomainCommand"
                    } else {
                        "AppCommand"
                    },
                    command_name(what)
                );
                self.emit(&cmd, "register row menu");
            }
        }
    }
}

impl Proto {
    /// B's Account bar: a « button folds it to a thin rail, » brings it back.
    fn foldable_account_bar(&mut self, ui: &mut Ui, prefix: &str) {
        if self.t.account_bar {
            self.account_bar(ui, &format!("{prefix}_account_bar"), |this, ui| {
                Sides::new().show(
                    ui,
                    |ui| ui.label(RichText::new("ACCOUNTS").small().weak()),
                    |ui| {
                        let r = ui.small_button("«").on_hover_text("Collapse Account bar");
                        this.h.track(&r, "account bar: collapse «");
                        if r.clicked() {
                            this.set_toggle("account_bar", Some(false), "click");
                        }
                    },
                );
            });
        } else {
            Panel::left(Id::new((prefix, "rail")))
                .exact_size(28.0)
                .resizable(false)
                .frame(self.chrome_frame(ui))
                .show(ui, |ui| {
                    let r = ui.small_button("»").on_hover_text("Show Account bar");
                    self.h.track(&r, "account bar rail: expand »");
                    if r.clicked() {
                        self.set_toggle("account_bar", Some(true), "click");
                    }
                });
        }
    }

    /// B's context header: what is shown, plus actions that depend on it.
    fn context_header(&mut self, ui: &mut Ui) {
        // Buttons claim the right first; the title, balance and count truncate into what's left.
        let header = ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let actions: &[&str] = match self.nav {
                    Nav::Account(_) => &["More…", "Reconcile", "Filter", "+ Transaction"],
                    Nav::Tab(_) => &["More…", "Customize"],
                };
                for label in actions {
                    let r = ui.button(*label);
                    self.h.track(&r, format!("context header: {label}"));
                    if r.clicked() {
                        self.emit(
                            &format!("AppCommand::{}", command_name(label)),
                            "context header",
                        );
                    }
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    // The balance can't truncate, so clip it rather than draw under the buttons.
                    ui.shrink_clip_rect(ui.max_rect());
                    ui.add(egui::Label::new(RichText::new(self.nav_title()).heading()).truncate());
                    if let Nav::Account(i) = self.nav {
                        Self::money_label(ui, self.money(), self.accounts[i].balance);
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!(
                                    "{} transactions",
                                    self.accounts[i].txns.len()
                                ))
                                .weak(),
                            )
                            .truncate(),
                        );
                    }
                });
            });
        });
        self.h.region(header.response.rect, "context header");
    }
}

impl Proto {
    /// C's closeable register tabs, one per opened account.
    fn register_tabs_row(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let mut close = None;
            for (pos, &i) in self.open_registers.clone().iter().enumerate() {
                let name = self.accounts[i].name;
                let r = ui.selectable_label(self.nav == Nav::Account(i), name);
                self.h.track(&r, format!("register tab: {name}"));
                if r.clicked() {
                    self.open_account(i, "register tab");
                }
                let x = ui.small_button("×");
                self.h.track(&x, format!("register tab close: {name}"));
                if r.middle_clicked() || x.clicked() {
                    close = Some(pos);
                }
                ui.separator();
            }
            if let Some(pos) = close {
                let i = self.open_registers.remove(pos);
                self.h.event(
                    "register_tab_closed",
                    json!({ "account": self.accounts[i].name }),
                );
                if self.nav == Nav::Account(i) {
                    self.nav = self
                        .open_registers
                        .last()
                        .map_or(Nav::Tab(0), |&j| Nav::Account(j));
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------------------------
// Variant D: the pick from the first session. Menu row, then the tab row on its own with an
// Accounts tab after Home; no toolbar. Accounts shows the open register under B's context
// header, with C's register tabs as a preference. B's foldable Account bar.

impl Proto {
    fn shell(&mut self, ui: &mut Ui) {
        let chrome = self.chrome_frame(ui);
        let menu = Panel::top("d_menu").frame(chrome).show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| self.menus(ui));
        });
        self.h.region(menu.response.rect, "menu row");
        let tabs = Panel::top("d_tabs").frame(chrome).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                for (pos, tab) in TABS.iter().enumerate() {
                    self.d_tab(ui, Some(pos), tab);
                    if pos == 0 {
                        self.d_tab(ui, None, "Accounts");
                    }
                }
            });
        });
        self.h.region(tabs.response.rect, "tab row");
        self.status_bar(ui);
        self.foldable_account_bar(ui, "d");
        let body = self.body_frame(ui);
        CentralPanel::default().frame(body).show(ui, |ui| {
            if matches!(self.nav, Nav::Account(_)) && self.t.register_tabs {
                self.register_tabs_row(ui);
                ui.separator();
            }
            self.context_header(ui);
            ui.separator();
            self.main_view_body(ui);
        });
    }

    /// One tab in D's tab row; `None` is the Accounts tab, which reopens the last register.
    fn d_tab(&mut self, ui: &mut Ui, pos: Option<usize>, label: &str) {
        let selected = match pos {
            Some(t) => self.nav == Nav::Tab(t),
            None => matches!(self.nav, Nav::Account(_)),
        };
        let r = ui.selectable_label(
            selected,
            RichText::new(label).size(look(self.variant).body + 2.0),
        );
        self.h.track(&r, format!("tab: {label}"));
        if r.clicked() {
            match pos {
                Some(t) => {
                    if matches!(self.nav, Nav::Account(_)) && self.t.account_bar {
                        self.set_toggle("account_bar", Some(false), "auto: left Accounts");
                        self.d_auto_folded = true;
                    }
                    self.open_tab(t, "tab");
                }
                None => {
                    if std::mem::take(&mut self.d_auto_folded) && !self.t.account_bar {
                        self.set_toggle("account_bar", Some(true), "auto: back to Accounts");
                    }
                    let i = self.last_account;
                    self.open_account(i, "tab: Accounts");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Prototype tooling is docked as the outermost right-hand panel: each variant lays out in the
// rest of the window, so the dock never covers the design. F4 collapses it to a thin edge.

impl Proto {
    fn set_dock(&mut self, open: bool, via: &str) {
        self.dock_open = open;
        self.h.event("dock", json!({ "open": open, "via": via }));
    }

    fn dock(&mut self, ui: &mut Ui) {
        let frame = {
            let v = ui.visuals();
            egui::Frame::side_top_panel(ui.style())
                .fill(v.warn_fg_color.gamma_multiply(0.10).blend(v.panel_fill))
                .stroke(egui::Stroke::new(1.0, v.warn_fg_color))
        };
        if !self.dock_open {
            Panel::right("prototype_dock_edge")
                .exact_size(22.0)
                .resizable(false)
                .frame(frame)
                .show(ui, |ui| {
                    let r = ui
                        .small_button("⏴")
                        .on_hover_text("Show prototype controls (F4)");
                    self.h.track(&r, "dock: expand");
                    if r.clicked() {
                        self.set_dock(true, "click");
                    }
                });
            return;
        }
        Panel::right("prototype_dock")
            .exact_size(270.0)
            .resizable(false)
            .frame(frame)
            .show(ui, |ui| {
                Sides::new().show(
                    ui,
                    |ui| ui.label(RichText::new("PROTOTYPE").strong()),
                    |ui| {
                        let r = ui.small_button("⏵").on_hover_text("Collapse (F4)");
                        self.h.track(&r, "dock: collapse");
                        if r.clicked() {
                            self.set_dock(false, "click");
                        }
                    },
                );
                ui.separator();
                ui.label(RichText::new("Variant").strong());
                for (v, (_, hint)) in Variant::ALL.into_iter().zip(VARIANT_KEYS) {
                    let r = ui.selectable_label(self.variant == v, format!("{hint}   {}", v.label()));
                    self.h.track(&r, format!("dock: variant {}", v.key()));
                    if r.clicked() {
                        self.set_variant(v, "click");
                    }
                }
                ui.separator();
                ui.label(RichText::new("Toggles").strong());
                for (&(key, label), (_, hint)) in TOGGLES.iter().zip(TOGGLE_KEYS) {
                    if is_cycle(key) {
                        let value = match key {
                            "scale" => format!("{}%", SCALES[self.t.scale]),
                            "font" => FONTS[self.t.font].1.to_owned(),
                            _ => GRIDS[self.t.grid].1.to_owned(),
                        };
                        let r = ui.button(format!("{hint}   {label}: {value}  (click to cycle)"));
                        self.h.track(&r, format!("dock: toggle {key}"));
                        if r.clicked() {
                            self.set_toggle(key, None, "click");
                        }
                        continue;
                    }
                    let mut value = *self.t.get_mut(key).unwrap();
                    let r = ui.checkbox(&mut value, format!("{hint}   {label}"));
                    self.h.track(&r, format!("dock: toggle {key}"));
                    if r.changed() {
                        self.set_toggle(key, Some(value), "click");
                    }
                }
                ui.separator();
                self.h.dock_spotlight(ui);
                ui.separator();
                ui.label(
                    RichText::new("A B C D or ◀ / ▶ variant · 1–0 toggles · F2 note · F3 marker · F4 hide this dock · F5 spotlight")
                        .small()
                        .weak(),
                );
            });
    }
}

fn main() -> eframe::Result {
    let args = Args::from_env();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Balanced — PROTOTYPE theming and density")
            .with_inner_size([1400.0, 860.0]),
        ..Default::default()
    };
    eframe::run_native(
        "balanced_theming_density_prototype",
        options,
        Box::new(move |_cc| Ok(Box::new(Proto::new(&args)))),
    )
}
