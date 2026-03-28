# Nixpkgs Automation

This repository includes automation for keeping `mchact` updated in `NixOS/nixpkgs`.

## One-command Flow

Run from repo root:

```sh
scripts/update-nixpkgs.sh
```

By default, the script will:
- detect version from `Cargo.toml`
- clone `<your-gh-user>/nixpkgs` into `/tmp/nixpkgs-<timestamp>`
- branch from `upstream/nixos-unstable`
- update `pkgs/by-name/mi/mchact/package.nix`
- resolve `hash` and `cargoHash`
- run `nix-build -A mchact` and `result/bin/mchact --help`
- commit, push, and open PR to `NixOS/nixpkgs`

## Deploy Integration

After release, you can trigger nixpkgs automation with:

```sh
AUTO_NIXPKGS_UPDATE=1 ./deploy.sh
```

## Useful Flags

```sh
scripts/update-nixpkgs.sh --version 0.0.164
scripts/update-nixpkgs.sh --draft
scripts/update-nixpkgs.sh --no-pr
scripts/update-nixpkgs.sh --nixpkgs-dir ~/focus/nixpkgs
```

## Failure Recovery

If the script fails mid-way:
- check printed temp dir path, inspect git/logs there
- re-run script (it creates a new timestamped temp repo by default)
- optionally run with `--nixpkgs-dir` to reuse a local checkout
