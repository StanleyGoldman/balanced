//! Running the programs tasks call.

use std::io::Write as _;
use std::process::{Command, Output, Stdio};

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
    output_accepting(program, args, &[0])
}

/// Runs `program` with `args` and returns what it printed on stdout, failing unless
/// it exits with one of `success`.
pub(crate) fn output_accepting(program: &str, args: &[&str], success: &[i32]) -> Result<String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("running {program}"))?;
    stdout_on(&format!("{program} {}", args.join(" ")), out, success)
}

/// Runs `program` with `args`, writing `stdin` to its standard input, and returns
/// what it printed on stdout, failing if it exits unsuccessfully. A secret passed
/// this way stays out of the process listing. A failure names the program alone,
/// since its arguments can be long request bodies, and shows its stdout too, where
/// an API's error response lands.
pub(crate) fn output_with_stdin(program: &str, args: &[&str], stdin: &str) -> Result<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("running {program}"))?;
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(stdin.as_bytes())
        .with_context(|| format!("writing to {program}'s stdin"))?;
    let out = child
        .wait_with_output()
        .with_context(|| format!("waiting for {program}"))?;
    if !out.status.success() {
        bail!(
            "{program} exited with {}: {}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim(),
            String::from_utf8_lossy(&out.stdout).trim()
        );
    }
    stdout_on(program, out, &[0])
}

/// Runs `program` with `args` and `env` set on top of this process's environment,
/// with its output on the terminal, and returns its exit code; `None` means a
/// signal killed it.
pub(crate) fn exit_code_with_env(
    program: &str,
    args: &[&str],
    env: &[(&str, &str)],
) -> Result<Option<i32>> {
    let status = Command::new(program)
        .args(args)
        .envs(env.iter().copied())
        .status()
        .with_context(|| format!("running {program}"))?;
    Ok(status.code())
}

/// `out`'s stdout as text, failing unless it exited with one of `success`.
/// `command` names it in the error.
fn stdout_on(command: &str, out: Output, success: &[i32]) -> Result<String> {
    if !out
        .status
        .code()
        .is_some_and(|code| success.contains(&code))
    {
        bail!(
            "{command} exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    String::from_utf8(out.stdout).with_context(|| format!("{command} printed invalid UTF-8"))
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
