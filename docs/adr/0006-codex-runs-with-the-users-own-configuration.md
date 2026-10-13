# Delegated Codex runs use the user's own Codex configuration

Claude delegates implementation, review and research to Codex with `codex exec "<prompt>" < /dev/null` and passes no approval, sandbox or bypass flags. The user's own Codex configuration decides what a run may do, so delegation never gives Codex more access than the user chose to give it. Bypass flags (`--dangerously-bypass-approvals-and-sandbox`, `-s danger-full-access`, `-a never`) are used only when the user asks for them on that run.

## Consequences

- `codex exec` is unattended. It runs with `approval: never` and keeps the configured sandbox, so a step the sandbox blocks comes back as a failed command instead of asking for approval. Claude then does that step itself under its own permission mode, or tells the user which step was blocked.
- Codex cannot start a nested `codex exec` from inside its sandbox, so prompts use Codex's built-in sub-agents instead.
- `< /dev/null` is required, because `codex exec` otherwise waits on stdin and hangs.
