+++
id = "msthtgch"
tags = ["oversee", "ci", "workflow"]
confidence = 0.6
helped = 0
harmed = 0
created = "2026-08-14T22:02:42.305307Z"
last_confirmed = "2026-08-14T22:02:42.305307Z"
+++

always run cargo fmt -- --check locally and wait for gh pr checks green BEFORE merging to main — oversee CI gates fmt/clippy/test and cargo test alone does not catch fmt drift
