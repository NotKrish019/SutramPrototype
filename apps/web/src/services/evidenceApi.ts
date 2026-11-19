import { ApiError, parseErrorBody } from './apiError';
import { buildApiUrl, DEFAULT_REQUEST_TIMEOUT_MS } from './config';
import { requestJson, type RequestOptions } from './http';

export interface EvidenceUploadReceipt {
  evidence_id: string;
  case_id: string;
  original_filename: string;
  source_type: string;
  mime_type: string;
  file_size_bytes: number;
  sha256_hash: string;
  storage_uri: string;
  status: string;
  created_at: string;
}

export interface ExtractionValidationIssue {
  severity: string;
  issue_type: string;
  message: string;
  record_ref?: string;
}

export interface ExtractionRunSummary {
  run_id: string;
  evidence_id: string;
  status: string;
  created_at: string;
  entity_count: number;
  event_count: number;
  relationship_count: number;
  validation_issues: ExtractionValidationIssue[];
}

export interface EvidenceUploadAndExtractResponse {
  receipt: EvidenceUploadReceipt;
  extraction: ExtractionRunSummary;
}

export interface UploadEvidenceOptions {
  caseId?: string;
  autoExtract?: boolean;
  sourceType?: string;
  sourceTimezone?: string;
  clockOffsetSeconds?: number;
  signal?: AbortSignal;
  timeoutMs?: number;
}

function parseUploadReceipt(data: unknown): EvidenceUploadReceipt {
  if (typeof data !== 'object' || data === null) {
    throw new TypeError('Upload receipt must be an object.');
  }
  const obj = data as Record<string, unknown>;
  return {
    evidence_id: String(obj.evidence_id || ''),
    case_id: String(obj.case_id || ''),
    original_filename: String(obj.original_filename || ''),
    source_type: String(obj.source_type || 'UNKNOWN'),
    mime_type: String(obj.mime_type || 'application/octet-stream'),
    file_size_bytes: Number(obj.file_size_bytes || 0),
    sha256_hash: String(obj.sha256_hash || ''),
    storage_uri: String(obj.storage_uri || ''),
    status: String(obj.status || 'INGESTED'),
    created_at: String(obj.created_at || ''),
  };
}

function parseExtractionSummary(data: unknown): ExtractionRunSummary {
  if (typeof data !== 'object' || data === null) {
    throw new TypeError('Extraction summary must be an object.');
  }
  const obj = data as Record<string, unknown>;
  return {
    run_id: String(obj.run_id || ''),
    evidence_id: String(obj.evidence_id || ''),
    status: String(obj.status || 'COMPLETED'),
    created_at: String(obj.created_at || ''),
    entity_count: Number(obj.entity_count || 0),
    event_count: Number(obj.event_count || 0),
    relationship_count: Number(obj.relationship_count || 0),
    validation_issues: Array.isArray(obj.validation_issues)
      ? (obj.validation_issues as ExtractionValidationIssue[])
      : [],
  };
}

export function parseUploadAndExtractResponse(data: unknown): EvidenceUploadAndExtractResponse {
  if (typeof data !== 'object' || data === null) {
    throw new TypeError('Evidence upload response must be an object.');
  }
  const obj = data as Record<string, unknown>;
  if (!obj.receipt) {
    throw new TypeError('Response missing required receipt field.');
  }
  const receipt = parseUploadReceipt(obj.receipt);
  const extraction = obj.extraction
    ? parseExtractionSummary(obj.extraction)
    : {
        run_id: '',
        evidence_id: receipt.evidence_id,
        status: 'SKIPPED',
        created_at: receipt.created_at,
        entity_count: 0,
        event_count: 0,
        relationship_count: 0,
        validation_issues: [],
      };

  return { receipt, extraction };
}

/**
 * Uploads an evidence file to the Rust backend using multipart/form-data.
 * Dispatches to `POST /api/v1/evidence/upload?auto_extract=true`.
 */
export async function uploadEvidence(
  file: File,
  options: UploadEvidenceOptions = {},
): Promise<EvidenceUploadAndExtractResponse> {
  const {
    caseId,
    autoExtract = true,
    sourceType,
    sourceTimezone = 'UTC',
    clockOffsetSeconds = 0,
    signal,
    timeoutMs = DEFAULT_REQUEST_TIMEOUT_MS,
  } = options;

  const formData = new FormData();
  formData.append('file', file, file.name);

  if (caseId) {
    formData.append('case_id', caseId);
  }
  formData.append('auto_extract', autoExtract ? 'true' : 'false');
  if (sourceType) {
    formData.append('source_type', sourceType);
  }
  formData.append('source_timezone', sourceTimezone);
  formData.append('clock_offset_seconds', String(clockOffsetSeconds));

  const query = autoExtract ? '?auto_extract=true' : '?auto_extract=false';
  const url = buildApiUrl(`/api/v1/evidence/upload${query}`);

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(new DOMException('Upload timed out', 'TimeoutError')), timeoutMs);

  const forwardAbort = () => controller.abort(signal?.reason);
  if (signal) {
    if (signal.aborted) {
      clearTimeout(timer);
      throw new DOMException('The request was aborted.', 'AbortError');
    }
    signal.addEventListener('abort', forwardAbort, { once: true });
  }

  try {
    const response = await fetch(url, {
      method: 'POST',
      body: formData,
      signal: controller.signal,
      headers: {
        Accept: 'application/json',
      },
    });

    clearTimeout(timer);

    const bodyText = await response.text();
    if (!response.ok) {
      let body: unknown;
      try {
        body = bodyText === '' ? undefined : JSON.parse(bodyText);
      } catch {
        body = undefined;
      }
      const parsed = parseErrorBody(body);
      const statusLabel = `HTTP ${response.status} ${response.statusText}`.trim();
      throw new ApiError({
        kind: 'http',
        message: parsed.message ? `${statusLabel}: ${parsed.message}` : statusLabel,
        url,
        status: response.status,
        code: parsed.code,
        details: parsed.details,
      });
    }

    let json: unknown;
    try {
      json = JSON.parse(bodyText);
    } catch (e) {
      throw new ApiError({
        kind: 'parse',
        message: `Response from ${url} is not valid JSON.`,
        url,
        cause: e,
      });
    }

    return parseUploadAndExtractResponse(json);
  } catch (error) {
    clearTimeout(timer);
    if (error instanceof ApiError) throw error;
    if (signal?.aborted) throw new DOMException('The request was aborted.', 'AbortError');
    throw new ApiError({
      kind: 'network',
      message: `Network error uploading evidence to ${url}. Is the backend running at port 8080?`,
      url,
      cause: error,
    });
  } finally {
    if (signal) {
      signal.removeEventListener('abort', forwardAbort);
    }
  }
}

/**
 * Fetches the list of evidence exhibits for a specific case from the backend.
 * `GET /api/v1/cases/:case_id/evidence`
 */
export async function fetchCaseEvidence(
  caseId: string,
  options: Pick<RequestOptions, 'signal' | 'timeoutMs'> = {},
): Promise<EvidenceUploadReceipt[]> {
  const path = `/api/v1/cases/${encodeURIComponent(caseId)}/evidence`;
  return requestJson(path, (data) => {
    if (!Array.isArray(data)) return [];
    return data.map(parseUploadReceipt);
  }, options);
}

/**
 * Fetches an evidence item by ID.
 * `GET /api/v1/evidence/:evidence_id`
 */
export async function fetchEvidenceItem(
  evidenceId: string,
  options: Pick<RequestOptions, 'signal' | 'timeoutMs'> = {},
): Promise<EvidenceUploadReceipt> {
  const path = `/api/v1/evidence/${encodeURIComponent(evidenceId)}`;
  return requestJson(path, parseUploadReceipt, options);
}
