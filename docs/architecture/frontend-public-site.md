# Frontend Public Site (`/`)

Owner: Member 3 (Frontend & Visualization). Routing is described in
[`docs/decisions/0001-frontend-client-side-routing.md`](../decisions/0001-frontend-client-side-routing.md).

## Structure

```
apps/web/src/
├── pages/LandingPage.tsx              # composes the sections below
├── pages/NotFoundPage.tsx
├── router/                            # router.ts, Link.tsx
├── styles/site.css                    # motion system, patterns, reduced-motion rules
├── components/ui/primitives.tsx       # brand primitives shared with the workspace
├── fixtures/synthetic/case026.ts      # CASE-026, synthetic and typed (shared with the workspace)
└── features/landing/
    ├── data/siteContent.ts            # copy, nav anchors, capabilities, deliverable status
    ├── hooks/useInView.ts             # reveal on scroll + scroll-spy
    └── components/
        ├── SiteHeader.tsx             # announcement bar, sticky nav, mobile menu
        ├── HeroSection.tsx + HeroStack3D.tsx    # CSS-3D evidence stack (no 3D library)
        ├── EvidenceSourcesSection.tsx + EvidenceNetworkVisual.tsx
        ├── PlatformSection.tsx + platformVisuals.tsx
        ├── workbench/                 # ProductPreview, EvidencePanel, KnowledgeGraph,
        │                              # EventTimeline, ProvenancePanel, selection.ts
        ├── WorkflowSection.tsx
        ├── ProvenanceSection.tsx
        ├── DeliverablesSection.tsx
        └── SiteFooter.tsx             # final CTA + footer (live API status)
```

## Design system

Tokens live in `tailwind.config.js`:

| Group | Tokens |
| --- | --- |
| Surfaces | `ink-950…700`, `ink-line`, `ink-strong` |
| Text | `fg`, `fg-muted`, `fg-subtle` |
| Type | `font-display` (Archivo, condensed via `site.css`), `font-sans` (Inter), `font-mono` (JetBrains Mono); `text-display-xl/lg/md`, `text-eyebrow`, `text-micro` |
| Layout | `max-w-site`, `px-gutter`, `py-section` |
| Motion | `duration-fast/base/slow`, `ease-out` |

Fonts load from Google Fonts and fall back to the system stack when offline.

**Colour carries meaning.** It is used consistently across the hero, graph, timeline and panels:

| Signal | Token | Drawn as |
| --- | --- | --- |
| Observed | `signal-observed` | solid line |
| Supplied | `signal-supplied` | dotted line |
| Inferred | `signal-amber` (also the brand accent) | dashed line |
| Flagged / contradiction | `signal-flag` | red |
| Verified / healthy | `signal-ok` | green |

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

## Visual-first principles (refinement pass)

- Each section has one heading, at most one supporting sentence, and one dominant visual.
- The hero is a CSS 3D stack: evidence floor → correlation → knowledge graph → timeline. Only the floor is
  interactive. Upper planes and beams are `pointer-events: none`, which is verified with `elementFromPoint`
  at all three widths.
- Platform tab C reuses the workspace's real `Swimlanes` and `SitePlan` components, so the site shows the
  product itself, not an illustration of it.
- All 3D and continuous motion stops under `prefers-reduced-motion`. The 3D entrance is transform-only,
  because opacity below 1 would flatten the 3D context.
