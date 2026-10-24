# Screenroom implementation rules

Read README.md and VALIDATION.md before making changes. Current targets: Windows/ultrawide and Samsung UA50AU8000RXER first; MacBook Air M4 optional; iPhone optional future. Do not expand legacy support into a full rewrite or universal compatibility promise.

Keep the entire SDLC verifiable by AI: clear device boundaries, meaningful tests, exact validation outcomes, and small reviewable changes. Select technology for delivered performance, visual quality, reliability, and coverage rather than founder familiarity. Never name or prescribe a model or effort setting in generated task instructions.

Strict $0 tools/services budget. No paid APIs, subscriptions, auto-billing, paid CI, app-store enrollment, or hosted media storage. Independent per-device history initially. Files stay on local disks/USB drives, read-only by default. Never move, rename, overwrite, or delete original media without separate user instructions.

Use current official Svelte/Tauri/Samsung docs before changing those APIs. Device playback must remain behind a platform adapter. Do not describe source scaffolding or mocked playback as hardware-certified behavior. Keep external website opening separate from content acquisition and streaming. Do not infer rights from reachability.

Never auto-match uncertain titles. Preserve user corrections and progress. Keep media-file identity separate from title/edition identity when the model evolves. Back up and migrate persisted data; never silently reset it after schema errors.

Run checks appropriate to the change. Maintain physical hardware gaps explicitly. TV navigation, ultrawide contain/letterboxing, suspend/crash progress saves, lost sources, and unsupported codecs are essential scenarios. Do not claim native compilation when only the frontend was built.
