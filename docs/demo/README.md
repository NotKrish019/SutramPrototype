# Demo: forensic report of the synthetic warehouse burglary

These files are the Phase 3I sample report for case `case-wh-burglary-001`. The case is fully
synthetic: every person, number, place and file is fabricated
(`backend/tests/reconstruction/adapters/synthetic_burglary.py`).

| File | Format |
| :--- | :--- |
| [`forensic-report-warehouse-burglary.json`](forensic-report-warehouse-burglary.json) | Structured report (`GET …/report`) |
| [`forensic-report-warehouse-burglary.md`](forensic-report-warehouse-burglary.md) | Markdown (`?format=markdown`) |
| [`forensic-report-warehouse-burglary.html`](forensic-report-warehouse-burglary.html) | Print-ready HTML (`?format=html`); open it and print to PDF |

**Contract status.** DEC-001 … DEC-007 are `PROPOSED`, not team-approved. The default
(production) configuration fails closed and would reconstruct nothing. This sample therefore
enables the proposal explicitly, and the report lists every decision as "enabled without verified
team sign-off (proposal only)". Every finding below is conditional on those decisions being
approved.

What the sample shows: 5 timeline events with raw and corrected UTC (the CCTV DVR runs 120 s
fast), 1 movement segment, 1 speed-threshold analytical lead (25.3 km in at most 300 s), 1
`CONTRADICTS` relationship with weight "Not provided", and limitations including uncalibrated
clocks and skipped video frame extraction (placeholder evidence bytes keep the sample
reproducible on any machine).

Regenerate after any change to the pipeline or the report (a test fails when these files are
stale):

```bash
cd backend && uv run python -m tests.integration.reconstruction.demo_report
```
