"""Reconstruction API routes (Member 4 — Phase 3G, report endpoint Phase 3I).

Thin handlers over ``ReconstructionService``. Every error from these routes, including request
validation errors, uses the standard envelope ``{"error": {"code", "message", "details"}}``.
Responses never contain absolute host paths, and ``weight: null`` stays ``null``. Logs carry
only run id, case id, status and counts: no payloads, paths or secrets.
"""

import logging
from collections import Counter
from collections.abc import Callable, Coroutine
from typing import Annotated, Any

from fastapi import APIRouter, Depends, Path, Query, Request, Response, status
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse
from fastapi.routing import APIRoute

from app.modules.reconstruction.pipeline import ReconstructionResult
from app.modules.reconstruction.run_store import RunNotFoundError
from app.modules.reconstruction.service import (
    ReconstructionService,
    get_reconstruction_service,
)
from app.modules.reporting.forensic_report import ForensicReport
from app.modules.reporting.report_render import render_html, render_markdown
from app.schemas.reconstruction_api import (
    SAFE_ID_PATTERN,
    ContradictionsView,
    DiagnosticsView,
    ErrorEnvelope,
    ReconstructionRunRequest,
    ReconstructionRunSummary,
    RelationshipsView,
    ReportFormat,
    RunCounts,
    RunLinks,
    SpatialView,
    TimelineView,
)

logger = logging.getLogger(__name__)


def _envelope(status_code: int, code: str, message: str, details: Any = None) -> JSONResponse:
    return JSONResponse(
        status_code=status_code,
        content={"error": {"code": code, "message": message, "details": details}},
    )


def _sanitize_validation_errors(errors: Any) -> list[dict[str, Any]]:
    """Keep location, message and type only. Never echo submitted values (they may hold paths)."""
    return [
        {"loc": list(err.get("loc", ())), "msg": err.get("msg", ""), "type": err.get("type", "")}
        for err in errors
    ]


class EnvelopeRoute(APIRoute):
    """Route class that converts every failure of these routes into the standard envelope."""

    def get_route_handler(self) -> Callable[[Request], Coroutine[Any, Any, Response]]:
        original = super().get_route_handler()

        async def handler(request: Request) -> Response:
            try:
                return await original(request)
            except RequestValidationError as exc:
                return _envelope(
                    422,
                    "REQUEST_VALIDATION_FAILED",
                    "The request is not valid.",
                    _sanitize_validation_errors(exc.errors()),
                )
            except RunNotFoundError as exc:
                return _envelope(status.HTTP_404_NOT_FOUND, "RUN_NOT_FOUND", str(exc))
            except (ValueError, TypeError) as exc:
                logger.info("reconstruction request rejected: %s", type(exc).__name__)
                return _envelope(
                    422,
                    "RECONSTRUCTION_INPUT_REJECTED",
                    "The reconstruction inputs could not be processed.",
                )
            except Exception:
                logger.exception("reconstruction request failed unexpectedly")
                return _envelope(
                    status.HTTP_500_INTERNAL_SERVER_ERROR,
                    "INTERNAL_SERVER_ERROR",
                    "An unexpected server error occurred.",
                )

        return handler


ERROR_RESPONSES: dict[int | str, dict[str, Any]] = {
    404: {"model": ErrorEnvelope, "description": "Run not found for this case."},
    422: {"model": ErrorEnvelope, "description": "Request validation failed."},
    500: {"model": ErrorEnvelope, "description": "Unexpected server error."},
}

router = APIRouter(
    prefix="/cases/{case_id}/reconstruction",
    tags=["Reconstruction"],
    route_class=EnvelopeRoute,
    responses=ERROR_RESPONSES,
)

CaseId = Annotated[str, Path(pattern=SAFE_ID_PATTERN, description="Case identifier.")]
RunId = Annotated[str, Path(pattern=SAFE_ID_PATTERN, description="Reconstruction run identifier.")]
Service = Annotated[ReconstructionService, Depends(get_reconstruction_service)]


def _links(case_id: str, run_id: str) -> RunLinks:
    base = f"/cases/{case_id}/reconstruction/runs/{run_id}"
    return RunLinks(
        result=base,
        timeline=f"{base}/timeline",
        spatial=f"{base}/spatial",
        contradictions=f"{base}/contradictions",
        relationships=f"{base}/relationships",
        diagnostics=f"{base}/diagnostics",
        report=f"{base}/report",
    )


def _base(result: ReconstructionResult) -> dict[str, Any]:
    return {
        "run_id": result.run_id,
        "case_id": result.case_id,
        "status": result.status,
        "result_sha256": result.result_sha256,
    }


@router.post(
    "/run",
    response_model=ReconstructionRunSummary,
    status_code=status.HTTP_201_CREATED,
    summary="Run an end-to-end reconstruction",
    description=(
        "Validates evidence (hash check), normalizes event candidates to UTC once, builds the "
        "timeline and spatial reconstruction, detects contradictions and exports CONTRADICTS "
        "relationships. Invalid items are excluded and reported; the run continues. Identical "
        "inputs produce the same run_id and result_sha256."
    ),
)
def run_reconstruction(case_id: CaseId, body: ReconstructionRunRequest, service: Service) -> ReconstructionRunSummary:
    result = service.run(
        case_id,
        evidence=body.evidence,
        event_candidates=body.candidates_payload(),
        entities=body.entities,
        frame_artifacts=body.frame_artifacts,
    )
    logger.info(
        "reconstruction run stored run_id=%s case_id=%s status=%s", result.run_id, case_id, result.status.value
    )
    severities = Counter(d.severity.value for d in result.diagnostics)
    return ReconstructionRunSummary(
        run_id=result.run_id,
        case_id=result.case_id,
        status=result.status,
        result_sha256=result.result_sha256,
        algorithm_version=result.algorithm_version,
        configuration_version=result.configuration_version,
        relationship_export_status=result.relationship_export_status,
        counts=RunCounts(
            timeline_events=len(result.timeline_events),
            movement_segments=len(result.movement_segments),
            contradiction_leads=len(result.contradiction_leads),
            relationships=len(result.relationships),
            excluded_inputs=len(result.excluded_inputs),
            diagnostics_by_severity={k: severities[k] for k in sorted(severities)},
        ),
        links=_links(result.case_id, result.run_id),
    )


@router.get("/runs/{run_id}", response_model=ReconstructionResult, summary="Full reconstruction result")
def get_run(case_id: CaseId, run_id: RunId, service: Service) -> ReconstructionResult:
    return service.get(case_id, run_id)


@router.get("/runs/{run_id}/timeline", response_model=TimelineView, summary="Timeline events (raw and corrected UTC)")
def get_timeline(case_id: CaseId, run_id: RunId, service: Service) -> TimelineView:
    result = service.get(case_id, run_id)
    return TimelineView(
        **_base(result),
        timeline_events=result.timeline_events,
        timeline_overlaps=result.timeline_overlaps,
    )


@router.get("/runs/{run_id}/spatial", response_model=SpatialView, summary="Spatial observations and movement segments")
def get_spatial(case_id: CaseId, run_id: RunId, service: Service) -> SpatialView:
    result = service.get(case_id, run_id)
    overlaps = (
        [
            overlap
            for entity_id in sorted(result.spatial.trajectories_by_entity)
            for overlap in result.spatial.trajectories_by_entity[entity_id].uncertain_spatial_overlaps
        ]
        if result.spatial is not None
        else []
    )
    return SpatialView(
        **_base(result),
        spatial_observations=result.spatial_observations,
        movement_segments=result.movement_segments,
        uncertain_spatial_overlaps=overlaps,
    )


@router.get("/runs/{run_id}/contradictions", response_model=ContradictionsView, summary="Contradiction leads")
def get_contradictions(case_id: CaseId, run_id: RunId, service: Service) -> ContradictionsView:
    result = service.get(case_id, run_id)
    return ContradictionsView(
        **_base(result),
        relationship_export_status=result.relationship_export_status,
        contradiction_leads=result.contradiction_leads,
        contradiction_overlaps_not_evaluated=result.contradiction_overlaps_not_evaluated,
    )


@router.get("/runs/{run_id}/relationships", response_model=RelationshipsView, summary="Canonical relationships")
def get_relationships(case_id: CaseId, run_id: RunId, service: Service) -> RelationshipsView:
    result = service.get(case_id, run_id)
    return RelationshipsView(
        **_base(result),
        relationship_export_status=result.relationship_export_status,
        relationships=result.relationships,
    )


@router.get(
    "/runs/{run_id}/diagnostics",
    response_model=DiagnosticsView,
    summary="Diagnostics, limitations, evidence gaps and video statuses",
)
def get_diagnostics(case_id: CaseId, run_id: RunId, service: Service) -> DiagnosticsView:
    result = service.get(case_id, run_id)
    return DiagnosticsView(
        **_base(result),
        diagnostics=result.diagnostics,
        limitations=result.limitations,
        evidence_gaps=result.evidence_gaps,
        excluded_inputs=result.excluded_inputs,
        video=result.video,
    )


_REPORT_MEDIA_TYPES: dict[ReportFormat, tuple[str, str]] = {
    ReportFormat.JSON: ("application/json", "json"),
    ReportFormat.MARKDOWN: ("text/markdown; charset=utf-8", "md"),
    ReportFormat.HTML: ("text/html; charset=utf-8", "html"),
}


@router.get(
    "/runs/{run_id}/report",
    response_model=ForensicReport,
    summary="Evidence-linked investigation report",
    description=(
        "Decision-support report for investigators, generated from the stored run: run metadata, "
        "evidence inventory with SHA-256 integrity status, timeline (raw and corrected UTC), "
        "movement and travel feasibility, contradiction leads (analytical leads, not facts), "
        "relationships (observed vs inferred), limitations and a chain-of-custody appendix. "
        "Every finding cites its supporting evidence. `format=json` (default) returns the "
        "structured report; `markdown` and `html` (print-ready; print to PDF from the browser) "
        "render the same content."
    ),
    responses={
        200: {
            "description": "The report in the requested format.",
            "content": {
                "text/markdown": {"schema": {"type": "string"}},
                "text/html": {"schema": {"type": "string"}},
            },
        }
    },
)
def get_report(
    case_id: CaseId,
    run_id: RunId,
    service: Service,
    report_format: Annotated[
        ReportFormat, Query(alias="format", description="json (default), markdown or html.")
    ] = ReportFormat.JSON,
) -> Response:
    report = service.report(case_id, run_id)
    media_type, extension = _REPORT_MEDIA_TYPES[report_format]
    headers = {
        "Content-Disposition": f'inline; filename="report-{run_id}.{extension}"',
        "Cache-Control": "no-store",
        "X-Content-Type-Options": "nosniff",
        "X-Report-Content-SHA256": report.content_sha256,
    }
    logger.info("reconstruction report served run_id=%s case_id=%s format=%s", run_id, case_id, report_format.value)
    if report_format == ReportFormat.MARKDOWN:
        return Response(render_markdown(report), media_type=media_type, headers=headers)
    if report_format == ReportFormat.HTML:
        headers["Content-Security-Policy"] = "default-src 'none'; style-src 'unsafe-inline'"
        return Response(render_html(report), media_type=media_type, headers=headers)
    return JSONResponse(report.model_dump(mode="json"), headers=headers)
