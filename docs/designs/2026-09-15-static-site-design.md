# Design: Braid Static Documentation Site

## Goal

Create a source-backed, responsive documentation site under `site/` that presents Braid's
runtime, architecture, safety model, tests, roadmap, and release history without a build step.

## Approved Approach

Build a seven-page static docs site modeled on the navigation and maintainability of
`../minibox/assets`, with a distinct warm risograph and woven-systems visual language.

## Context Map

### Files to Add

| File | Purpose |
| --- | --- |
| `site/index.html` | Product overview and runtime flow |
| `site/architecture.html` | Hexagonal boundaries and end-to-end data flow |
| `site/crates.html` | Workspace crate inventory and dependency roles |
| `site/safety.html` | Redaction, hooks, trust boundaries, and current hardening work |
| `site/testing.html` | Test strategy, local gates, and CI coverage |
| `site/roadmap.html` | Rendered delivery phases linked to GitHub epics |
| `site/changelog.html` | Curated release history from `CHANGELOG.md` |
| `site/style.css` | Shared tokens, layout, components, responsive rules, and motion |
| `site/architecture.svg` | Accessible source-backed dependency and runtime diagram |

### Sources

| Source | Site content derived from it |
| --- | --- |
| `README.md` | Project description and crate summaries |
| `ROADMAP.md` | Delivered baseline, phases, dependencies, and issue URLs |
| `CHANGELOG.md` | Release history |
| `Cargo.toml` and `crates/*/Cargo.toml` | Workspace membership and dependency direction |
| `crates/braid-engine/src/engine.rs` | Runtime flow and injected boundaries |
| `crates/braid-ports/src/lib.rs` | Public port contracts |
| `crates/braid-cli/src/main.rs` | Operator commands and composition root |
| `.github/workflows/ci.yml` | CI gates |

### Reference Pattern

`../minibox/assets` supplies the no-build multi-page structure, sticky navigation, shared CSS,
responsive grids, and accessible diagrams. Braid will not reuse minibox's palette, copy, or
product-specific components.

## Visual System

- **Theme:** warm risograph print crossed with a technical system map.
- **Palette:** paper `#f3eee4`, raised paper `#fffaf0`, charcoal `#222522`, muted ink
  `#68675f`, coral thread `#d9573f`, deep teal `#125c60`, and rule `#cfc5b4`.
- **Typography:** Fraunces for display, Karla for prose, and Fragment Mono for code and labels,
  loaded from Google Fonts with deliberate serif/sans/mono fallbacks.
- **Atmosphere:** a low-contrast woven line grid plus offset coral/teal registration marks.
- **Motion:** one staggered page-load reveal on the landing hero, disabled for reduced motion.
- **Responsive behavior:** horizontal mobile navigation, single-column card grids, scalable type,
  and diagrams that remain legible through horizontal overflow or responsive sizing.

## Information Architecture

Every page uses the same navigation order: Overview, Architecture, Crates, Safety, Testing,
Roadmap, Changelog. The active page is visually and semantically marked with
`aria-current="page"`.

The landing page introduces the platform through the actual runtime flow rather than generic
feature marketing. Secondary pages present source-backed reference material and link back to
the repository where implementation detail belongs.

## Data Flow

1. Repository files provide authoritative project facts.
2. Facts are curated into static semantic HTML and one SVG diagram.
3. Shared CSS provides all styling and motion without JavaScript.
4. Any static file server or GitHub Pages can serve `site/` directly.

## Crate Ownership and Public API

No Rust crate owns or imports the site. No public Rust API, serialization format, CLI output,
manifest, dependency edge, or feature flag changes.

## Accessibility

- Semantic landmarks, heading order, visible focus states, and skip links.
- Sufficient text/background contrast and non-color status labels.
- Descriptive SVG title/description and useful image alternative text.
- `prefers-reduced-motion` disables the reveal animation.

## Out of Scope

- JavaScript frameworks, static-site generators, search, analytics, or deployment workflows.
- Automatically generating pages from Markdown.
- Copying minibox branding, statistics, or architecture claims.
- Modifying Rust source or existing planning documents.

## Risk

- [ ] Breaking API changes: no.
- [ ] New Rust or build dependency: no.
- [ ] External runtime dependency: Google Fonts only; readable fallbacks remain available.
- [ ] Content drift: mitigated by linking each page to its authoritative repository source.
