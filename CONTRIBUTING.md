# Contributing

## License

Balanced is licensed under `GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception`:
the [GPL](LICENSE), version 3 or later, with the
[Balanced Plugin Exception](LICENSE-EXCEPTION.md). By contributing, you agree that
your contribution is licensed under those same terms. There is no contributor
license agreement.

Every Rust source file opens with the line

```rust
// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception
```

and `cargo xtask spdx` fails on a file without it.

## Sign off every commit

Each commit must carry a `Signed-off-by:` trailer, certifying that you wrote the
change or otherwise have the right to submit it under the project license, as the
[Developer Certificate of Origin 1.1](https://developercertificate.org/) sets out.
The trailer names you as `user.name` and `user.email` in your git config, and it
is a line in the commit message, not a cryptographic signature.

```sh
git commit -s
```

Merge commits need no sign-off. `cargo xtask dco`, part of `cargo xtask ci`, fails
on any other commit without one.

To sign off commits you have already made on a branch:

```sh
git rebase --signoff origin/main
```

To have git add the trailer to every commit in this clone, install a
`prepare-commit-msg` hook:

```sh
hook="$(git rev-parse --git-path hooks)/prepare-commit-msg"
cat > "$hook" <<'EOF'
#!/bin/sh
git interpret-trailers --in-place --if-exists doNothing \
  --trailer "Signed-off-by: $(git config user.name) <$(git config user.email)>" "$1"
EOF
chmod +x "$hook"
```

## Checks

Run `cargo xtask ci` before you push; see the [README](README.md#checks).
