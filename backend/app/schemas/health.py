from pydantic import BaseModel, Field


class HealthResponse(BaseModel):
    """Schema for service health check response."""

    status: str = Field(default="ok", description="Operational status of the service")
    project: str = Field(
        default="sutram-digital-forensics", description="Registered project identifier"
    )
