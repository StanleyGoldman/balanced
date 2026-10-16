#![expect(
    clippy::disallowed_methods,
    reason = "event lines carry wall-clock timestamps; the clock ban is for the app, whose `Clock` lives in `balanced_ui`"
)]

use std::io::Write;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

/// Build one event line: `ts_ms`, `t` and `event`, plus the fields of `fields` (a JSON object).
pub(crate) fn event_line(start: Instant, kind: &str, fields: Value) -> Value {
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let mut line = json!({
        "ts_ms": ts_ms,
        "t": (start.elapsed().as_secs_f64() * 1000.0).round() / 1000.0,
        "event": kind,
    });
    if let (Some(line), Value::Object(fields)) = (line.as_object_mut(), fields) {
        line.extend(fields);
    }
    line
}

/// Append one event line to the log and flush, so the agent's tail sees it at once.
pub(crate) fn write_event(out: &mut impl Write, start: Instant, kind: &str, fields: Value) {
    let line = event_line(start, kind, fields);
    writeln!(out, "{line}").and_then(|()| out.flush()).ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_carries_timestamps_kind_and_fields() {
        let line = event_line(
            Instant::now(),
            "click",
            json!({ "widget": "payee cell, row 3" }),
        );
        assert_eq!(line["event"], "click");
        assert_eq!(line["widget"], "payee cell, row 3");
        assert!(line["ts_ms"].as_u64().is_some_and(|ms| ms > 0));
        assert!(line["t"].as_f64().is_some());
    }

    #[test]
    fn write_appends_one_json_line() {
        let mut out = Vec::new();
        write_event(&mut out, Instant::now(), "marker", json!({ "n": 1 }));
        write_event(&mut out, Instant::now(), "session_end", json!({}));
        let text = String::from_utf8(out).unwrap();
        let lines: Vec<Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["n"], 1);
        assert_eq!(lines[1]["event"], "session_end");
    }
}
