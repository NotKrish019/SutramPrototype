# Frontend Investigator Workspace (`/workspace`)

Owner: Member 3 (Frontend & Visualization). Shares its design system with the public site
([`frontend-public-site.md`](frontend-public-site.md)). Routing is described in
[`0001-frontend-client-side-routing.md`](../decisions/0001-frontend-client-side-routing.md).

## Routes

| Path | View |
| --- | --- |
| `/workspace` | Overview: case portfolio, investigations table, backend telemetry, data sources, activity |
| `/workspace/cases/:caseId` | Case overview |
| `…/evidence` | Evidence repository and local evidence staging |
| `…/correlation` | Evidence × entity matrix and relationship ledger |
| `…/graph` | Knowledge graph workspace |
| `…/timeline` | Event reconstruction (swimlanes and chronology) |
| `…/spatial` | Schematic site plan |
| `…/contradictions` | Contradiction and inconsistency review |
| `…/reports` | Report preview |
| `/workspace/settings`, `/workspace/help` | Read-only configuration; glossary and shortcuts |

Selections live in the query string, so they can be deep-linked across views. Query-only updates replace
the history entry and keep the scroll position.

| View | Query parameter |
| --- | --- |
| Evidence | `?evidence=EV-02` |
| Graph | `?node=ENT-PA` or `?edge=R2` |
| Timeline | `?event=EVT-3` |
| Contradictions | `?item=DX-01` |
| Spatial | `?place=PL-AP` |
| Correlation | `?rel=R2` |

## Structure

```
src/features/workspace/
├── types.ts                 # view models: shared/schemas field names, canonical enum values only
├── format.ts                # precision-aware time formatting (never more precise than recorded)
├── caseIndex.ts             # id lookups + URL-backed selection hook
├── data/
│   ├── repository.ts        # WorkspaceRepository interface + synthetic implementation
│   ├── WorkspaceDataContext.tsx  # provider, useResource (abortable, retryable), useCaseList/Detail
│   └── BackendHealthContext.tsx  # ONE useBackendHealth instance shared by top bar + overview card
├── layout/                  # WorkspaceLayout, sidebar, top bar, search, backend indicator, nav model
├── graph/graphModel.ts      # graph build + BFS shortest path
├── components/ui.tsx        # dense panels, badges, filters, empty/loading/error states
└── pages/                   # one component per view
src/fixtures/synthetic/      # case026.ts (shared with the site) + workspace.ts
src/components/ui/primitives.tsx  # brand primitives shared by site and workspace
src/pages/HomePage.tsx       # workspace overview (`/workspace`)
```

## Data sources

The backend currently exposes only `GET /health`:

- **Connectivity is live.** It uses the existing API client and `useBackendHealth` (typed errors, timeout,
  cancellation, 15 s polling, hidden-tab pause, Retry Now). One instance is provided via context and is
  read by both the top-bar indicator and the restyled `BackendStatusCard`.
- **Case records are synthetic.** `syntheticRepository` serves `src/fixtures/synthetic/workspace.ts`. Every
  surface that shows synthetic content is labelled **Synthetic demonstration data**.

To integrate a real API, implement `WorkspaceRepository` (`listCases`, `getCaseDetail`, `source: 'api'`) and
pass it to `<WorkspaceApp repository={…}>`. No view changes are needed.

## Honesty rules enforced in code and tests

- **Enum values are never invented.** Where the canonical enums have no suitable value (access-control
  logs, network logs, payment cards), the field is `null` and labelled "no canonical type".
- **`weight` and `confidence` are never fabricated.** `null` renders as "Not provided", never `0`.
- **Inferred relationships are marked.** They have `is_observed: false` and are drawn dashed amber.
  `CONTRADICTS` uses a distinct red treatment.
- **Timestamps keep their recorded precision.** They show the source's recorded value and time zone,
  uncertainty windows, and `CONCURRENT_UNCERTAIN` ordering. Conflicts are drawn, not resolved.
- **Spatial is a schematic plan, not a map.** AP association is an area of uncertainty, and no movement is
  reconstructed without location records.
- **Unsupported operations are explained, not simulated:**
  - Case creation is not offered.
  - Upload staging hashes files locally only, and **Submit** is disabled.
  - Report export is disabled.
  - Notes, accounts and notifications are absent.

## Contract gaps surfaced for the team

- `EvidenceType` lacks values for access-control logs and network/syslog exports.
- `EntityType` lacks payment card / bank account.
- No case, evidence, entity, event, relationship, activity or report endpoints exist yet.
- Member 4's contradiction engine (`ContradictionLead`) is not exposed via API.

## Visualisation libraries

Cytoscape.js and Leaflet are not installed. Adding them needs team approval, and Leaflet would also need a
tile service. The graph is a dependency-free SVG with zoom, pan, fit, search, filters, selection and path
tracing. Spatial is a schematic SVG plan. Both can be swapped behind the same page components later.

## Visual hierarchy (workspace refinement)

Every case view follows the same order: one dominant investigation visual, a compact strip of key
figures, supporting evidence previews, and secondary detail in a contextual inspector. Explanatory copy is
kept to short labels; caveats that affect interpretation (synthetic data, clock uncertainty, "not evidence
of wrongdoing", "movement not reconstructed") stay visible.

| View | Primary visual | Secondary detail |
| --- | --- | --- |
| `/workspace` | Metric strip + "In focus" case with evidence thumbnails | Case table, backend telemetry, activity |
| Overview | Relationship graph preview (entities + events) | Review queue with A ≠ B comparison, site plan, timeline strip, evidence gallery |
| Evidence | Gallery (list view available) | Inspector: preview → provenance → hash → links; raw metadata collapsed |
| Graph | Full-width canvas, colour by node type | Node-type chips double as legend and filter; relationship filters collapsed |
| Timeline | Swimlanes (true to scale) | Event inspector with supporting-artifact preview |
| Spatial | Schematic plan, or synthetic 3D model for CASE-026 (sensors only, no route) | Place inspector with evidence previews |
| Contradictions | Records side by side with ≠ connector and a clock axis | Events, entities, verification, limitations |

### Evidence previews

`components/EvidencePreview.tsx` shows original synthetic artwork (shared with the public site) only for
artifacts registered by `case_id/evidence_id` **and** only when the repository source is `synthetic`.
Anything else, including future API evidence, gets a neutral type glyph and file extension, never an
invented picture. Synthetic previews carry a "Synthetic" tag.

The overview timeline strip compresses quiet gaps over 15 minutes into a marked axis break and says so in
its header; within each cluster spacing is true to scale. The full timeline view remains strictly true to
scale.

### Motion

`styles/workspace.css` provides an inspector slide-in, a view fade on section change, and one-time path
drawing. All of it is disabled under `prefers-reduced-motion`.

### Known limitations

- The workspace reuses landing artwork and the 3D model from `features/landing/board/`; if they become
  shared across more features they should move to a shared components folder.
- Timeline ranges in `EventReconstruction.tsx` are fixed to CASE-026's evening (pre-existing).
