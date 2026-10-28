# Optional bundled daemons

QuixiDesk looks for `routstrd` here before `~/.local/bin` and `PATH`:

```text
resources/bin/routstrd
```

The binary is intentionally gitignored (100+ MB). Populate it with:

```bash
./scripts/install-routstrd.sh --bundle                 # stable release
./scripts/install-routstrd.sh --bundle --confidential  # confidential-upstream fork
```

`--confidential` builds the fork with Bun and needs a matching
`@routstr/sdk` confidential build (the fork currently depends on an unreleased
SDK commit). See `scripts/install-routstrd.sh` and the README's Routstr section.
