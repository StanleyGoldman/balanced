// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The undo log: the entries on its two stacks, as `read` returns them and as the
//! `undo_entry` rows hold them.

use balanced_store::{Stack, Store};
use rusqlite::Connection;
use tempfile::TempDir;

/// A new data file in a temporary directory, with the store over it. The directory lives
/// as long as the test holds it.
fn new_file() -> (TempDir, std::path::PathBuf, Store) {
    let dir = tempfile::tempdir().expect("the temp dir is writable");
    let path = dir.path().join("Household.balanced");
    let store = Store::create(&path, "USD").expect("a new path takes a data file");
    (dir, path, store)
}

/// The entries `store` reads back: the undo stack's, then the redo stack's, oldest first.
fn stacks(store: &Store) -> (Vec<String>, Vec<String>) {
    let contents = store.read().expect("the file reads");
    (contents.undo, contents.redo)
}

fn entry(n: u32) -> String {
    format!(r#"{{"delete_account":{{"account":{n}}}}}"#)
}

fn push(store: &Store, stack: Stack, entries: &[u32]) {
    for &n in entries {
        store
            .push_entry(stack, &entry(n))
            .expect("the file takes a JSON entry");
    }
}

#[test]
fn a_new_file_has_an_empty_undo_log() {
    let (_dir, _path, store) = new_file();

    assert_eq!(stacks(&store), (Vec::new(), Vec::new()));
}

#[test]
fn pushed_entries_read_back_oldest_first_on_their_own_stack() {
    let (_dir, _path, store) = new_file();

    push(&store, Stack::Undo, &[1, 2]);
    push(&store, Stack::Redo, &[3]);

    assert_eq!(stacks(&store), (vec![entry(1), entry(2)], vec![entry(3)]));
}

#[test]
fn popping_takes_the_latest_entry_off_its_stack() {
    let (_dir, _path, store) = new_file();
    push(&store, Stack::Undo, &[1, 2]);
    push(&store, Stack::Redo, &[3, 4]);

    store.pop_entry(Stack::Undo).unwrap();

    assert_eq!(stacks(&store), (vec![entry(1)], vec![entry(3), entry(4)]));
}

#[test]
fn an_entry_pushed_after_a_pop_is_the_latest() {
    let (_dir, _path, store) = new_file();
    push(&store, Stack::Undo, &[1, 2]);
    store.pop_entry(Stack::Undo).unwrap();

    push(&store, Stack::Undo, &[3]);

    assert_eq!(stacks(&store).0, vec![entry(1), entry(3)]);
}

#[test]
fn dropping_the_oldest_keeps_the_rest_in_order() {
    let (_dir, _path, store) = new_file();
    push(&store, Stack::Undo, &[1, 2, 3]);
    push(&store, Stack::Redo, &[4]);

    store.drop_oldest_entry(Stack::Undo).unwrap();
    push(&store, Stack::Undo, &[5]);

    assert_eq!(
        stacks(&store),
        (vec![entry(2), entry(3), entry(5)], vec![entry(4)])
    );
}

#[test]
fn clearing_a_stack_leaves_the_other() {
    let (_dir, _path, store) = new_file();
    push(&store, Stack::Undo, &[1]);
    push(&store, Stack::Redo, &[2, 3]);

    store.clear_stack(Stack::Redo).unwrap();

    assert_eq!(stacks(&store), (vec![entry(1)], Vec::new()));
}

#[test]
fn the_rows_hold_each_entry_on_its_stack_in_order() {
    let (_dir, path, store) = new_file();
    push(&store, Stack::Undo, &[1, 2]);
    push(&store, Stack::Redo, &[3]);
    drop(store);

    let connection = Connection::open(&path).unwrap();
    let rows = connection
        .prepare("SELECT stack, command FROM undo_entry ORDER BY stack DESC, position")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<(String, String)>, _>>()
        .unwrap();

    assert_eq!(
        rows,
        [
            ("undo".to_owned(), entry(1)),
            ("undo".to_owned(), entry(2)),
            ("redo".to_owned(), entry(3)),
        ]
    );
}
