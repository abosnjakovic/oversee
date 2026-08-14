# vhs tape for oversee README screenshots.
# Regenerate with:  vhs assets/oversee.tape
# Requires a release build first:  cargo build --release

Output assets/demo.gif

Set FontSize 14
Set Width 1280
Set Height 860
Set Padding 16

Hide
Type "./target/release/oversee"
Enter
Sleep 5s
Show

# Bar meters (default view): per-core CPU, GPU, MEM/SWP.
Sleep 4s
Screenshot assets/overview.png

# Filter processes by name.
Type "/"
Sleep 1s
Type@150ms "brave"
Sleep 1s
Enter
Sleep 3s

# Clear the filter again.
Type "/"
Sleep 500ms
Escape
Sleep 2s

# Help popup with the keybinds and colour key.
Type "?"
Sleep 4s
Screenshot assets/help.png
Type "?"
Sleep 2s
