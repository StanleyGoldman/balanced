//! The keymap: one fixed table of each user action's default chords. It feeds both
//! the Shortcut text beside menu items and the per-frame dispatcher.

use balanced_i18n::Localizer;
use eframe::egui::os::OperatingSystem;
use eframe::egui::{self, Key, KeyboardShortcut, ModifierNames, Modifiers};

use crate::i18n::fl;
use crate::user_action::UserAction;

/// A chord and the user action it carries out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Binding {
    pub(crate) chord: KeyboardShortcut,
    pub(crate) action: UserAction,
}

impl Binding {
    const fn new(modifiers: Modifiers, key: Key, action: UserAction) -> Self {
        Self {
            chord: KeyboardShortcut::new(modifiers, key),
            action,
        }
    }

    /// How many modifiers the chord holds. Ctrl, Cmd and Command count once
    /// between them.
    fn modifier_count(self) -> usize {
        let Modifiers { alt, shift, .. } = self.chord.modifiers;
        [alt, shift, self.holds_command()]
            .into_iter()
            .filter(|&held| held)
            .count()
    }

    /// Whether the chord holds Ctrl, Cmd or Command.
    fn holds_command(self) -> bool {
        let Modifiers {
            ctrl,
            mac_cmd,
            command,
            ..
        } = self.chord.modifiers;
        ctrl || mac_cmd || command
    }

    /// Whether the chord also types into a focused text field: Space, or a key
    /// whose egui name is one character (a letter, digit or symbol), with no
    /// modifier but Shift.
    fn types_text(self) -> bool {
        let key = self.chord.logical_key;
        let types = key == Key::Space || key.name().chars().count() == 1;
        types && !self.chord.modifiers.alt && !self.holds_command()
    }

    /// Whether a focused text field keeps the chord for itself: Ctrl with Z or Y (undo
    /// and redo, Shift or not), or with C, X, V or A (copy, cut, paste, select all).
    fn edits_text(self) -> bool {
        const KEYS: [Key; 6] = [Key::Z, Key::Y, Key::C, Key::X, Key::V, Key::A];
        self.holds_command() && !self.chord.modifiers.alt && KEYS.contains(&self.chord.logical_key)
    }
}

/// Every default chord. An action with several chords lists the one its menu item
/// shows first.
pub(crate) const KEYMAP: &[Binding] = &[
    Binding::new(Modifiers::COMMAND, Key::W, UserAction::CloseRegisterTab),
    Binding::new(Modifiers::COMMAND, Key::F4, UserAction::CloseRegisterTab),
    Binding::new(Modifiers::COMMAND, Key::Tab, UserAction::NextRegisterTab),
    Binding::new(
        Modifiers::COMMAND.plus(Modifiers::SHIFT),
        Key::Tab,
        UserAction::PreviousRegisterTab,
    ),
    Binding::new(Modifiers::COMMAND, Key::N, UserAction::NewTransaction),
    Binding::new(Modifiers::COMMAND, Key::D, UserAction::DeleteTransaction),
    Binding::new(Modifiers::COMMAND, Key::Q, UserAction::Quit),
    Binding::new(Modifiers::NONE, Key::F6, UserAction::SwitchPane),
    Binding::new(Modifiers::SHIFT, Key::F6, UserAction::SwitchPane),
];

/// The chord `action`'s menu item shows: its first chord in [`KEYMAP`].
pub(crate) fn chord(action: UserAction) -> Option<KeyboardShortcut> {
    KEYMAP
        .iter()
        .find(|binding| binding.action == action)
        .map(|binding| binding.chord)
}

/// `chord` as Shortcut text beside a menu item, such as `Ctrl+Q`, with the
/// modifier names in the text language. On macOS, Command shows as Cmd and Alt as
/// Option.
pub(crate) fn shortcut_text(
    chord: KeyboardShortcut,
    i18n: &Localizer,
    os: OperatingSystem,
) -> String {
    let alt = fl!(i18n, "menu-modifier-alt");
    let ctrl = fl!(i18n, "menu-modifier-ctrl");
    let shift = fl!(i18n, "menu-modifier-shift");
    let mac_cmd = fl!(i18n, "menu-modifier-cmd");
    let mac_alt = fl!(i18n, "menu-modifier-option");
    let names = ModifierNames {
        is_short: false,
        alt: &alt,
        ctrl: &ctrl,
        shift: &shift,
        mac_cmd: &mac_cmd,
        mac_alt: &mac_alt,
        concat: "+",
    };
    chord.format(&names, os == OperatingSystem::Mac)
}

/// The user actions whose chords were pressed this frame, in the order they were
/// matched. Each matched key press is consumed, so nothing else acts on it.
///
/// egui matches a chord even when Shift or Alt is held beyond it, so chords with
/// more modifiers are tried first: Ctrl+Shift+Z matches before Ctrl+Z can. A chord
/// that types a character acts only while no widget has focus, so a focused field
/// gets the letter. A focused text field also keeps its editing chords, such as
/// Ctrl+Z and Ctrl+C.
pub(crate) fn dispatch(ctx: &egui::Context, bindings: &[Binding]) -> Vec<UserAction> {
    let typing = ctx.egui_wants_keyboard_input();
    let editing = ctx.text_edit_focused();
    let mut bindings: Vec<Binding> = bindings
        .iter()
        .copied()
        .filter(|binding| !(typing && binding.types_text()))
        .filter(|binding| !(editing && binding.edits_text()))
        .collect();
    bindings.sort_by_key(|binding| std::cmp::Reverse(binding.modifier_count()));
    ctx.input_mut(|input| {
        bindings
            .iter()
            .filter(|binding| input.consume_shortcut(&binding.chord))
            .map(|binding| binding.action)
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use balanced_i18n::Localizer;
    use icu_locale::{Locale, locale};

    use super::*;

    fn localizer(locale: &Locale) -> Localizer {
        Localizer::new(&locale.id, locale).unwrap()
    }

    const CTRL_Q: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Q);

    #[test]
    fn shortcut_text_names_ctrl_in_english() {
        let i18n = localizer(&locale!("en-US"));

        assert_eq!(shortcut_text(CTRL_Q, &i18n, OperatingSystem::Nix), "Ctrl+Q");
    }

    #[test]
    fn shortcut_text_names_ctrl_in_german() {
        let i18n = localizer(&locale!("de-DE"));

        assert_eq!(shortcut_text(CTRL_Q, &i18n, OperatingSystem::Nix), "Strg+Q");
    }

    #[test]
    fn shortcut_text_names_command_on_macos() {
        let i18n = localizer(&locale!("en-US"));

        assert_eq!(shortcut_text(CTRL_Q, &i18n, OperatingSystem::Mac), "Cmd+Q");
    }

    #[test]
    fn shortcut_text_names_every_modifier_in_german() {
        let i18n = localizer(&locale!("de-DE"));
        let chord = KeyboardShortcut::new(
            Modifiers::COMMAND | Modifiers::ALT | Modifiers::SHIFT,
            Key::Z,
        );

        assert_eq!(
            shortcut_text(chord, &i18n, OperatingSystem::Nix),
            "Strg+Alt+Umschalt+Z"
        );
    }

    /// The actions `dispatch` returns for one frame in which `key` is pressed with
    /// `modifiers` held, after `setup` has drawn a frame of its own.
    fn dispatched(
        bindings: &[Binding],
        mut setup: impl FnMut(&mut egui::Ui),
        modifiers: Modifiers,
        key: Key,
    ) -> Vec<UserAction> {
        let ctx = egui::Context::default();
        // Each frame's font textures are dropped unused: the tests paint nothing.
        ctx.run_ui(egui::RawInput::default(), |ui| setup(ui))
            .textures_delta
            .clear();
        let input = egui::RawInput {
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        };
        let mut actions = Vec::new();
        ctx.run_ui(input, |ui| actions = dispatch(ui.ctx(), bindings))
            .textures_delta
            .clear();
        actions
    }

    #[test]
    fn dispatch_matches_a_modified_chord_before_a_plainer_one() {
        // The plainer chord comes first, so table order alone would match it.
        let bindings = [
            Binding::new(Modifiers::COMMAND, Key::Z, UserAction::About),
            Binding::new(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::Z,
                UserAction::Quit,
            ),
        ];

        let actions = dispatched(
            &bindings,
            |_| {},
            Modifiers::COMMAND | Modifiers::SHIFT,
            Key::Z,
        );

        assert_eq!(actions, [UserAction::Quit]);
    }

    const BARE_T: [Binding; 1] = [Binding::new(Modifiers::NONE, Key::T, UserAction::Quit)];

    #[test]
    fn dispatch_acts_on_a_bare_letter_while_no_text_field_has_focus() {
        let actions = dispatched(&BARE_T, |_| {}, Modifiers::NONE, Key::T);

        assert_eq!(actions, [UserAction::Quit]);
    }

    #[test]
    fn dispatch_leaves_a_bare_letter_to_a_focused_text_field() {
        let mut text = String::new();
        let focus_a_field = |ui: &mut egui::Ui| {
            ui.text_edit_singleline(&mut text).request_focus();
        };

        let actions = dispatched(&BARE_T, focus_a_field, Modifiers::NONE, Key::T);

        assert_eq!(actions, []);
    }

    #[test]
    fn dispatch_leaves_the_editing_chords_to_a_focused_text_field() {
        let mut text = String::new();
        let mut focus_a_field = |ui: &mut egui::Ui| {
            ui.text_edit_singleline(&mut text).request_focus();
        };
        let chords = [
            (Modifiers::COMMAND, Key::Z),
            (Modifiers::COMMAND | Modifiers::SHIFT, Key::Z),
            (Modifiers::COMMAND, Key::Y),
            (Modifiers::COMMAND, Key::C),
            (Modifiers::COMMAND, Key::X),
            (Modifiers::COMMAND, Key::V),
            (Modifiers::COMMAND, Key::A),
        ];

        let dispatched: Vec<Vec<UserAction>> = chords
            .into_iter()
            .map(|(modifiers, key)| {
                let bindings = [Binding::new(modifiers, key, UserAction::Quit)];
                dispatched(&bindings, &mut focus_a_field, modifiers, key)
            })
            .collect();

        assert_eq!(dispatched, vec![Vec::<UserAction>::new(); chords.len()]);
    }

    #[test]
    fn dispatch_leaves_space_to_a_focused_text_field() {
        let bindings = [Binding::new(Modifiers::NONE, Key::Space, UserAction::Quit)];
        let mut text = String::new();
        let focus_a_field = |ui: &mut egui::Ui| {
            ui.text_edit_singleline(&mut text).request_focus();
        };

        let actions = dispatched(&bindings, focus_a_field, Modifiers::NONE, Key::Space);

        assert_eq!(actions, []);
    }
}
