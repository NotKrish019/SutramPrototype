import { requestJson, type RequestOptions } from './http';
import type { CaseStatus } from '../types';

/** Case record as returned by `GET /api/v1/cases` on the Rust API. */
export interface CaseRecordDto {
  case_id: string;
  title: string;
  description: string | null;
  fir_number: string | null;
  status: CaseStatus;
  investigator_id: string | null;
  created_at: string;
  updated_at: string;
}

const STATUSES: readonly CaseStatus[] = ['DRAFT', 'ACTIVE', 'UNDER_REVIEW', 'ARCHIVED'];

function str(value: unknown): string | null {
  return typeof value === 'string' && value.trim() !== '' ? value : null;
}

export function parseCaseRecord(data: unknown): CaseRecordDto {
  if (typeof data !== 'object' || data === null) throw new TypeError('Case record must be an object.');
  const o = data as Record<string, unknown>;
  const caseId = str(o.case_id);
  if (!caseId) throw new TypeError('Case record is missing case_id.');
  const status = STATUSES.includes(o.status as CaseStatus) ? (o.status as CaseStatus) : 'ACTIVE';
  return {
    case_id: caseId,
    title: str(o.title) ?? caseId,
    description: str(o.description),
    fir_number: str(o.fir_number),
    status,
    investigator_id: str(o.investigator_id),
    created_at: str(o.created_at) ?? new Date(0).toISOString(),
    updated_at: str(o.updated_at) ?? str(o.created_at) ?? new Date(0).toISOString(),
  };
}

export function parseCaseList(data: unknown): CaseRecordDto[] {
  if (!Array.isArray(data)) throw new TypeError('Case list must be an array.');
  return data.map(parseCaseRecord);
}

/** Every case registered on the API (created from the CLI, the web, or the demo). */
export function fetchCases(options: RequestOptions = {}): Promise<CaseRecordDto[]> {
  return requestJson('/api/v1/cases', parseCaseList, options);
}

export function fetchCase(caseId: string, options: RequestOptions = {}): Promise<CaseRecordDto> {
  return requestJson(`/api/v1/cases/${encodeURIComponent(caseId)}`, parseCaseRecord, options);
}
