from typing import List
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    """Application configuration settings loaded from environment or .env file."""

    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        extra="ignore",
    )

    PROJECT_NAME: str = "sutram-digital-forensics"
    VERSION: str = "0.1.0"
    APP_ENV: str = "development"
    APP_HOST: str = "0.0.0.0"
    APP_PORT: int = 8000

    # CORS configuration
    CORS_ORIGINS: List[str] = [
        "http://localhost:5173",
        "http://127.0.0.1:5173",
        "http://localhost:3000",
    ]

    # Database URLs (Placeholders for Phase 1+)
    DATABASE_URL: str = "postgresql+asyncpg://postgres:postgres@localhost:5432/sutram_forensics"
    NEO4J_URI: str = "bolt://localhost:7687"
    NEO4J_USER: str = "neo4j"
    NEO4J_PASSWORD: str = "ChangeMeInLocalEnv123!"

    # Storage paths and upload constraints
    EVIDENCE_STORAGE_PATH: str = "./data/evidence_storage"
    MAX_UPLOAD_SIZE_BYTES: int = 50 * 1024 * 1024  # 50 MB default ceiling
    ALLOWED_EVIDENCE_EXTENSIONS: List[str] = [
        ".csv",
        ".json",
        ".txt",
        ".pdf",
        ".mp4",
        ".mov",
        ".png",
        ".jpg",
        ".jpeg",
    ]
    DECISION_SERVICE_URL: str = "http://localhost:3001"


settings = Settings()
