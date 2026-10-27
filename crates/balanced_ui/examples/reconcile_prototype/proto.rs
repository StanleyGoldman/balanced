//! PROTOTYPE (#374): the harness, and the hooks the patched app modules call.
//! Throwaway.

use std::cell::RefCell;
use std::collections::HashMap;

use balanced_prototype_harness::Harness;
use eframe::egui::{Rect, Response};
use serde_json::{Value, json};

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

/// Names a larger area for the spotlight and the `click` verb.
pub fn region(rect: Rect, name: impl Into<String>) {
    let name = name.into();
    RECTS.with(|r| r.borrow_mut().insert(name.clone(), rect));
    with(|h| h.region(rect, name));
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

/// Logs the Command the real app would send.
pub fn would_emit(what: impl Into<String>, fields: Value) {
    let mut fields = fields;
    if !fields.is_object() {
        fields = json!({});
    }
    fields["would_emit"] = json!(what.into());
    log("command", fields);
}

pub fn screenshot(label: impl Into<String>) {
    with(|h| h.screenshot(label.into()));
}

/// A line the prototype shows over the window for a few seconds.
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
