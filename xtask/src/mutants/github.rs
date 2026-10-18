//! The GitHub calls a CI mutant run makes: reading the open `mutant` issues and
//! carrying out the plan's actions. It talks to the REST API through `curl`, which
//! the CI image already has, so xtask needs no HTTP or TLS crates.

use anyhow::{Context as _, Result};
use serde::Deserialize;
use serde_json::{Value, json};

use super::REPO;
use super::plan::{Action, CloseReason, OpenIssue};
use crate::process;

/// The environment variable holding the token. In CI it comes from the
/// `mutants_github_token` secret: issues read and write, pull requests read.
const TOKEN_VAR: &str = "GITHUB_TOKEN";

/// GitHub's largest page size.
const PER_PAGE: usize = 100;

pub(crate) struct GitHub {
    token: String,
}

impl GitHub {
    /// A client with the token from `GITHUB_TOKEN`. A dry run needs it too: the
    /// repository is private, so even reading its issues takes a token.
    pub(crate) fn from_env() -> Result<Self> {
        let token = std::env::var(TOKEN_VAR).with_context(|| {
            format!("{TOKEN_VAR} is unset; by hand, `{TOKEN_VAR}=$(gh auth token)` works")
        })?;
        Ok(Self { token })
    }

    /// Every open issue labelled `mutant`.
    pub(crate) fn open_mutant_issues(&self) -> Result<Vec<OpenIssue>> {
        #[derive(Deserialize)]
        struct Listed {
            number: u64,
            body: Option<String>,
            /// Set on a pull request, which the issues endpoint lists too.
            pull_request: Option<Value>,
        }
        let mut issues = Vec::new();
        for page in 1.. {
            let text = self.request(
                "GET",
                &format!("issues?labels=mutant&state=open&per_page={PER_PAGE}&page={page}"),
                None,
            )?;
            let listed: Vec<Listed> =
                serde_json::from_str(&text).context("reading the list of mutant issues")?;
            let last = listed.len() < PER_PAGE;
            issues.extend(
                listed
                    .into_iter()
                    .filter(|listed| listed.pull_request.is_none())
                    .map(|listed| OpenIssue {
                        number: listed.number,
                        body: listed.body.unwrap_or_default(),
                    }),
            );
            if last {
                break;
            }
        }
        Ok(issues)
    }

    /// GitHub's `commits/{commit}/pulls` response: every pull request holding
    /// `commit`, as JSON.
    pub(crate) fn pulls_holding(&self, commit: &str) -> Result<String> {
        self.request("GET", &format!("commits/{commit}/pulls"), None)
    }

    /// How many commits pull request `number` holds. The list of pull requests holding
    /// a commit leaves the count out, so it takes this call of its own.
    pub(crate) fn commit_count(&self, number: u64) -> Result<u32> {
        #[derive(Deserialize)]
        struct Pull {
            commits: u32,
        }
        let text = self.request("GET", &format!("pulls/{number}"), None)?;
        let pull: Pull = serde_json::from_str(&text)
            .with_context(|| format!("reading pull request #{number}"))?;
        Ok(pull.commits)
    }

    /// Carries out one of the plan's actions.
    pub(crate) fn apply(&self, action: &Action) -> Result<()> {
        match action {
            Action::Open(issue) => {
                let created = self.request(
                    "POST",
                    "issues",
                    Some(&json!({
                        "title": issue.title,
                        "body": issue.body,
                        "labels": issue.labels,
                    })),
                )?;
                let created: Value =
                    serde_json::from_str(&created).context("reading the opened issue")?;
                eprintln!(
                    "opened {}",
                    created["html_url"].as_str().unwrap_or("an issue")
                );
            }
            Action::Comment { issue, body } => self.comment(*issue, body)?,
            Action::Close {
                issue,
                reason,
                comment,
            } => {
                self.comment(*issue, comment)?;
                let state_reason = match reason {
                    CloseReason::Caught => "completed",
                    CloseReason::NoLongerGenerated => "not_planned",
                };
                self.request(
                    "PATCH",
                    &format!("issues/{issue}"),
                    Some(&json!({"state": "closed", "state_reason": state_reason})),
                )
                .with_context(|| format!("closing #{issue}"))?;
            }
        }
        Ok(())
    }

    fn comment(&self, issue: u64, body: &str) -> Result<()> {
        self.request(
            "POST",
            &format!("issues/{issue}/comments"),
            Some(&json!({"body": body})),
        )
        .with_context(|| format!("commenting on #{issue}"))?;
        Ok(())
    }

    /// Sends one request to `repos/<REPO>/<path>` and returns the response body. The
    /// token goes to curl on stdin, so it never shows in a process listing.
    fn request(&self, method: &str, path: &str, body: Option<&Value>) -> Result<String> {
        let url = format!("https://api.github.com/repos/{REPO}/{path}");
        let body = body.map(Value::to_string);
        let mut args = vec![
            "--silent",
            "--show-error",
            "--fail-with-body",
            "--header",
            "@-",
            "--header",
            "Accept: application/vnd.github+json",
            "--header",
            "X-GitHub-Api-Version: 2022-11-28",
            "--request",
            method,
        ];
        if let Some(body) = &body {
            args.extend(["--header", "Content-Type: application/json"]);
            args.extend(["--data-binary", body]);
        }
        args.push(&url);
        let header = format!("Authorization: Bearer {}\n", self.token);
        process::output_with_stdin("curl", &args, &header)
            .with_context(|| format!("{method} {url}"))
    }
}
