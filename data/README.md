# Data Management & Forensic Evidence Policy

This directory documents datasets used within **YUKTI Digital Forensics**.

## STRICT RULES:
1. **NO REAL FORENSIC EVIDENCE:** Under no circumstances should real victim, suspect, or sensitive evidentiary data be committed to this repository.
2. **SYNTHETIC ONLY:** Only synthesized or mock evidentiary artifacts (mock CDR records, synthetic chat exports, fabricated CCTV sample frames, mock file system images) are permitted.
3. **LARGE FILES:** Datasets exceeding 50 MB should be stored externally and retrieved via automation scripts (`scripts/`).
4. **SECURE STORAGE:** Local raw evidence ingested during testing should be directed to the path designated in `EVIDENCE_STORAGE_PATH` (configured in `.env`), which is explicitly ignored by `.gitignore`.
