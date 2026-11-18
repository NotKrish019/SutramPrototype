import { DEFAULT_CASE_ID, SYNTHETIC_CASES, SYNTHETIC_CASE_DETAILS } from '../../../fixtures/synthetic/workspace';
import type { CaseDetail, DataSource, WorkspaceCase } from '../types';

/**
 * Data access for the workspace. The backend currently exposes only `/health`,
 * so the only implementation is the synthetic one below. A future API-backed
 * repository implements the same interface (see docs/architecture/frontend-workspace.md).
 */
export interface WorkspaceRepository {
  readonly source: DataSource;
  /** Case to open when none is selected yet. */
  readonly defaultCaseId: string | null;
  listCases(signal: AbortSignal): Promise<WorkspaceCase[]>;
  /** Resolves `null` when the case has no detailed records available. */
  getCaseDetail(caseId: string, signal: AbortSignal): Promise<CaseDetail | null>;
}

function abortable<T>(signal: AbortSignal, value: () => T): Promise<T> {
  if (signal.aborted) return Promise.reject(new DOMException('The request was aborted.', 'AbortError'));
  return Promise.resolve().then(value);
}

export const syntheticRepository: WorkspaceRepository = {
  source: 'synthetic',
  defaultCaseId: DEFAULT_CASE_ID,
  listCases: (signal) => abortable(signal, () => SYNTHETIC_CASES),
  getCaseDetail: (caseId, signal) => abortable(signal, () => SYNTHETIC_CASE_DETAILS[caseId] ?? null),
};
