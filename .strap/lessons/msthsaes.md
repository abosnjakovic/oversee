+++
id = "msthsaes"
tags = ["github", "docs"]
confidence = 0.6
helped = 0
harmed = 0
created = "2026-08-14T22:01:47.956936Z"
last_confirmed = "2026-08-14T22:01:47.956936Z"
+++

freshly pushed README images can look stale on github.com for minutes — raw.githubusercontent.com CDN caches ~5min and browsers cache longer; verify with curl -sI content-length vs local file size before debugging the repo
