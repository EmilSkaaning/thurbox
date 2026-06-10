//! Data-driven [`AgentProvider`] built from an [`AgentDef`].
//!
//! Replaces the former hard-coded `ClaudeProvider`: any agent described in
//! `agents.toml` is launched through this single provider, which maps the
//! session's resume/fork/session ids onto the definition's argument templates.

use crate::session::{AgentDef, LaunchArgs, SessionConfig};

use super::provider::AgentProvider;

/// An [`AgentProvider`] backed by a declarative [`AgentDef`].
pub struct GenericProvider {
    def: AgentDef,
}

impl GenericProvider {
    /// Wrap an agent definition.
    pub fn new(def: AgentDef) -> Self {
        Self { def }
    }

    /// The underlying definition.
    pub fn def(&self) -> &AgentDef {
        &self.def
    }
}

impl AgentProvider for GenericProvider {
    fn command(&self) -> &str {
        &self.def.command
    }

    fn build_args(&self, config: &SessionConfig) -> Vec<String> {
        self.def.build_args(&LaunchArgs {
            resume_id: config.resume_session_id.as_deref(),
            fork_id: config.fork_session_id.as_deref(),
            new_session_id: config.agent_session_id.as_deref(),
            system_prompt: config.system_prompt.as_deref(),
            extra_args: &config.extra_args,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::agent_config::builtin_registry;

    #[test]
    fn claude_provider_builds_resume_without_model() {
        let reg = builtin_registry();
        let provider = GenericProvider::new(reg.get("claude").unwrap().clone());
        assert_eq!(provider.command(), "claude");

        let config = SessionConfig {
            resume_session_id: Some("abc-123".into()),
            ..SessionConfig::default()
        };
        let args = provider.build_args(&config);
        assert_eq!(args, vec!["--resume", "abc-123"]);
        assert!(!args.iter().any(|a| a == "--model"));
    }

    #[test]
    fn fresh_session_pins_id_no_model() {
        let reg = builtin_registry();
        let provider = GenericProvider::new(reg.get("claude").unwrap().clone());

        let config = SessionConfig {
            agent_session_id: Some("new-id".into()),
            ..SessionConfig::default()
        };
        let args = provider.build_args(&config);
        assert_eq!(args, vec!["--session-id", "new-id"]);
    }

    #[test]
    fn provider_threads_system_prompt_and_extra_args() {
        let mut def = builtin_registry().get("claude").unwrap().clone();
        def.prompt_args = vec!["--append-system-prompt".into(), "{prompt}".into()];
        let provider = GenericProvider::new(def);

        let config = SessionConfig {
            agent_session_id: Some("id-9".into()),
            system_prompt: Some("be brief".into()),
            extra_args: vec!["--permission-mode".into(), "plan".into()],
            ..SessionConfig::default()
        };
        let args = provider.build_args(&config);
        assert_eq!(
            args,
            vec![
                "--session-id",
                "id-9",
                "--append-system-prompt",
                "be brief",
                "--permission-mode",
                "plan"
            ]
        );
    }
}
