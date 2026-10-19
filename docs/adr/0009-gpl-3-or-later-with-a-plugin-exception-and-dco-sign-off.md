# GPL-3.0-or-later with a plugin exception, and DCO sign-off

Balanced is licensed `GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception`. The exception is an additional permission under GPLv3 section 7: a module that interacts with Balanced only through the **Extension Interface** may be distributed under any license, combined with Balanced. The Extension Interface is the set of crates and modules that `LICENSE-EXCEPTION.md` designates. The list starts empty, and only interfaces built for extensions are designated. Changes to Balanced itself stay under the GPL.

Contributors sign off each commit under the Developer Certificate of Origin. There is no CLA.

The goal is for changes to Balanced to stay open, while anyone, including the author, can ship closed extensions. The author plans paid business modules that load dynamically.

Decided in a grilling session on 2026-10-19. Built by [#299](https://github.com/StanleyGoldman/balanced/issues/299).

## Consequences

- **The exception ships from the first public commit.** Adding it after DCO contributions land would need every contributor's consent. A designated list is how it can ship before the extension API exists.
- **The exception is the only door kept open.** Without a CLA, contributed code can't be relicensed. Closing the core, dual-licensing it, or moving it to a source-available license is out once the first outside contribution merges. "Or later" still allows a move to a future GPL version.
- **Third parties have the same right as the author.** Anyone can ship a closed extension through the Extension Interface. No one, the author included, can ship a closed fork of the core.
- **The boundary is where the maintainer designates it.** Contributors accept that designation decides what counts as the Extension Interface. The exception text limits it to interfaces built for extensions, so internals can't be designated just to open them up.
- **Dependencies stay permissive.** `deny.toml`'s allowlist is unchanged. Apache-2.0 dependencies are compatible with GPLv3 but not GPLv2-only, which is one reason the license is v3.
- **The name isn't licensed.** `TRADEMARKS.md` reserves "Balanced" and its logo. A fork that distributes builds renames.
- **Docs, translations and assets made for Balanced use the code's license.** Third-party assets, such as the OFL fonts, keep their own.
- **The exception text is reviewed by a lawyer before the repo goes public.** It is the only custom legal wording in the scheme. The author arranges the review.
- **Loading a plugin is a separate decision.** Rust has no stable ABI, so how extensions load (`abi_stable`, a C ABI, WASM) is an architecture decision for the extension API, not part of this one.
- **CI moves to GitHub Actions after the repo goes public** ([#300](https://github.com/StanleyGoldman/balanced/issues/300)). The DCO and SPDX checks live in `cargo xtask`, so they don't depend on which CI runs them.

## Considered options

- **Plain GPLv3:** rejected. Under the FSF's reading, a plugin loaded into the process forms a combined work with the core. Once contributors' code is in the core, a closed plugin would have to be GPL too.
- **MPL-2.0:** rejected. Its copyleft applies file by file, so it allows closed extensions with no custom wording. But a fork could add closed files that reach into the internals, not only an extension API, which is weaker than "changes stay open".
- **AGPL-3.0:** rejected. Its network clause adds nothing for a desktop app.
- **A CLA:** rejected. Only a CLA would keep relicensing the core possible, and the extension goal doesn't need that. A CLA also costs contributor friction, a bot, and paperwork.
- **GPL-3.0-only:** rejected. Under the DCO, "or later" is the only route left to a newer GPL version.
- **Adding the exception once the extension API exists:** rejected. By then contributions under plain GPL would block it.
