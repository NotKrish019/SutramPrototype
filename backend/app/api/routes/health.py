from fastapi import APIRouter
from app.schemas.health import HealthResponse

router = APIRouter(tags=["Health"])


@router.get(
    "/health",
    response_model=HealthResponse,
    summary="Health check endpoint",
    description="Returns the operational status of the YUKTI backend service.",
)
async def get_health() -> HealthResponse:
    """Return operational health status."""
    return HealthResponse(status="ok", project="sutram-digital-forensics")
