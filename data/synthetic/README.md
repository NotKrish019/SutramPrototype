# Synthetic Evidence Directory

This folder is reserved for lightweight, synthetic test fixtures used in integration tests and automated development pipelines.

### Supported Synthetic Modalities (Phase 1+):
- **Mock Call Detail Records (CDR):** Synthetic CSVs with simulated IMSI, IMEI, cell tower IDs, and timestamps.
- **Mock Chat Transcripts:** Anonymized JSON logs simulating instant messaging threads.
- **Mock Document Scans:** Synthetic PDFs and TIFFs containing known ground-truth text for OCR benchmarking.
- **Mock CCTV Metadata:** Frame sequences and timestamped detection bounding boxes.

### Synthetic Fixtures (Phase 1B):

#### 1. `sample_cdr.csv`
- `record_id`: Unique record identifier for each CDR row.
- `caller_phone`: Originating party phone number string as recorded by carrier.
- `receiver_phone`: Destination party phone number string.
- `call_timestamp`: ISO 8601 formatted call start timestamp as recorded.
- `duration_seconds`: Integer/float call duration in seconds.
- `cell_tower_id`: Base Transceiver Station / Cell tower identifier.
- `call_type`: Telephony call direction (`VOICE_OUTGOING`, `VOICE_INCOMING`, `MISSED_CALL`).

#### 2. `sample_chat_export.json`
- `export_format`: Format descriptor (`whatsapp_chat_export_v1`).
- `export_timestamp`: UTC export generation instant.
- `chat_title`: Conversation or group subject.
- `chat_id`: Unique identifier for the chat channel.
- `messages`: Array of message objects:
  - `message_id`: Unique message identifier.
  - `sender_id`: Originating phone number or user handle.
  - `sender_name`: Display name associated with sender in export.
  - `recipients`: List of recipient phone numbers or group members.
  - `timestamp`: ISO 8601 message transmission timestamp.
  - `content`: Raw message text body containing conversational text and embedded identifiers.

### Integrity Requirements:
Every synthetic file placed here should have an accompanying `.sha256` checksum file to validate pipeline hashing logic.

### Demo case (`demo/`)

A small synthetic investigation used by `yukti demo` and the backend tests: two operators' call records and an Android WhatsApp `.txt` export. It produces ROBUST, SENSITIVE and FRAGILE contradiction leads. See [`demo/README.md`](demo/README.md).
