# SUTRAM Digital Forensics — Decision Support Service

## Purpose & Architecture
This microservice is reserved for hosting **Laya (Receptron)** decision intelligence and ONNX Runtime model inference.

In future phases, it will evaluate:
- Investigator hypotheses regarding suspect alibis.
- Plausibility scores for reconstructed criminal sequences.
- Contradiction weighting across multi-source evidence.

## Phase 0 Status
- **Current State:** Lightweight HTTP service providing health telemetry and contract endpoints.
- **Model Status:** Model weights and ONNX runtime are deferred to Phase 5. No heavy weights or inference engines are loaded in Phase 0.

## Development Scripts
```bash
# Install dependencies
npm ci

# Run test suite
npm run test

# Start development server
npm run dev

# Build TypeScript to dist/
npm run build
```
- Health Check: `GET http://localhost:3001/health`
