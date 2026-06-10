//! Declarative coding-agent definitions.
//!
//! A [`AgentDef`] describes how to launch one coding-agent CLI (claude, codex,
//! gemini, opencode, aider, …) as data: the command name plus a set of
//! argument-group templates. Definitions are loaded from
//! `~/.config/thurbox/agents.toml` (see [`crate::agent::agent_config`]) and
//! seeded with built-ins on first run, so users can register custom agents
//! without recompiling.
//!
//! This module is pure data + pure logic (no filesystem, no local imports
//! beyond serde/std) to satisfy the `session/` architecture rule. The TOML
//! loading and the `AgentProvider` bridge live in `crate::agent`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Placeholder substituted with a session id in resume/fork/new-session groups.
const ID_PLACEHOLDER: &str = "{id}";

/// Placeholder substituted with the system prompt in `prompt_args`.
const PROMPT_PLACEHOLDER: &str = "{prompt}";

/// One coding-agent CLI definition.
///
/// Each `*_args` group is appended to the final argument list **only** when its
/// driving value is present (the session is being resumed, a system prompt was
/// supplied, etc.), with `{id}` / `{prompt}` substituted token-by-token. This
/// avoids any "unresolved placeholder" heuristics: a group with no value is
/// simply omitted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDef {
    /// Display + lookup name (e.g. `"claude"`). Unique within a registry.
    pub name: String,
    /// CLI executable to run (e.g. `"claude"`, `"opencode"`).
    pub command: String,
    /// Static arguments always passed, before any templated group. Bake a
    /// model or any other flag here if you want one (e.g. `["--model", "opus"]`).
    #[serde(default)]
    pub args: Vec<String>,
    /// Emitted when resuming a known session; `{id}` is substituted.
    #[serde(default)]
    pub resume_args: Vec<String>,
    /// Emitted when forking from a parent conversation; `{id}` is substituted.
    #[serde(default)]
    pub fork_args: Vec<String>,
    /// Emitted on a fresh spawn to pin a session id; `{id}` is substituted.
    #[serde(default)]
    pub new_session_args: Vec<String>,
    /// When true, this agent resumes its most-recent session in the launch
    /// directory using id-less `resume_args` (no `{id}` token). thurbox cannot
    /// pin or read back the agent's real session id for these CLIs, so restart
    /// relies on the agent's own "last session in this directory" resolution
    /// (e.g. `codex resume --last`, `opencode --continue`). Agents that pin ids
    /// (claude) leave this `false` and resume by a thurbox-known id instead.
    #[serde(default)]
    pub resume_latest: bool,
    /// Environment variables injected into every session of this agent.
    /// Lowest precedence: per-spawn `--env` and thurbox-internal variables
    /// (`THURBOX_SESSION_ID`, …) override these on key collision.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Emitted when a system prompt is supplied for the launch; `{prompt}`
    /// is substituted (e.g. `["--append-system-prompt", "{prompt}"]`).
    /// Agents without this template reject `--system-prompt` at spawn time.
    #[serde(default)]
    pub prompt_args: Vec<String>,
}

/// Per-launch inputs to [`AgentDef::build_args`].
///
/// Bundles the session-selection ids with the per-spawn customizations so
/// new launch knobs don't ripple through every call site.
#[derive(Debug, Clone, Copy, Default)]
pub struct LaunchArgs<'a> {
    /// Resume an existing session by thurbox-known id.
    pub resume_id: Option<&'a str>,
    /// Fork from a parent conversation by id (wins over `resume_id`).
    pub fork_id: Option<&'a str>,
    /// Pin a fresh session id on first spawn.
    pub new_session_id: Option<&'a str>,
    /// System prompt substituted into the agent's `prompt_args` template.
    pub system_prompt: Option<&'a str>,
    /// Per-spawn arguments appended after every templated group, so they
    /// can override anything baked into the definition.
    pub extra_args: &'a [String],
}

impl AgentDef {
    /// Build the CLI argument list for one launch.
    ///
    /// Session-selection precedence mirrors the historical Claude behaviour:
    /// fork wins over resume, which wins over a fresh `new_session` id. Then
    /// come `prompt_args` (when a system prompt is supplied), the static
    /// `args`, and finally any per-spawn `extra_args` (last, so they can
    /// override baked-in flags). No model is ever passed — the agent uses its
    /// own default config (bake one into `args` if needed).
    pub fn build_args(&self, launch: &LaunchArgs<'_>) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();

        if let Some(id) = launch.fork_id {
            out.extend(subst(&self.fork_args, ID_PLACEHOLDER, id));
        } else if let Some(id) = launch.resume_id {
            out.extend(subst(&self.resume_args, ID_PLACEHOLDER, id));
        } else if let Some(id) = launch.new_session_id {
            out.extend(subst(&self.new_session_args, ID_PLACEHOLDER, id));
        }

        if let Some(prompt) = launch.system_prompt {
            out.extend(subst(&self.prompt_args, PROMPT_PLACEHOLDER, prompt));
        }

        out.extend(self.args.iter().cloned());
        out.extend(launch.extra_args.iter().cloned());

        out
    }

    /// Whether a restart should trigger this agent's resume group via
    /// "latest session in the launch directory" semantics rather than a
    /// thurbox-known session id. True only when [`Self::resume_latest`] is set
    /// and there are `resume_args` to emit.
    pub fn resumes_latest(&self) -> bool {
        self.resume_latest && !self.resume_args.is_empty()
    }
}

/// Replace `placeholder` with `value` in every token of `tokens`.
fn subst(tokens: &[String], placeholder: &str, value: &str) -> Vec<String> {
    tokens
        .iter()
        .map(|t| t.replace(placeholder, value))
        .collect()
}

/// A set of agent definitions plus the name of the default agent.
///
/// Unknown fields are tolerated but reported: the loader names every
/// unrecognized key in a startup warning (stale keys from older versions and
/// typos both surface without stranding the user on built-ins).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRegistry {
    /// Config-format version, for future migrations. Currently `1`.
    #[serde(default)]
    pub config_version: Option<u32>,
    /// Name of the agent selected by default in the picker / headless spawns.
    #[serde(default)]
    pub default: String,
    /// All known agent definitions, in display order.
    #[serde(default)]
    pub agents: Vec<AgentDef>,
}

impl AgentRegistry {
    /// Look up an agent definition by name.
    pub fn get(&self, name: &str) -> Option<&AgentDef> {
        self.agents.iter().find(|a| a.name == name)
    }

    /// The default agent definition: the one named by `default`, falling back
    /// to the first defined agent.
    pub fn default_agent(&self) -> Option<&AgentDef> {
        self.get(&self.default).or_else(|| self.agents.first())
    }

    /// The default agent's name, or empty string when the registry is empty.
    pub fn default_name(&self) -> String {
        self.default_agent()
            .map(|a| a.name.clone())
            .unwrap_or_default()
    }

    /// All agent names in display order.
    pub fn names(&self) -> Vec<&str> {
        self.agents.iter().map(|a| a.name.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claude() -> AgentDef {
        AgentDef {
            name: "claude".into(),
            command: "claude".into(),
            resume_args: vec!["--resume".into(), "{id}".into()],
            fork_args: vec!["--resume".into(), "{id}".into(), "--fork-session".into()],
            new_session_args: vec!["--session-id".into(), "{id}".into()],
            ..AgentDef::default()
        }
    }

    #[test]
    fn fresh_session_pins_id() {
        let d = claude();
        let args = d.build_args(&LaunchArgs {
            new_session_id: Some("new-id"),
            ..LaunchArgs::default()
        });
        assert_eq!(args, vec!["--session-id", "new-id"]);
        // No model is ever passed.
        assert!(!args.iter().any(|a| a == "--model"));
    }

    #[test]
    fn resume_takes_precedence_over_new() {
        let d = claude();
        let args = d.build_args(&LaunchArgs {
            resume_id: Some("resume-id"),
            new_session_id: Some("new-id"),
            ..LaunchArgs::default()
        });
        assert_eq!(args, vec!["--resume", "resume-id"]);
    }

    #[test]
    fn fork_takes_precedence_over_resume() {
        let d = claude();
        let args = d.build_args(&LaunchArgs {
            resume_id: Some("resume-id"),
            fork_id: Some("fork-id"),
            new_session_id: Some("new-id"),
            ..LaunchArgs::default()
        });
        assert_eq!(args, vec!["--resume", "fork-id", "--fork-session"]);
    }

    #[test]
    fn static_args_only_when_no_session_group() {
        let d = AgentDef {
            name: "codex".into(),
            command: "codex".into(),
            args: vec!["--quiet".into()],
            ..AgentDef::default()
        };
        let args = d.build_args(&LaunchArgs {
            new_session_id: Some("ignored"),
            ..LaunchArgs::default()
        });
        assert_eq!(args, vec!["--quiet"]);
    }

    #[test]
    fn idless_resume_and_fork_pass_tokens_verbatim() {
        // Mirrors the seeded codex definition: id-less resume/fork groups that
        // resolve "latest in cwd" inside the agent and ignore any supplied id.
        let d = AgentDef {
            name: "codex".into(),
            command: "codex".into(),
            resume_args: vec!["resume".into(), "--last".into()],
            fork_args: vec!["fork".into(), "--last".into()],
            resume_latest: true,
            ..AgentDef::default()
        };
        // resume id present, but no {id} token -> tokens unchanged.
        assert_eq!(
            d.build_args(&LaunchArgs {
                resume_id: Some("ignored-uuid"),
                ..LaunchArgs::default()
            }),
            vec!["resume", "--last"]
        );
        // fork wins over resume, still id-less.
        assert_eq!(
            d.build_args(&LaunchArgs {
                resume_id: Some("ignored-uuid"),
                fork_id: Some("also-ignored"),
                ..LaunchArgs::default()
            }),
            vec!["fork", "--last"]
        );
        assert!(d.resumes_latest());
    }

    #[test]
    fn prompt_args_substituted_when_system_prompt_present() {
        let d = AgentDef {
            prompt_args: vec!["--append-system-prompt".into(), "{prompt}".into()],
            args: vec!["--quiet".into()],
            ..claude()
        };
        let args = d.build_args(&LaunchArgs {
            new_session_id: Some("id-1"),
            system_prompt: Some("be terse"),
            ..LaunchArgs::default()
        });
        // Emit order: session-selection group, prompt_args, static args.
        assert_eq!(
            args,
            vec![
                "--session-id",
                "id-1",
                "--append-system-prompt",
                "be terse",
                "--quiet"
            ]
        );
    }

    #[test]
    fn prompt_args_omitted_without_system_prompt() {
        let d = AgentDef {
            prompt_args: vec!["--append-system-prompt".into(), "{prompt}".into()],
            ..claude()
        };
        let args = d.build_args(&LaunchArgs {
            new_session_id: Some("id-1"),
            ..LaunchArgs::default()
        });
        assert_eq!(args, vec!["--session-id", "id-1"]);
    }

    #[test]
    fn extra_args_appended_last() {
        let d = AgentDef {
            args: vec!["--quiet".into()],
            ..claude()
        };
        let extra = vec!["--permission-mode".to_string(), "plan".to_string()];
        let args = d.build_args(&LaunchArgs {
            new_session_id: Some("id-1"),
            extra_args: &extra,
            ..LaunchArgs::default()
        });
        assert_eq!(
            args,
            vec![
                "--session-id",
                "id-1",
                "--quiet",
                "--permission-mode",
                "plan"
            ]
        );
    }

    #[test]
    fn env_and_prompt_args_roundtrip_toml() {
        let toml = r#"
            name = "custom"
            command = "custom"
            prompt_args = ["--system", "{prompt}"]
            [env]
            FOO = "bar"
        "#;
        let d: AgentDef = toml::from_str(toml).unwrap();
        assert_eq!(d.env.get("FOO").map(String::as_str), Some("bar"));
        assert_eq!(d.prompt_args, vec!["--system", "{prompt}"]);
        // Fields default to empty when absent.
        let bare: AgentDef = toml::from_str("name = \"x\"\ncommand = \"x\"").unwrap();
        assert!(bare.env.is_empty());
        assert!(bare.prompt_args.is_empty());
    }

    #[test]
    fn resumes_latest_requires_flag_and_resume_args() {
        let mut d = AgentDef {
            name: "x".into(),
            command: "x".into(),
            resume_latest: true,
            ..AgentDef::default()
        };
        // Flag set but no resume_args -> nothing to emit, so not "resumes latest".
        assert!(!d.resumes_latest());
        d.resume_args = vec!["--continue".into()];
        assert!(d.resumes_latest());
        d.resume_latest = false;
        assert!(!d.resumes_latest());
    }

    #[test]
    fn registry_lookup_and_default() {
        let reg = AgentRegistry {
            config_version: None,
            default: "codex".into(),
            agents: vec![
                claude(),
                AgentDef {
                    name: "codex".into(),
                    command: "codex".into(),
                    ..AgentDef::default()
                },
            ],
        };
        assert_eq!(reg.get("claude").unwrap().command, "claude");
        assert_eq!(reg.default_agent().unwrap().name, "codex");
        assert_eq!(reg.names(), vec!["claude", "codex"]);
    }

    #[test]
    fn registry_default_falls_back_to_first() {
        let reg = AgentRegistry {
            config_version: None,
            default: "missing".into(),
            agents: vec![claude()],
        };
        assert_eq!(reg.default_agent().unwrap().name, "claude");
    }
}
