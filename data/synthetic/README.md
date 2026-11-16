# Synthetic Evidence Directory

This folder is reserved for lightweight, synthetic test fixtures used in integration tests and automated development pipelines.

### Supported Synthetic Modalities (Phase 1+):
- **Mock Call Detail Records (CDR):** Synthetic CSVs with simulated IMSI, IMEI, cell tower IDs, and timestamps.
- **Mock Chat Transcripts:** Anonymized JSON logs simulating instant messaging threads.
- **Mock Document Scans:** Synthetic PDFs and TIFFs containing known ground-truth text for OCR benchmarking.
- **Mock CCTV Metadata:** Frame sequences and timestamped detection bounding boxes.

### Integrity Requirements:
Every synthetic file placed here should have an accompanying `.sha256` checksum file to validate pipeline hashing logic.
