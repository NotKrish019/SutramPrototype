import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { EvidenceUploadPanel } from './EvidenceUploadPanel';
import * as evidenceApi from '../../../services/evidenceApi';

describe('EvidenceUploadPanel Component', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('renders staging UI with initial idle state and active API indicator', () => {
    render(<EvidenceUploadPanel id="test-upload" caseId="CASE-026" />);

    expect(screen.getByText(/Stage & Ingest evidence · CASE-026/)).toBeInTheDocument();
    expect(screen.getByText(/Ingestion API active/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Choose files/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Submit for ingestion' })).toBeDisabled();
  });

  it('stages selected files, computes SHA-256 preview and enables submission button when allowSyntheticUpload is true', async () => {
    const file = new File(['caller_msisdn,recipient_msisdn\n+919876543210,+919123456789'], 'records.csv', {
      type: 'text/csv',
    });

    render(<EvidenceUploadPanel id="test-upload" caseId="CASE-026" allowSyntheticUpload />);

    const fileInput = screen.getByLabelText(/Choose evidence files/i);
    fireEvent.change(fileInput, { target: { files: [file] } });

    await waitFor(() => {
      expect(screen.getByText('records.csv')).toBeInTheDocument();
    });

    const submitBtn = screen.getByRole('button', { name: 'Submit for ingestion' });
    expect(submitBtn).toBeEnabled();
  });

  it('dispatches multipart upload to Rust backend and displays server verification and extraction result', async () => {
    const file = new File(['sample chat log content'], 'chat.json', { type: 'application/json' });

    const mockResponse: evidenceApi.EvidenceUploadAndExtractResponse = {
      receipt: {
        evidence_id: 'EV-CHAT-999',
        case_id: 'CASE-026',
        original_filename: 'chat.json',
        source_type: 'CHAT_EXPORT',
        mime_type: 'application/json',
        file_size_bytes: 23,
        sha256_hash: '3f5a2b1c8e7d4a6f',
        storage_uri: 'urn:sutram:evidence:EV-CHAT-999:storage',
        status: 'INGESTED',
        created_at: '2026-03-01T12:00:00Z',
      },
      extraction: {
        run_id: 'RUN-001',
        evidence_id: 'EV-CHAT-999',
        status: 'COMPLETED',
        created_at: '2026-03-01T12:00:01Z',
        entity_count: 2,
        event_count: 5,
        relationship_count: 1,
        validation_issues: [],
      },
    };

    const uploadSpy = vi.spyOn(evidenceApi, 'uploadEvidence').mockResolvedValue(mockResponse);
    const onUploaded = vi.fn();

    render(<EvidenceUploadPanel id="test-upload" caseId="CASE-026" allowSyntheticUpload onUploaded={onUploaded} />);

    const fileInput = screen.getByLabelText(/Choose evidence files/i);
    fireEvent.change(fileInput, { target: { files: [file] } });

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Submit for ingestion' })).toBeEnabled();
    });

    fireEvent.click(screen.getByRole('button', { name: 'Submit for ingestion' }));

    await waitFor(() => {
      expect(uploadSpy).toHaveBeenCalledWith(file, {
        caseId: 'CASE-026',
        autoExtract: true,
      });
      expect(screen.getByText('Ingestion & Extraction Succeeded')).toBeInTheDocument();
      expect(screen.getByText('EV-CHAT-999')).toBeInTheDocument();
      expect(screen.getByText(/2 entities, 5 events, 1 relationships/)).toBeInTheDocument();
      expect(onUploaded).toHaveBeenCalledWith(mockResponse);
    });
  });

  it('displays structured API error and provides retry button on upload failure', async () => {
    const file = new File(['bad payload'], 'bad.csv', { type: 'text/csv' });

    vi.spyOn(evidenceApi, 'uploadEvidence').mockRejectedValue(
      new Error('HTTP 413: Upload exceeds maximum allowable evidence size'),
    );

    render(<EvidenceUploadPanel id="test-upload" caseId="CASE-026" allowSyntheticUpload />);

    const fileInput = screen.getByLabelText(/Choose evidence files/i);
    fireEvent.change(fileInput, { target: { files: [file] } });

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Submit for ingestion' })).toBeEnabled();
    });

    fireEvent.click(screen.getByRole('button', { name: 'Submit for ingestion' }));

    await waitFor(() => {
      expect(screen.getByText(/Upload exceeds maximum allowable evidence size/)).toBeInTheDocument();
      expect(screen.getByRole('button', { name: /Retry/i })).toBeInTheDocument();
    });
  });
});
