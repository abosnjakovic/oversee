# vhs tape for oversee README screenshots.
# Regenerate with:  vhs assets/oversee.tape
# Requires a build first:  cargo build   (or --release)

Output assets/demo.gif

Set FontSize 14
Set Width 1280
Set Height 860
Set Padding 16

# Launch and let the timeline buffer fill (it scrolls in from the right over
# time, so a narrower window fills faster).
Hide
Type "./target/debug/oversee"
Enter
Sleep 70s
Show

# Overview (default) — combined CPU / GPU / memory.
Screenshot assets/overview.png
Sleep 2s

# Tab cycles the timeline metric.
Tab
Sleep 3s
Screenshot assets/cpu.png

# GPU cores (only interesting with sudo / powermetrics; skipped as a still).
Tab
Sleep 3s

Tab
Sleep 3s
Screenshot assets/memory.png

# Back to overview for the trailing gif frame.
Tab
Sleep 2s
