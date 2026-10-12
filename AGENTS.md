# AGENTS.md

## Agent skills

### Issue tracker

Issues live as local markdown files under `.scratch/<feature>/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Default five-role vocabulary (needs-triage, needs-info, ready-for-agent, ready-for-human, wontfix), recorded as a `Status:` line. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one root `CONTEXT.md` + `docs/adr/`. See `docs/agents/domain.md`.

## Agent delegation policy

When running Matt Pocock engineering workflows, Claude is the orchestrator. Codex is consider as executor or sub-agent to help speed up implementation and review work. Claude owns the final decision and is responsible for the quality of the work.

Note: Codex usually hangs and did not produce any output. Check its progress after a 2 minutes. If Codex is stuck, you can cancel the current task and re-run it.

### Permission mode

Delegated Codex runs use the user's own Codex configuration: run `codex exec "<prompt>" < /dev/null` and pass no approval, sandbox or bypass flags (ADR 0006). Never add `--dangerously-bypass-approvals-and-sandbox`, `-s danger-full-access` or `-a never` unless the user asks for that run.

`codex exec` is unattended: it runs with `approval: never` whatever `approval_policy` says, and keeps the configured sandbox. A step the sandbox blocks does not wait for approval. It comes back to Codex as a failed command (for example `Access to the path ... is denied`), and Codex reports it. When that happens, Claude does the blocked step itself under its own permission mode, so the user is asked through Claude, or tells the user which step was blocked and lets them decide whether to widen the sandbox. Check the run header (`approval:` and `sandbox:` lines) when a result looks incomplete.

Codex cannot start a nested `codex exec` from inside a sandboxed Codex session. Without network access it loops on `invalid peer certificate: UnknownIssuer` and `Reconnecting... waiting for network`. With `sandbox_workspace_write.network_access=true` it fails with `Could not find home directory`, because the Windows elevated sandbox user has no home. Do not ask Codex to delegate to `codex exec`. Use Codex's built-in sub-agents in the prompt instead, or have Claude do the work.

### Research

Deligate research tasks to Codex. Claude can self-invoke to perform research work when Codex is unavailable or stuck.

Use:

    codex exec "<research instruction prompt>"

### /implement

For each ticket:

1. Claude reads the ticket/spec and determines the required implementation.
2. Delegate implementation to Codex CLI.
3. Codex modifies the working tree and runs relevant tests.
4. Claude verifies the result and acceptance criteria.
5. Continue the normal `/implement` workflow.

Use:

    codex exec "<implementation prompt>"

Codex must not commit unless explicitly instructed.

Note: Some specific cases, Claude can work in parallel with Codex to speed up the implementation:
- Codex hit the quota limit and cannot continue, Claude can continue the implementation.
- Tasks can be done in parallel, Claude can work on one task while Codex works on another task.

### /code-review

When `/implement` reaches the code-review phase, delegate the actual review work
to Codex instead of performing the review directly with Claude sub-agents.

The review must preserve Matt Pocock's two review axes:

1. Standards review
   - Compare the diff against repository coding standards.
   - Identify maintainability/design/code-quality issues.
   - Treat code smells as judgement calls rather than automatic violations.

2. Spec review
   - Compare the implementation against the originating ticket/spec.
   - Identify missing requirements, incorrect behavior, and scope deviations.

Use independent Codex invocations for the two axes where practical.

Example:

    codex exec "<standards review prompt>"

    codex exec "<spec review prompt>"

Claude then:
- collects both reviews
- evaluates the findings
- delegates fixes to Codex when appropriate
- reruns review if necessary
- commits only after the implementation is acceptable

Claude owns orchestration and the final decision.
Codex performs implementation and review work. Claude can self-invoke to perform implementation and review work when Codex is unavailable or stuck.
