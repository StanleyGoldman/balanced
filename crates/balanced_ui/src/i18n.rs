// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! The `fl!` macro: every user-visible string goes through it.

/// Looks up a message for the user interface.
///
/// The first argument is a `&Localizer`, then the message id, an optional attribute,
/// and named arguments: `fl!(i18n, "account-bar-total", amount = total)`. Ids and
/// argument names are checked against the `en-US` file at compile time.
macro_rules! fl {
    ($i18n:expr, $($message:tt)+) => {{
        let i18n: &balanced_i18n::Localizer = $i18n;
        i18n.finish(i18n_embed_fl::fl!(i18n.loader(), $($message)+))
    }};
}

pub(crate) use fl;

#[cfg(test)]
mod tests {
    use balanced_i18n::Localizer;

    #[test]
    fn looks_up_a_message() {
        let i18n = Localizer::for_tests();

        assert_eq!(fl!(&i18n, "menu-file"), "File");
    }

    #[test]
    fn passes_named_arguments() {
        let i18n = Localizer::for_tests();

        assert_eq!(
            fl!(&i18n, "register-transaction-count", count = 2),
            "2 transactions"
        );
    }

    #[test]
    fn looks_up_an_attribute() {
        let i18n = Localizer::for_tests();

        assert_eq!(
            fl!(&i18n, "account-bar-total", "tooltip"),
            "Sum of the balances of the accounts shown in the Account bar"
        );
    }

    #[test]
    fn brackets_pseudo_locale_messages() {
        let i18n = Localizer::pseudo().unwrap();

        assert_eq!(fl!(&i18n, "menu-file"), "[Ƒîîĺéé]");
    }
}
