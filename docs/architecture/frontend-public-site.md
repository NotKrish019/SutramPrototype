# Frontend Public Site (`/`)

Owner: Member 3 (Frontend & Visualization). Routing is described in
[`docs/decisions/0001-frontend-client-side-routing.md`](../decisions/0001-frontend-client-side-routing.md).

## Structure

```
apps/web/src/
├── pages/LandingPage.tsx              # composes the sections below
├── pages/NotFoundPage.tsx
├── router/                            # router.ts, Link.tsx
├── styles/site.css                    # board materials, thread/settle/snap motion, reduced-motion rules
├── components/ui/primitives.tsx       # brand primitives shared with the workspace
├── fixtures/synthetic/case026.ts      # CASE-026, synthetic and typed (shared with the workspace)
└── features/landing/
    ├── data/siteContent.ts            # copy, nav anchors, capability strip, build status
    ├── hooks/useInView.ts             # reveal on scroll + scroll-spy
    ├── board/                         # the evidence-board material library
    │   ├── materials.tsx              # Pushpin, Thread, EvidencePrint, BoardLabel, SyntheticStamp, EditorialHeading
    │   ├── ThreadCanvas.tsx           # red threads measured between live `data-pin-id` anchors
    │   ├── hooks.ts                   # reduced motion, media queries, pointer parallax, seeded PRNG
    │   ├── artwork/                   # original SVG artwork: CCTV still, fingerprint, phone, map, documents
    │   └── wireframe/                 # model.ts (geometry) + WireframeModel.tsx (projected SVG 3D, drag/keys)
    └── components/
        ├── SiteHeader.tsx             # transparent → charcoal nav, scroll-spy, mobile menu
        ├── HeroBoard.tsx              # 02 hero: pinned evidence + rotating reconstruction (desktop / compact)
        ├── EvidenceSourcesBoard.tsx   # 03 six evidence categories as pinned artifacts
        ├── CorrelationBoard.tsx       # 04 interactive correlation, related-entity path
        ├── ReconstructionSection.tsx  # 05 3D site model, sensors, route
        ├── TimelineSection.tsx        # 06 true-to-scale timeline, clock conflict
        ├── ContradictionSection.tsx   # 07 the thread that snaps
        ├── ProvenanceChain.tsx        # 08 artifact → fact → event → lead
        ├── workbench/                 # 09 ProductPreview, EvidencePanel, KnowledgeGraph,
        │                              #    EventTimeline, ProvenancePanel, selection.ts
        ├── TechnologySection.tsx      # pipeline + honest build-status ledger
        └── SiteFooter.tsx             # 10 final brand moment + footer (live API status)
```

## Art direction

The identity follows `referenceImage.jpeg` in the repository root: a dark investigation board with
pinned prints, red string, fingerprint and CCTV evidence, a wireframe reconstruction and editorial
type. It is rebuilt from components; the image itself is never used as page content, and the poster's
event, organiser and team branding is not reproduced.

Tokens live in `tailwind.config.js`:

| Group | Tokens |
| --- | --- |
| Surfaces | `ink-950…700`, `ink-line`, `ink-strong` (near-black → charcoal) |
| Text | `fg` (warm ivory), `fg-bright`, `fg-muted`, `fg-subtle` |
| Evidence materials | `paper`, `paper-light`, `paper-dark` (parchment prints and documents) |
| Accent | `crimson`, `crimson-deep`, `crimson-muted` (threads, critical indicators) |
| Reconstruction | `steel`, `steel-dim` (wireframe geometry) |
| Type | `font-serif` (Fraunces, editorial headlines), `font-sans` (Inter), `font-mono` (JetBrains Mono, IDs and timestamps) |
| Layout | `max-w-site`, `max-w-wide`, `px-gutter`, `py-section` |
| Motion | `duration-fast/base/slow`, `ease-out` |

Fonts load from Google Fonts and fall back to the system stack when offline. Green (`signal-ok`) is a
small semantic status colour only.

**Threads carry meaning.** Observed and supplied links are solid; inferred links are dashed and labelled
for review. A thread that snaps marks a contradiction, never a conclusion.

### Thread rules

- Threads are drawn between measured `data-pin-id` anchors (`ThreadCanvas`), so they stay attached in
  every responsive composition. It re-measures on resize, after fonts load, when revealed, and after any
  transform transition (prints settling or lifting) inside its container.
- Anchors that are not rendered at the current breakpoint are skipped.
- A thread must never cross body text or a photograph. Tie it off at a frame edge, route it under the
  print, or drop it (the evidence-sources board shows threads only at `lg` and up).
- Labels can be positioned along a thread with `labelT` (0…1) to keep them off prints.

### 3D approach

No WebGL or 3D library is used. Depth comes from CSS perspective, per-layer pointer parallax, settle
transforms and shadows. The spatial reconstruction is a small projected SVG model (`board/wireframe`)
with drag and arrow-key rotation and clickable sensor markers. This keeps the bundle small, works on
low-powered devices, and needs no WebGL fallback.

## Content rules

- All case data is fictional and labelled **Synthetic demonstration data**. Nothing on the page is produced
  by the backend.
- Deliverable status labels (`Planned`, `In development`, `Prototype`) reflect the repository. Update
  `DELIVERABLES` in `siteContent.ts` when a module ships.
- Inferred links are never presented as proof. Leads are framed as items to verify.
- Hashing copy: digests help detect change after acquisition. They do not by themselves establish authenticity.

## Accessibility and motion

- Semantic sections with headings, and a skip link.
- Tabs follow the ARIA tabs pattern (arrow keys, Home/End, roving tabindex).
- Mobile menu: `aria-expanded`, Escape and outside-click close it, focus returns to the toggle.
- SVG diagrams have titles. Interactive graph nodes are focusable `role="button"` elements with Enter/Space support.
- `prefers-reduced-motion` disables flow animations, reveal transitions and smooth scrolling.
- Reveal-on-scroll is a progressive enhancement: content is visible when `IntersectionObserver` is unavailable.

## Integration with existing code

- The footer API status uses the existing `useBackendHealth` hook. The workspace at `/workspace` is unchanged.
- `/workspace`'s header brand links back to `/`.

## Visual-first principles

- Each section has one heading, at most one supporting sentence, and one dominant visual.
- The workspace preview (section 09) reuses the real workbench components with synthetic data.
- All continuous motion (auto-rotation, thread pulses, parallax, settle and snap) stops under
  `prefers-reduced-motion`; content is shown in its final state.
