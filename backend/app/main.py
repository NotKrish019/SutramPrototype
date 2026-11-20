from fastapi import FastAPI, HTTPException, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse

from app.core.config import settings
from app.api.routes import health, evidence, reconstruction


def create_application() -> FastAPI:
    """Application factory for YUKTI digital forensics backend."""
    application = FastAPI(
        title="YUKTI Digital Forensics API",
        description="AI-Powered Digital Evidence Correlation and Crime Reconstruction Platform",
        version=settings.VERSION,
        docs_url="/docs",
        redoc_url="/redoc",
        openapi_url="/openapi.json",
    )

    # CORS configuration
    application.add_middleware(
        CORSMiddleware,
        allow_origins=settings.CORS_ORIGINS,
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )

    # Include route modules
    application.include_router(health.router)
    application.include_router(evidence.router)
    application.include_router(reconstruction.router)

    # Global exception handler
    @application.exception_handler(Exception)
    async def global_exception_handler(request: Request, exc: Exception):
        if isinstance(exc, HTTPException):
            return JSONResponse(
                status_code=exc.status_code,
                content={"detail": exc.detail},
                headers=exc.headers,
            )
        return JSONResponse(
            status_code=500,
            content={
                "error": {
                    "code": "INTERNAL_SERVER_ERROR",
                    "message": "An unexpected server error occurred.",
                    "details": str(exc) if settings.APP_ENV == "development" else None,
                }
            },
        )

    return application


app = create_application()
