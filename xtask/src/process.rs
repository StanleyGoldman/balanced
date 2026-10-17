//! Running the programs tasks call.

use std::process::Command;

use anyhow::{Context as _, Result, bail};

/// Runs `program` with `args`, failing if it exits unsuccessfully.
pub(crate) fn run(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("running {program}"))?;
    if !status.success() {
        bail!("{program} {} exited with {status}", args.join(" "));
    }
    Ok(())
}

/// Runs `program` with `args` and returns what it printed on stdout.
pub(crate) fn output(program: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("running {program}"))?;
    if !out.status.success() {
        bail!(
            "{program} {} exited with {}: {}",
            args.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    String::from_utf8(out.stdout).with_context(|| format!("{program} printed invalid UTF-8"))
}

/// The memory cap for a mutants run. A mutant can make a loop allocate without end,
/// filling RAM before cargo-mutants' per-mutant timeout fires, and on Linux the
/// kernel's OOM kill then takes the whole terminal session down with it.
const MUTANTS_MEMORY_MAX: &str = "MemoryMax=8G";

/// `program` with `args` in a memory-capped systemd scope on Linux, where the cap
/// kills only the runaway test. Elsewhere, the plain command, with a warning.
pub(crate) fn memory_capped(program: &str, args: &[&str]) -> Command {
    if cfg!(target_os = "linux") {
        let mut capped = Command::new("systemd-run");
        capped
            .args(["--user", "--scope", "--quiet", "-p", MUTANTS_MEMORY_MAX])
            .args(["-p", "MemorySwapMax=0", "--", program])
            .args(args);
        return capped;
    }
    eprintln!("warning: no memory cap on this OS; a runaway mutant can exhaust RAM");
    let mut plain = Command::new(program);
    plain.args(args);
    plain
}
