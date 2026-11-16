export type CaseStatus = 'DRAFT' | 'ACTIVE' | 'UNDER_REVIEW' | 'ARCHIVED';

export type EvidenceType =
  | 'CCTV'
  | 'CDR'
  | 'CHAT_EXPORT'
  | 'DOCUMENT_SCAN'
  | 'DISK_IMAGE'
  | 'AUDIO_RECORDING'
  | 'FINANCIAL_LEDGER';

export type EntityType =
  | 'PERSON'
  | 'PHONE_NUMBER'
  | 'VEHICLE'
  | 'LOCATION'
  | 'ORGANIZATION'
  | 'DEVICE'
  | 'CRYPTO_WALLET';

export interface HealthCheckResponse {
  status: string;
  project: string;
}

export interface CaseSummary {
  case_id: string;
  title: string;
  fir_number?: string;
  description: string;
  status: CaseStatus;
  created_at: string;
}
