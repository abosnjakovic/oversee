# oversee perf

Tooling for profiling the TUI's CPU usage.

## One-time setup

```sh
cargo install samply
```

## Record a flamegraph

```sh
./perf/record.sh            # idle profile, 30s
```

Opens the Firefox profiler UI on completion. The release build keeps line-tables
so frames are symbolicated.

## Measure idle CPU + capture a baseline

```sh
./perf/measure.sh           # writes perf/baseline/{idle-cpu.txt,profile-summary.txt,idle.json}
```

Re-run after each change. The `Makefile` `perf` target wraps this and diffs the
idle CPU against `perf/baseline/idle-cpu.txt`.

## Enabling the profile macro

The `profile!()` macro is gated behind a feature flag (default off). To enable
in-process timing logs to `/tmp/oversee-profile.log`:

```sh
cargo build --release --features profile
```

`measure.sh` will summarise the log into `perf/baseline/profile-summary.txt`
when the file is present.

## Acceptance target

Idle 10-sample `top` average below **2%** on the developer machine that took
the baseline.
