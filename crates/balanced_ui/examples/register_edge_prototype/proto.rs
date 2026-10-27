//! PROTOTYPE (#483): the settings each step compares, the harness, and the hooks the
//! patched app modules call. Throwaway.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};

use balanced_prototype_harness::Harness;
use eframe::egui::{Rect, Response};
use serde_json::{Value, json};

/// One question's setting. Option 0 is always today's built behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    EnterKey,
    EnterInList,
    EscInList,
    ClrKeyboard,
    DiscardTab,
    UndoPrompt,
    SplitAfterPrompt,
    SplitCreate,
    SplitCreateCancelFocus,
    CtrlDReconciled,
    QuitReconciled,
    PayeeRecallAgain,
    Locale,
}

pub struct SettingDef {
    pub setting: Setting,
    pub id: &'static str,
    pub step: u8,
    pub ticket: u16,
    pub title: &'static str,
    pub options: &'static [&'static str],
}

pub const SETTINGS: [SettingDef; 13] = [
    SettingDef {
        setting: Setting::EnterKey,
        id: "enter",
        step: 1,
        ticket: 417,
        title: "Enter in an open cell",
        options: &[
            "records the row (today)",
            "moves to the next field; Ctrl+Enter records",
        ],
    },
    SettingDef {
        setting: Setting::EnterInList,
        id: "enter-list",
        step: 1,
        ticket: 417,
        title: "Enter in an open Payee/Category list",
        options: &[
            "picks the entry and records (today)",
            "picks the entry only",
        ],
    },
    SettingDef {
        setting: Setting::EscInList,
        id: "esc-list",
        step: 2,
        ticket: 455,
        title: "Esc with a list open",
        options: &[
            "1st closes the list, 2nd throws the row away (today)",
            "one Esc throws the row away",
        ],
    },
    SettingDef {
        setting: Setting::ClrKeyboard,
        id: "clr-keys",
        step: 3,
        ticket: 443,
        title: "Clr from the keyboard",
        options: &[
            "no way (today)",
            "cursor stops on Clr and Flag; Space toggles",
            "Mark Cleared / Uncleared in More",
            "Ctrl+Shift+C toggles Clr",
        ],
    },
    SettingDef {
        setting: Setting::DiscardTab,
        id: "discard-tab",
        step: 4,
        ticket: 464,
        title: "Leave-row prompt's Discard",
        options: &["in the Tab order (today)", "click only"],
    },
    SettingDef {
        setting: Setting::UndoPrompt,
        id: "undo-prompt",
        step: 5,
        ticket: 462,
        title: "Undo/Redo on a changed row",
        options: &[
            "prompt as built: Save then undo (today)",
            "prompt offers Discard and Continue editing only",
            "Save, then stop: no undo",
        ],
    },
    SettingDef {
        setting: Setting::SplitAfterPrompt,
        id: "split-after",
        step: 6,
        ticket: 473,
        title: "[Split] click on another row, after the prompt",
        options: &[
            "cursor moves only (today)",
            "Split dialog opens after Save/Discard",
        ],
    },
    SettingDef {
        setting: Setting::SplitCreate,
        id: "split-create",
        step: 7,
        ticket: 472,
        title: "Split dialog Category: a new name",
        options: &[
            "no Create row (today)",
            "Create row opens New Category over the Split dialog",
            "Done refuses with a hint",
        ],
    },
    SettingDef {
        setting: Setting::SplitCreateCancelFocus,
        id: "split-cancel-focus",
        step: 7,
        ticket: 472,
        title: "New Category cancelled: focus lands",
        options: &[
            "back in the line's Category, text kept",
            "on the line's Category, text selected",
        ],
    },
    SettingDef {
        setting: Setting::CtrlDReconciled,
        id: "ctrl-d",
        step: 8,
        ticket: 444,
        title: "Ctrl+D after changing a Reconciled amount",
        options: &[
            "delete confirm only (today)",
            "amount confirm, then delete confirm",
        ],
    },
    SettingDef {
        setting: Setting::QuitReconciled,
        id: "quit",
        step: 9,
        ticket: 445,
        title: "Quit (Ctrl+Q) after changing a Reconciled amount",
        options: &[
            "leave-row prompt; Save asks the amount confirm (today)",
            "throw the row's changes away silently",
            "save the amount, no prompt or confirm",
        ],
    },
    SettingDef {
        setting: Setting::PayeeRecallAgain,
        id: "recall",
        step: 10,
        ticket: 446,
        title: "Re-commit an unchanged draft Payee",
        options: &[
            "recall only on a change (today)",
            "recall each time, filling empty cells",
        ],
    },
    SettingDef {
        setting: Setting::Locale,
        id: "locale",
        step: 11,
        ticket: 469,
        title: "Language",
        options: &["en-US", "de-DE"],
    },
];

pub const STEPS: u8 = 11;

pub const VARIANTS: [(&str, &str); 4] = [
    ("A", "overlay strip (today)"),
    ("B", "compact icon buttons"),
    ("C", "strip below the row (over the next row)"),
    ("D", "overflow into More"),
];

static VALUES: [AtomicU8; SETTINGS.len()] = [const { AtomicU8::new(0) }; SETTINGS.len()];
static VARIANT: AtomicU8 = AtomicU8::new(0);

pub fn def(setting: Setting) -> &'static SettingDef {
    SETTINGS.iter().find(|d| d.setting == setting).unwrap()
}

fn index(setting: Setting) -> usize {
    SETTINGS.iter().position(|d| d.setting == setting).unwrap()
}

/// The option `setting` is on; 0 is today's behaviour.
pub fn get(setting: Setting) -> u8 {
    VALUES[index(setting)].load(Ordering::Relaxed)
}

pub fn is(setting: Setting, option: u8) -> bool {
    get(setting) == option
}

pub fn set(setting: Setting, option: u8) {
    let n = def(setting).options.len() as u8;
    VALUES[index(setting)].store(option % n, Ordering::Relaxed);
}

/// The #469 variant: 0 = A … 3 = D.
pub fn variant() -> u8 {
    VARIANT.load(Ordering::Relaxed)
}

pub fn set_variant(v: u8) {
    VARIANT.store(v % VARIANTS.len() as u8, Ordering::Relaxed);
}

/// A line the prototype shows over the window for a few seconds, for what the real
/// app would do but the prototype doesn't (closing the window).
pub fn notice(text: &str) {
    NOTICE.with(|n| *n.borrow_mut() = Some((text.to_owned(), std::time::Instant::now())));
    log("notice", json!({ "text": text }));
}

pub fn current_notice() -> Option<String> {
    NOTICE.with(|n| {
        n.borrow()
            .as_ref()
            .filter(|(_, at)| at.elapsed().as_secs() < 5)
            .map(|(text, _)| text.clone())
    })
}

thread_local! {
    static NOTICE: RefCell<Option<(String, std::time::Instant)>> = const { RefCell::new(None) };
    static HARNESS: RefCell<Option<Harness>> = const { RefCell::new(None) };
    static RECTS: RefCell<HashMap<String, Rect>> = RefCell::new(HashMap::new());
    static PREV_RECTS: RefCell<HashMap<String, Rect>> = RefCell::new(HashMap::new());
}

pub fn install(harness: Harness) {
    HARNESS.with(|h| *h.borrow_mut() = Some(harness));
}

/// Runs `f` on the harness. Panics if called re-entrantly.
pub fn with<R>(f: impl FnOnce(&mut Harness) -> R) -> R {
    HARNESS.with(|h| f(h.borrow_mut().as_mut().expect("harness installed")))
}

/// Names a widget for the log, the spotlight and the `click` control verb.
pub fn track(response: &Response, name: impl Into<String>) {
    let name = name.into();
    RECTS.with(|r| r.borrow_mut().insert(name.clone(), response.rect));
    with(|h| h.track(response, name));
}

/// The rect of the tracked widget whose name contains `name` (case-insensitive),
/// preferring an exact match.
pub fn find(name: &str) -> Option<(String, Rect)> {
    let lower = name.to_lowercase();
    let look = |r: &HashMap<String, Rect>| {
        r.iter()
            .find(|(n, _)| n.to_lowercase() == lower)
            .or_else(|| r.iter().find(|(n, _)| n.to_lowercase().contains(&lower)))
            .map(|(n, rect)| (n.clone(), *rect))
    };
    RECTS
        .with(|r| look(&r.borrow()))
        .or_else(|| PREV_RECTS.with(|r| look(&r.borrow())))
}

/// Starts a frame's names: last frame's stay findable until this frame draws.
pub fn clear_rects() {
    RECTS.with(|r| PREV_RECTS.with(|p| std::mem::swap(&mut *r.borrow_mut(), &mut *p.borrow_mut())));
    RECTS.with(|r| r.borrow_mut().clear());
}

/// Logs an app event.
pub fn log(kind: &str, fields: Value) {
    with(|h| h.event(kind, fields));
}

/// Logs the Command the app sends, and so what the real app would do.
pub fn would_emit(what: impl std::fmt::Debug) {
    let mut text = format!("{what:?}");
    if text.len() > 300 {
        text.truncate(300);
        text.push('…');
    }
    log("command", json!({ "would_emit": text }));
}

/// Logs a decision a patched module made under a setting.
pub fn decided(setting: Setting, what: &str) {
    let d = def(setting);
    log(
        "decided",
        json!({
            "setting": d.id,
            "option": d.options[get(setting) as usize],
            "what": what,
        }),
    );
}
