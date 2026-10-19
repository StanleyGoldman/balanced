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
    /// `None` for a normal dependency, else `"dev"` or `"build"`.
    kind: Option<String>,
}

/// One workspace crate and the workspace crates it may depend on.
struct Layer {
    name: &'static str,
    /// Allowed as any kind of dependency: every crate below this one.
    below: &'static [&'static str],
    /// Allowed only as a dev-dependency, which is how examples take the harness.
    dev_only: &'static [&'static str],
}

/// Prototype tooling, which only examples use.
const HARNESS: &str = "balanced_prototype_harness";

/// The layer rule: `balanced` → `balanced_ui` → {`balanced_core` → `balanced_store`,
/// `balanced_i18n`}, with `balanced_prototype_harness` and `xtask` standing apart.
const LAYERS: &[Layer] = &[
    Layer {
        name: "balanced",
        below: &[
            "balanced_ui",
            "balanced_core",
            "balanced_store",
            "balanced_i18n",
        ],
        dev_only: &[HARNESS],
    },
    Layer {
        name: "balanced_ui",
        below: &["balanced_core", "balanced_store", "balanced_i18n"],
        dev_only: &[HARNESS],
    },
    Layer {
        name: "balanced_core",
        below: &["balanced_store"],
        dev_only: &[],
    },
    Layer {
        name: "balanced_store",
        below: &[],
        dev_only: &[],
    },
    Layer {
        name: "balanced_i18n",
        below: &[],
        dev_only: &[],
    },
    Layer {
        name: HARNESS,
        below: &[],
        dev_only: &[],
    },
    Layer {
        name: "xtask",
        below: &[],
        dev_only: &[],
    },
];

/// The SQLite binding, which only [`STORE`] may use: it owns all SQL.
const SQLITE: &str = "rusqlite";

const STORE: &str = "balanced_store";

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
        let layer = LAYERS.iter().find(|layer| layer.name == package.name);
        if layer.is_none() {
            violations.push(format!(
                "{} has no layer: add it to LAYERS in xtask/src/layers.rs and to ARCHITECTURE.md",
                package.name
            ));
        }
        for dependency in &package.dependencies {
            let name = dependency.name.as_str();
            let allowed = layer.is_some_and(|layer| {
                layer.below.contains(&name)
                    || (dependency.kind.as_deref() == Some("dev") && layer.dev_only.contains(&name))
            });
            if members.contains(&name) && !allowed {
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
    use super::{Dependency, Package, check};

    fn package(name: &str, dependencies: &[(&str, Option<&str>)]) -> Package {
        Package {
            name: name.to_owned(),
            dependencies: dependencies
                .iter()
                .map(|&(name, kind)| Dependency {
                    name: name.to_owned(),
                    kind: kind.map(str::to_owned),
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

        assert!(failure(&packages).contains("balanced_core depends on balanced_ui"));
    }

    #[test]
    fn rusqlite_outside_the_store_fails() {
        let packages = [
            package("balanced_core", &[("rusqlite", None)]),
            package("balanced_store", &[("rusqlite", None)]),
        ];

        let failure = failure(&packages);

        assert!(failure.contains("balanced_core depends on rusqlite"));
        assert!(!failure.contains("balanced_store depends on rusqlite"));
    }

    #[test]
    fn the_harness_is_allowed_only_as_a_dev_dependency() {
        let harness = || package("balanced_prototype_harness", &[]);

        check(&[
            package(
                "balanced_ui",
                &[("balanced_prototype_harness", Some("dev"))],
            ),
            harness(),
        ])
        .expect("examples may take the harness");
        assert!(
            failure(&[
                package("balanced_ui", &[("balanced_prototype_harness", None)]),
                harness(),
            ])
            .contains("balanced_ui depends on balanced_prototype_harness")
        );
    }

    #[test]
    fn a_crate_missing_from_the_rule_fails() {
        assert!(
            failure(&[package("balanced_reports", &[])]).contains("balanced_reports has no layer")
        );
    }
}
