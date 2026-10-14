//! Rebuilds this crate whenever a message file changes, so `fl!` checks every id and
//! argument against the current `en-US` file.

fn main() {
    println!("cargo::rerun-if-changed=../balanced_i18n/locales");
}
