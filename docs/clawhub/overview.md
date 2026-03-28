# ClawHub Integration

## What it adds

mchact integrates with ClawHub to search and install skill packs from the runtime, CLI, and agent tool layer.

- CLI: `mchact skill search|install|list|inspect|available`
- Agent tools: `clawhub_search`, `clawhub_install`
- Lockfile: `clawhub.lock.json` (managed install state)

## Storage locations

- Skills directory: `<data_dir>/skills` (default: `~/.mchact/skills`)
- Lockfile: `<data_dir>/clawhub.lock.json` (default: `~/.mchact/clawhub.lock.json`)
- Optional config override: `skills_dir` in `mchact.config.yaml`

Compatibility behavior:
- Existing configured paths (`data_dir` / `skills_dir` / `working_dir`) are always respected.
- New defaults (`~/.mchact`, `<data_dir>/skills`, `~/.mchact/working_dir`) are used only when fields are not configured.

## Config

In `mchact.config.yaml`:

```yaml
clawhub_registry: "https://clawhub.ai"
clawhub_token: ""
clawhub_agent_tools_enabled: true
clawhub_skip_security_warnings: false
```

Notes:

- `clawhub_agent_tools_enabled: true` controls whether the agent can call `clawhub_search` and `clawhub_install`.
- ClawHub config is flattened into the top-level config object, not nested under a separate `clawhub:` block.

## Operational notes

- Use `sync_skills` for GitHub-backed skill repos and `clawhub_install` for ClawHub packages. They are different acquisition paths.
- Keep `clawhub_skip_security_warnings: false` in production.
- Review `clawhub.lock.json` in CI for supply-chain traceability.
- Pin versions in automation instead of implicit latest.
