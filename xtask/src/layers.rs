// SPDX-License-Identifier: GPL-3.0-or-later WITH AdditionRef-Balanced-Plugin-Exception

//! `cargo xtask layers`: workspace crates depend only on the crates below them.
//!
//! `ARCHITECTURE.md` draws the layers. [`LAYERS`] is the same rule in code, read
//! against the dependencies `cargo metadata` reports for each workspace member.

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;

use crate::{Task, process};

pub(crate) const TASK: Task = Task {
    name: "layers",
    args: "",
    summary: "require workspace crates to depend only on the layers below them",
    run: |args| args.is_empty().then(run),
};

/// What `cargo metadata --no-deps` prints, as far as this check reads it.
#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<Package>,
}

/// A workspace member as `cargo metadata --no-deps` reports it.
#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    dependencies: Vec<Dependency>,
}

/// One dependency a member's manifest declares.
#[derive(Debug, Deserialize)]
struct Dependency {
    name: String,
    /// `None` for a normal dependency.
    kind: Option<Kind>,
}

/// How a dependency is used, when it isn't a normal dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    /// For tests, examples and benches only.
    Dev,
    /// For the build script only.
    Build,
}

/// One workspace crate and the workspace crates it may depend on.
struct Layer {
    name: &'static str,
    /// The crates directly below this one. It may also depend on anything below them.
    uses: &'static [&'static str],
    /// Allowed only as a dev-dependency, which is how examples take the harness.
    dev_only: &'static [&'static str],
}

/// The crate that owns all SQL, and the only one that may use [`SQLITE`].
const STORE: &str = "balanced_store";

/// The SQLite binding.
const SQLITE: &str = "rusqlite";

/// Prototype tooling, which only examples use.
const HARNESS: &str = "balanced_prototype_harness";

/// The layer rule: `balanced` → `balanced_ui` → {`balanced_core` → `balanced_store`,
/// `balanced_i18n`}, with `balanced_prototype_harness` and `xtask` standing apart.
const LAYERS: &[Layer] = &[
    Layer {
        name: "balanced",
        uses: &["balanced_ui"],
        dev_only: &[HARNESS],
    },
    Layer {
        name: "balanced_ui",
        uses: &["balanced_core", "balanced_i18n"],
        dev_only: &[HARNESS],
    },
    Layer {
        name: "balanced_core",
        uses: &[STORE],
        dev_only: &[],
    },
    Layer {
        name: STORE,
        uses: &[],
        dev_only: &[],
    },
    Layer {
        name: "balanced_i18n",
        uses: &[],
        dev_only: &[],
    },
    Layer {
        name: HARNESS,
        uses: &[],
        dev_only: &[],
    },
    Layer {
        name: "xtask",
        uses: &[],
        dev_only: &[],
    },
];

impl Layer {
    fn named(name: &str) -> Option<&'static Self> {
        LAYERS.iter().find(|layer| layer.name == name)
    }

    /// Whether `name` sits anywhere below this layer.
    fn is_above(&self, name: &str) -> bool {
        self.uses.iter().any(|&used| {
            used == name || Self::named(used).is_some_and(|layer| layer.is_above(name))
        })
    }

    fn may_depend_on(&self, dependency: &Dependency) -> bool {
        let name = dependency.name.as_str();
        self.is_above(name) || (dependency.kind == Some(Kind::Dev) && self.dev_only.contains(&name))
    }
}

pub(crate) fn run() -> Result<()> {
    let json = process::output(
        "cargo",
        &["metadata", "--no-deps", "--locked", "--format-version", "1"],
    )?;
    let Metadata { packages } =
        serde_json::from_str(&json).context("parsing the output of cargo metadata")?;
    check(&packages)?;
    eprintln!("layers: {} workspace crates checked", packages.len());
    Ok(())
}

/// Checks `packages` against the layer rule, naming every violation.
fn check(packages: &[Package]) -> Result<()> {
    let members: Vec<&str> = packages
        .iter()
        .map(|package| package.name.as_str())
        .collect();
    let mut violations = Vec::new();
    for package in packages {
        let Some(layer) = Layer::named(&package.name) else {
            violations.push(format!(
                "{} has no layer: add it to LAYERS in xtask/src/layers.rs and to ARCHITECTURE.md",
                package.name
            ));
            continue;
        };
        for dependency in &package.dependencies {
            let name = dependency.name.as_str();
            if members.contains(&name) && !layer.may_depend_on(dependency) {
                violations.push(format!("{} depends on {name}", package.name));
            }
            if name == SQLITE && package.name != STORE {
                violations.push(format!(
                    "{} depends on {name}; only {STORE} may",
                    package.name
                ));
            }
        }
    }
    if !violations.is_empty() {
        bail!(
            "the workspace breaks the crate layers in ARCHITECTURE.md:\n{}",
            violations.join("\n")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Dependency, Kind, Package, check};

    fn package(name: &str, dependencies: &[(&str, Option<Kind>)]) -> Package {
        Package {
            name: name.to_owned(),
            dependencies: dependencies
                .iter()
                .map(|&(name, kind)| Dependency {
                    name: name.to_owned(),
                    kind,
                })
                .collect(),
        }
    }

    /// The error `check` returns for `packages`, as one string.
    fn failure(packages: &[Package]) -> String {
        format!("{:#}", check(packages).expect_err("the check fails"))
    }

    #[test]
    fn an_upward_edge_names_both_crates() {
        let packages = [
            package("balanced_core", &[("balanced_ui", None)]),
            package("balanced_ui", &[("balanced_core", None)]),
        ];

        let failure = failure(&packages);

        assert_eq!(
            failure,
            "the workspace breaks the crate layers in ARCHITECTURE.md:\n\
             balanced_core depends on balanced_ui"
        );
    }

    #[test]
    fn an_edge_between_siblings_fails() {
        let packages = [
            package("balanced_i18n", &[("balanced_core", None)]),
            package("balanced_core", &[]),
        ];

        let failure = failure(&packages);

        assert_eq!(
            failure,
            "the workspace breaks the crate layers in ARCHITECTURE.md:\n\
             balanced_i18n depends on balanced_core"
        );
    }

    #[test]
    fn a_crate_may_skip_a_layer() {
        let packages = [
            package("balanced_ui", &[("balanced_store", None)]),
            package("balanced_store", &[]),
        ];

        let result = check(&packages);

        result.expect("balanced_store sits below balanced_ui");
    }

    #[test]
    fn rusqlite_outside_the_store_fails() {
        let packages = [
            package("balanced_core", &[("rusqlite", None)]),
            package("balanced_store", &[("rusqlite", None)]),
        ];

        let failure = failure(&packages);

        assert_eq!(
            failure,
            "the workspace breaks the crate layers in ARCHITECTURE.md:\n\
             balanced_core depends on rusqlite; only balanced_store may"
        );
    }

    #[test]
    fn examples_may_take_the_harness_as_a_dev_dependency() {
        let packages = [
            package(
                "balanced_ui",
                &[("balanced_prototype_harness", Some(Kind::Dev))],
            ),
            package("balanced_prototype_harness", &[]),
        ];

        let result = check(&packages);

        result.expect("a dev-dependency on the harness is allowed");
    }

    #[test]
    fn the_harness_as_a_normal_dependency_fails() {
        let packages = [
            package("balanced_ui", &[("balanced_prototype_harness", None)]),
            package("balanced_prototype_harness", &[]),
        ];

        let failure = failure(&packages);

        assert_eq!(
            failure,
            "the workspace breaks the crate layers in ARCHITECTURE.md:\n\
             balanced_ui depends on balanced_prototype_harness"
        );
    }

    #[test]
    fn a_crate_missing_from_the_rule_fails() {
        let packages = [
            package("balanced_reports", &[("balanced_core", None)]),
            package("balanced_core", &[]),
        ];

        let failure = failure(&packages);

        assert_eq!(
            failure,
            "the workspace breaks the crate layers in ARCHITECTURE.md:\n\
             balanced_reports has no layer: add it to LAYERS in xtask/src/layers.rs and to ARCHITECTURE.md"
        );
    }
}
