+++
id = "msr37myo"
tags = ["rust", "debugging", "macos"]
confidence = 0.6
helped = 0
harmed = 0
created = "2026-08-13T05:38:17.472618Z"
last_confirmed = "2026-08-13T05:38:17.472618Z"
+++

rust debugging (codelldb/lldb) hangs on launch in this environment until the hosting terminal app (Ghostty) has the macOS Privacy & Security -> Developer Tools grant; even bare 'lldb --batch -o run' hangs. DevToolsSecurity/developer-mode alone is insufficient. python/js/lua debugging unaffected.
