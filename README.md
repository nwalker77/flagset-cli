# flagset-cli

A small feature flag library, plus a CLI for poking at it from a terminal
or a deploy script. No dependencies - the whole thing is std only.

## The problem this solves

Most feature flag setups I've dealt with end up as one of two things: a
hardcoded `if user.id == 42` scattered through the codebase, or a full
SaaS integration for a rollout that's really just "turn this on for 10%
of accounts and a specific list of internal testers." This is the second
thing, minus the SaaS: a config file you can read in a code review, a
rollout percentage that's the same every time you check it (not
re-randomized per process), and an override list for the cases where the
percentage math shouldn't apply - a support rep flipping a flag on for
one customer, or a kill switch for one bad account.

## Config format

```
# comment lines and blank lines are ignored
flag dark_mode
flag checkout_v2 rollout=25
flag legacy_export enabled=false
flag legacy_export_pilot enabled=false
override legacy_export_pilot acct-9001=true
```

- `flag <name>` with no attributes means enabled, 100% rollout.
- `rollout=N` is a percentage (0..=100) of keys that get the flag, chosen
  by hashing `(flag name, key)` - the same key always lands in the same
  bucket, across restarts, because the hash isn't seeded randomly.
- `enabled=false` turns a flag fully off regardless of rollout.
- `override <flag> <key>=true|false` pins one key to a fixed result. An
  override wins even over `enabled=false` or a 0% rollout - it's meant as
  an escape hatch, not part of the normal rollout math. A flag must be
  declared before any `override` line that targets it.

## Library usage

```rust
let flags = flagset_cli::parse(&config_text)?;

if flags.is_enabled("checkout_v2", &user.id)? {
    // new path
}
```

`is_enabled` returns `Err` for a flag name that was never declared,
instead of silently treating a typo as "off." If you want to show a
user which rollout bucket they're in without evaluating a specific flag,
`flagset_cli::bucket(flag_name, key)` is the same hash the library uses
internally, exposed directly.

## CLI usage

```
$ flagset flags.conf list
checkout_v2 enabled=true rollout=25 overrides=0
dark_mode enabled=true rollout=100 overrides=0

$ flagset flags.conf check checkout_v2 user-482
on

$ flagset flags.conf validate
ok: 2 flag(s)
```

`validate` parses the config and exits non-zero with a `line N: ...` error on
stderr if it's malformed, without needing a flag or key to check - useful as
a CI step that guards a config file before it's deployed.

## Status

Early. The parser and evaluator are stable enough to build on; the CLI is
intentionally minimal for now. See the test suite in `tests/` for the
exact semantics of overrides, disabled flags, and malformed config -
those are the cases that are easy to get subtly wrong.

## License

MIT, see `LICENSE`.
