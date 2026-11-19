import React, { useCallback, useId, useRef, useState } from 'react';
import { CheckCircle2, ChevronRight, FileUp, Info, Loader2, RotateCw, Trash2, Upload, XCircle } from 'lucide-react';
import { WsButton } from '../components/ui';
import { formatBytes } from '../format';
import {
  uploadEvidence,
  type EvidenceUploadAndExtractResponse,
} from '../../../services/evidenceApi';
import { Link } from '../../../router/Link';
import { useWorkspaceRepository } from '../data/WorkspaceDataContext';

/** Files above this size are listed but not hashed in the browser (memory safety). */
export const LOCAL_HASH_LIMIT_BYTES = 256 * 1024 * 1024;

type HashState =
  | { status: 'hashing' }
  | { status: 'done'; sha256: string }
  | { status: 'skipped'; reason: string }
  | { status: 'error'; reason: string };

type UploadState =
  | { status: 'idle' }
  | { status: 'uploading' }
  | { status: 'success'; data: EvidenceUploadAndExtractResponse }
  | { status: 'error'; message: string };

interface StagedFile {
  key: string;
  file: File;
  hash: HashState;
  upload: UploadState;
}

const SUPPORTED = [
  'CDR · call detail records (CSV)',
  'CHAT_EXPORT · messaging exports (JSON)',
  'CCTV · video exports (MP4 / AVI / MKV)',
  'DOCUMENT_SCAN · PDF / image scans',
  'FINANCIAL_LEDGER · statements, CSV',
  'DISK_IMAGE · raw, dd, forensic images',
];

function toHex(buffer: ArrayBuffer): string {
  return Array.from(new Uint8Array(buffer), (byte) => byte.toString(16).padStart(2, '0')).join('');
}

function readBuffer(file: File): Promise<ArrayBuffer> {
  if (typeof file.arrayBuffer === 'function') return file.arrayBuffer();
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as ArrayBuffer);
    reader.onerror = () => reject(reader.error ?? new Error('Could not read file.'));
    reader.readAsArrayBuffer(file);
  });
}

/** SHA-256 of a local file, computed with Web Crypto. Exported for tests. */
export async function sha256(file: File): Promise<string> {
  if (!globalThis.crypto?.subtle) throw new Error('Web Crypto is unavailable in this context.');
  return toHex(await globalThis.crypto.subtle.digest('SHA-256', await readBuffer(file)));
}

export interface EvidenceUploadPanelProps {
  id: string;
  caseId?: string;
  allowSyntheticUpload?: boolean;
  onUploaded?: (res: EvidenceUploadAndExtractResponse) => void;
}

/**
 * Investigator Evidence Ingestion and Automated Extraction Panel.
 * Handles client-side integrity preview and dispatches multipart upload
 * directly to the Rust backend ingestion and extraction pipeline.
 */
export const EvidenceUploadPanel: React.FC<EvidenceUploadPanelProps> = ({
  id,
  caseId = 'CASE-026',
  allowSyntheticUpload = false,
  onUploaded,
}) => {
  const [files, setFiles] = useState<StagedFile[]>([]);
  const [dragging, setDragging] = useState(false);
  const [isSubmittingAll, setIsSubmittingAll] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const hintId = useId();

  // Inspect repository context to guard against accidental mutation of synthetic demonstration cases
  let repoSource = 'synthetic';
  try {
    const repo = useWorkspaceRepository();
    repoSource = repo.source;
  } catch {
    // Context unavailable in isolated unit tests
  }

  // Demonstration cases are read-only fixtures unless explicit upload is requested or in live repository mode
  const isSyntheticCase = !allowSyntheticUpload && (caseId === 'CASE-026' || repoSource === 'synthetic');

  const addFiles = useCallback((list: FileList | null) => {
    if (!list || list.length === 0) return;
    const staged = Array.from(list).map((file, index) => ({
      key: `${file.name}-${file.size}-${file.lastModified}-${Date.now()}-${index}`,
      file,
      hash:
        file.size > LOCAL_HASH_LIMIT_BYTES
          ? ({ status: 'skipped', reason: `Larger than ${formatBytes(LOCAL_HASH_LIMIT_BYTES)} — server computes SHA-256` } as const)
          : ({ status: 'hashing' } as const),
      upload: { status: 'idle' } as UploadState,
    }));
    setFiles((current) => [...current, ...staged]);

    for (const item of staged) {
      if (item.hash.status !== 'hashing') continue;
      sha256(item.file)
        .then((digest) =>
          setFiles((current) =>
            current.map((f) => (f.key === item.key ? { ...f, hash: { status: 'done', sha256: digest } } : f)),
          ),
        )
        .catch((error: unknown) =>
          setFiles((current) =>
            current.map((f) =>
              f.key === item.key
                ? { ...f, hash: { status: 'error', reason: error instanceof Error ? error.message : 'Hashing failed' } }
                : f,
            ),
          ),
        );
    }
  }, []);

  const uploadSingleFile = useCallback(
    async (item: StagedFile) => {
      setFiles((current) =>
        current.map((f) => (f.key === item.key ? { ...f, upload: { status: 'uploading' } } : f)),
      );

      try {
        const response = await uploadEvidence(item.file, {
          caseId,
          autoExtract: true,
        });

        setFiles((current) =>
          current.map((f) =>
            f.key === item.key ? { ...f, upload: { status: 'success', data: response } } : f,
          ),
        );

        if (onUploaded) {
          onUploaded(response);
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Upload failed';
        setFiles((current) =>
          current.map((f) =>
            f.key === item.key ? { ...f, upload: { status: 'error', message } } : f,
          ),
        );
      }
    },
    [caseId, onUploaded],
  );

  const handleSubmitAll = useCallback(async () => {
    if (isSyntheticCase) return;
    setIsSubmittingAll(true);
    const pending = files.filter((f) => f.upload.status === 'idle' || f.upload.status === 'error');
    for (const item of pending) {
      await uploadSingleFile(item);
    }
    setIsSubmittingAll(false);
  }, [files, isSyntheticCase, uploadSingleFile]);

  const pendingCount = files.filter((f) => f.upload.status === 'idle' || f.upload.status === 'error').length;
  const isUploadingAny = isSubmittingAll || files.some((f) => f.upload.status === 'uploading');

  return (
    <section id={id} aria-labelledby={`${id}-heading`} className="border border-ink-line bg-ink-900">
      <div className="flex items-center justify-between gap-3 border-b border-ink-line px-4 py-2.5">
        <h2 id={`${id}-heading`} className="font-mono text-micro uppercase text-fg-muted">
          Stage & Ingest evidence · {caseId}
        </h2>
        <span className="font-mono text-micro uppercase text-signal-ok flex items-center gap-1.5">
          <span className="inline-block h-2 w-2 rounded-full bg-signal-ok" /> Ingestion API active
        </span>
      </div>

      <div className="grid gap-4 p-4 lg:grid-cols-12">
        <div className="lg:col-span-7">
          <div
            onDragOver={(event) => {
              event.preventDefault();
              setDragging(true);
            }}
            onDragLeave={() => setDragging(false)}
            onDrop={(event) => {
              event.preventDefault();
              setDragging(false);
              addFiles(event.dataTransfer.files);
            }}
            className={`flex flex-col items-center justify-center gap-3 border border-dashed px-4 py-8 text-center transition-colors ${
              dragging ? 'border-signal-accent bg-signal-accent/[0.06]' : 'border-ink-strong bg-ink-950'
            }`}
          >
            <FileUp aria-hidden="true" className="h-6 w-6 text-fg-subtle" />
            <p className="text-sm text-fg">Drag files here, or</p>
            <WsButton icon={Upload} onClick={() => inputRef.current?.click()} aria-describedby={hintId}>
              Choose files
            </WsButton>
            <input
              ref={inputRef}
              type="file"
              multiple
              className="sr-only"
              aria-label="Choose evidence files"
              onChange={(event) => {
                addFiles(event.target.files);
                event.target.value = '';
              }}
            />
            <p id={hintId} className="max-w-sm text-xs leading-relaxed text-fg-subtle">
              Files stay in this browser. A SHA-256 digest is computed locally as a preview; server independently verifies hash on upload.
            </p>
          </div>

          {files.length > 0 && (
            <ul className="mt-3 divide-y divide-ink-line border border-ink-line" aria-label="Staged files">
              {files.map((item) => (
                <li key={item.key} className="flex flex-col gap-2 p-3 bg-ink-950">
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0 flex-1">
                      <p className="truncate font-mono text-xs font-semibold text-fg">{item.file.name}</p>
                      <p className="font-mono text-micro text-fg-subtle">
                        {formatBytes(item.file.size)} · {item.file.type || 'unknown type'}
                      </p>
                      <p className="mt-1 break-all font-mono text-micro" aria-live="polite">
                        {item.hash.status === 'hashing' && <span className="text-fg-muted">Computing SHA-256…</span>}
                        {item.hash.status === 'done' && (
                          <span className="text-fg-muted">
                            <span className="text-signal-ok">SHA-256 (local preview)</span> {item.hash.sha256}
                          </span>
                        )}
                        {item.hash.status === 'skipped' && <span className="text-fg-subtle">{item.hash.reason}</span>}
                        {item.hash.status === 'error' && (
                          <span className="text-signal-flag">Could not hash: {item.hash.reason}</span>
                        )}
                      </p>
                    </div>

                    <div className="flex items-center gap-2">
                      {!isSyntheticCase && item.upload.status === 'idle' && (
                        <button
                          type="button"
                          onClick={() => uploadSingleFile(item)}
                          disabled={isUploadingAny}
                          className="px-2.5 py-1 font-mono text-micro uppercase bg-ink-800 hover:bg-ink-700 text-fg rounded border border-ink-line"
                        >
                          Upload
                        </button>
                      )}
                      {item.upload.status === 'uploading' && (
                        <span className="flex items-center gap-1.5 font-mono text-micro text-signal-accent">
                          <Loader2 className="h-3.5 w-3.5 animate-spin" /> Ingesting…
                        </span>
                      )}
                      {item.upload.status !== 'uploading' && (
                        <button
                          type="button"
                          aria-label={`Remove ${item.file.name}`}
                          onClick={() => setFiles((current) => current.filter((f) => f.key !== item.key))}
                          className="p-1 text-fg-subtle hover:text-signal-flag"
                        >
                          <Trash2 aria-hidden="true" className="h-3.5 w-3.5" />
                        </button>
                      )}
                    </div>
                  </div>

                  {item.upload.status === 'success' && (
                    <div className="mt-2 p-2.5 rounded border border-signal-ok/30 bg-signal-ok/[0.04]">
                      <div className="flex items-center gap-2 text-signal-ok font-mono text-xs font-semibold">
                        <CheckCircle2 className="h-4 w-4 shrink-0" />
                        <span>Ingestion & Extraction Succeeded</span>
                      </div>
                      <div className="mt-1.5 space-y-1 font-mono text-micro text-fg-muted">
                        <p>
                          <span className="text-fg-subtle">Evidence ID:</span>{' '}
                          <span className="text-fg font-bold">{item.upload.data.receipt.evidence_id}</span>
                        </p>
                        <p className="break-all">
                          <span className="text-fg-subtle">Server SHA-256:</span> {item.upload.data.receipt.sha256_hash}
                        </p>
                        <p>
                          <span className="text-fg-subtle">Extraction:</span>{' '}
                          <span className="text-signal-ok uppercase font-semibold">
                            {item.upload.data.extraction.status}
                          </span>{' '}
                          ({item.upload.data.extraction.entity_count} entities,{' '}
                          {item.upload.data.extraction.event_count} events,{' '}
                          {item.upload.data.extraction.relationship_count} relationships)
                        </p>
                      </div>

                      <div className="mt-2.5 flex flex-wrap gap-2 text-xs">
                        <Link
                          to={`/workspace/cases/${encodeURIComponent(caseId)}/evidence?evidence=${encodeURIComponent(item.upload.data.receipt.evidence_id)}`}
                          className="inline-flex items-center gap-1 text-signal-accent hover:underline font-mono text-micro"
                        >
                          View exhibit <ChevronRight className="h-3 w-3" />
                        </Link>
                        <Link
                          to={`/workspace/cases/${encodeURIComponent(caseId)}/graph`}
                          className="inline-flex items-center gap-1 text-signal-accent hover:underline font-mono text-micro"
                        >
                          Explore graph <ChevronRight className="h-3 w-3" />
                        </Link>
                        <Link
                          to={`/workspace/cases/${encodeURIComponent(caseId)}/reconstruction`}
                          className="inline-flex items-center gap-1 text-signal-accent hover:underline font-mono text-micro"
                        >
                          Timeline <ChevronRight className="h-3 w-3" />
                        </Link>
                      </div>
                    </div>
                  )}

                  {item.upload.status === 'error' && (
                    <div className="mt-2 p-2 rounded border border-signal-flag/30 bg-signal-flag/[0.05] flex items-center justify-between gap-2">
                      <div className="flex items-center gap-2 text-signal-flag font-mono text-xs">
                        <XCircle className="h-4 w-4 shrink-0" />
                        <span>{item.upload.message}</span>
                      </div>
                      <button
                        type="button"
                        onClick={() => uploadSingleFile(item)}
                        className="inline-flex items-center gap-1 px-2 py-0.5 font-mono text-micro uppercase bg-ink-800 text-fg hover:bg-ink-700 rounded border border-ink-line"
                      >
                        <RotateCw className="h-3 w-3" /> Retry
                      </button>
                    </div>
                  )}
                </li>
              ))}
            </ul>
          )}

          <div className="mt-3 flex flex-wrap items-center gap-3">
            <WsButton
              variant="primary"
              icon={Upload}
              disabled={isSyntheticCase || pendingCount === 0 || isUploadingAny}
              onClick={handleSubmitAll}
              aria-describedby={`${id}-submit-hint`}
            >
              Submit for ingestion
            </WsButton>
            <p id={`${id}-submit-hint`} className="text-xs text-fg-subtle">
              {isSyntheticCase
                ? 'Disabled in synthetic demonstration case. Select or create an active case to ingest evidence.'
                : 'Direct multipart stream to Rust backend (POST /api/v1/evidence/upload?auto_extract=true).'}
            </p>
          </div>
        </div>

        <div className="lg:col-span-5">
          <p className="mb-2 flex items-center gap-2 font-mono text-micro uppercase text-fg-subtle">
            <Info aria-hidden="true" className="h-3.5 w-3.5" /> Canonical evidence types
          </p>
          <ul className="space-y-1.5">
            {SUPPORTED.map((item) => (
              <li key={item} className="font-mono text-xs text-fg-muted">
                {item}
              </li>
            ))}
          </ul>
          <p className="mt-4 text-xs leading-relaxed text-fg-subtle">
            When ingestion is available, originals will be stored read-only and their SHA-256 recorded on arrival. A
            local preview digest is for comparison only and is not part of the chain of custody.
          </p>
        </div>
      </div>
    </section>
  );
};
