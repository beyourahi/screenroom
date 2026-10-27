> **SCOPE AND STACK NOTICE (2026-10-26):** [`ADR-002_RUST_FIRST_STACK.md`](ADR-002_RUST_FIRST_STACK.md) as amended by [`ADR-003_DESKTOP_AND_TV_SCOPE.md`](ADR-003_DESKTOP_AND_TV_SCOPE.md) is the authoritative architecture: desktop/laptop (Windows, macOS, Linux) and smart TVs (Samsung Tizen, LG webOS, Android TV, other compatible TVs) only, with equal priority; shared Rust core, iced + wgpu + libmpv desktop, LightningJS 3 + Blits + TypeScript TV with platform-native playback, SQLite where supported, FFmpeg/FFprobe, MIT first-party code. iOS/Android phone and tablet apps and the Flutter fallback are out of scope. This document's product requirements, entities and evidence rules still apply to desktop and TV; any mobile, Flutter-only or Svelte/Tauri statement inside it is history, not a requirement. No physical-device gate has passed -- see `IMPLEMENTATION_STATUS.md` for what is actually verified.

# Dropout Design System — mandatory visual-source integration

**User decision (2026-10-25):** The owner's **private Dropout Studio Design System** is the authoritative visual/component language for this product. The prototype palette is superseded. Product composition must still look like an exceptional movie/TV player, not a generic SaaS admin dashboard.

## Upstream reviewed and its actual ownership

Private repo: `beyourahi/dropout-design-system` on `main`, inspected at commit `41406ab436ef2b9c1edcf69e9d4ada4b94d13eec` (2026-10-25). Sources reviewed: `README.md`, `DESIGN.md`, `GUIDELINES.md`, DTCG 2025.10 source `src/lib/tokens/tokens.json`, and `consumer-inventory.json`. On inspection, official consumer inventory listed Dropout Studio, Horcrux and beyourahi.com; **this app had not been registered**. That must be addressed through the approved process, not bypassed.

Canonical chain: DTCG semantic tokens → foundations → shadcn-svelte/Bits UI → Dropout `ui/` primitives → `signature/` elements → `patterns/` → consuming product. Canonical source `src/lib/` stays in private DS upstream; approved immutable-commit-pinned *vendoring* into a consumer's own `src/lib/ds` uses manifest `vendor/dropout-design-system/manifest.json`, source SHA and hashes. No public registry/source download supported. **Do not bundle DS private source in this planning ZIP or republish it.** A future public app distribution must separately clear DS licensing/source policy.

## Mandatory design principles

- **Black-first, editorial, typographic, restrained, premium** with canonical `.dark`; media art can be colorful without changing the DS semantic surface system. Use neutral ink ramp, semantic contextual `surface-1`, `surface-2`, `surface-3`, `surface-input`, deliberate `signal` action accent; *never* invent a pale-green screenroom theme or raw hex local color palette.
- **No borders, outlines, rings, stroke-like shadows** on Dropout-owned controls at rest, hover, focus, invalid, selected, disabled or loading. Focus uses stronger **semantic fill + contrasting foreground without transition latency**; rich focusable content follows the canonical `data-focus-container` marker/gutter contract where feasible. Samsung D-pad focus must remain unmissable from the sofa—test an approved adapter rather than quietly adding contrary rings.
- WCAG 2.2 AA text/nontext contrast checked against actual adjacent surface; 44×44 coarse targets, accessible names, no hover-only behavior, reduced-motion and valid logical properties. TV 10-foot typography and poster grids are product-specific adaptations, not independent color tokens.
- `Heading`, `Eyebrow`, `Tile`, `Cta`, `IconButton`, `StatusBadge` as appropriate, with generic `Button`, `Field`, `Dialog`, `Card` and other canonical primitives instead of new local versions. Static Lucide icons as default; selected MovingIcons only from canonical DS barrel, and never with constant unnecessary TV animation.
- App-owned cinematic designs: **Continue Watching first**, real poster rails, show/season hierarchy, rich details, clean player HUD, overlays, source selection, profile switch, library search, empty/offline/unknown/error states, deterministic focus graph. No giant marketing hero that hides the library, no fake sample media.

## Integration and older Samsung compatibility

1. Read current actual DS docs/commits before implementation. Request approval/addition of an app consumer to canonical `consumer-inventory.json`; apply its **reviewed** sync procedure and SHA/hash manifest; no ad-hoc copies, submodules, second product repo, registry mirror or divergent generic controls.
2. Use canonical `src/lib/ds` imports and the verified CSS ordering from upstream guidelines: Tailwind first, DS `tokens.css`, then approved animations. Dark root class. Product-specific cinema screens compose—not fork—upstream primitives.
3. Samsung Tizen 6.0 **Chromium M76** requires compatibility tests for OKLCH semantic CSS, nested declarations/selectors, CSS functions and JS APIs. Generate compatible compiled **sRGB/fallback role values derived from canonical tokens** (not independent invented colors). Validate both the syntax and visual parity on actual TV. A feature that is unsupported must have a tested fallback, not invisible controls or default unstyled rendering.
4. Use component subset/tree-shaking and static assets so TV does not carry Storybook, unused icon libraries, heavy blur or all desktop UI dependencies. Design performance is judged from actual installed-app input/CPU/RAM/paint behavior, not only gzip sizes.
5. Reusable design-system improvements go to private DS upstream first and propagate through approved sync. Domain-specific player and remote behavior stays in product. Never expose private DS sources, copied manifests containing secrets, or credentials in public deliverables.

## Acceptance tests

- [ ] Approved pinned DS revision and manifest; product registered via canonical private inventory without violating privacy/distribution rules.
- [ ] Canonical black-first semantic colors/typography/geometry and borderless focus match DS on desktop; no original prototype palette survives.
- [ ] All general controls use approved DS; platform adapters preserve design semantics rather than forking a second theme.
- [ ] Contrast, reduced motion, target size, icon accessibility, rich focus marker and forced-colors as relevant pass actual app inspections.
- [ ] TV compatibility build looks correct under M76 and responds to D-pad/Back/media keys, with legible focus at couch distance.
- [ ] Offline startup without remote fonts/CDN/scripts; no unnecessary animations, expensive blur or multiple decoder previews.
- [ ] Cinematic UI does not degrade real video frame pacing, audio/seek priority or poster-scroll interaction.

**Status:** Canonical upstream researched. No DS code was vendored, generated, shipped, modified or physically tested by this planning revision.
