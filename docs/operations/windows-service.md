# Windows Gateway Service

mchact can install the gateway as a native Windows Service directly from `mchact.exe`.
No WinSW wrapper, `.cmd`, or extra helper script is required at runtime.

## Requirements

- Run `install`, `start`, `stop`, `restart`, and `uninstall` from an elevated terminal.
- Prepare a working directory that already contains `mchact.config.yaml`, or pass `--config` explicitly.
- Prefer absolute paths in `mchact.config.yaml`, especially for `data_dir`, because Windows services do not inherit your interactive shell context.

## Install

Example:

```powershell
cd D:\mchact-runtime
mchact gateway install
```

Default behavior:

- service name: `mchactGateway`
- display name: `mchact Gateway`
- host binary: the current `mchact.exe`
- service command line: `mchact.exe --config <path> gateway service-run --working-dir <dir>`
- startup mode: automatic
- failure policy: restart after 5 seconds, then after 15 seconds
- install command starts the service automatically

## Manage

```powershell
mchact gateway status
mchact gateway start
mchact gateway stop
mchact gateway restart
mchact gateway uninstall
```

## Notes

- `mchact gateway install` requires a real config file. Run `mchact setup` first, then install the service from that configured working directory, or set `MCHACT_CONFIG`.
- The native Windows service host starts `mchact start` internally and uses the configured working directory for runtime startup.
- mchact runtime logs are still written under `<data_dir>/runtime/logs`.
- If your provider auth depends on per-user home files such as `~/.codex/auth.json`, a Windows service running as `LocalSystem` may still behave differently from an interactive user session. In that case, prefer API-key based config or another launcher that runs under your own account.
