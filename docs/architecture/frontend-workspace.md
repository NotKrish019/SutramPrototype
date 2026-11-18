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
