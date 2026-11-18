import React, { useCallback, useId, useRef, useState } from 'react';
import { FileUp, Info, Trash2, Upload } from 'lucide-react';
import { WsButton } from '../components/ui';
import { formatBytes } from '../format';

/** Files above this size are listed but not hashed in the browser (memory safety). */
export const LOCAL_HASH_LIMIT_BYTES = 256 * 1024 * 1024;

type HashState = { status: 'hashing' } | { status: 'done'; sha256: string } | { status: 'skipped'; reason: string } | { status: 'error'; reason: string };

interface StagedFile {
  key: string;
  file: File;
  hash: HashState;
}

const SUPPORTED = [
  'CCTV · video exports',
  'CDR · call detail records',
  'CHAT_EXPORT · messaging exports',
  'DOCUMENT_SCAN · PDF / image scans',
  'DISK_IMAGE · E01, RAW, DD',
  'AUDIO_RECORDING · audio files',
  'FINANCIAL_LEDGER · statements, CSV',
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

/**
 * Upload-ready staging UI. Files are read locally to compute a SHA-256 preview; nothing is
 * transmitted because the backend has no ingestion endpoint yet.
 */
export const EvidenceUploadPanel: React.FC<{ id: string }> = ({ id }) => {
  const [files, setFiles] = useState<StagedFile[]>([]);
  const [dragging, setDragging] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const hintId = useId();

  const addFiles = useCallback((list: FileList | null) => {
    if (!list || list.length === 0) return;
    const staged = Array.from(list).map((file, index) => ({
      key: `${file.name}-${file.size}-${file.lastModified}-${Date.now()}-${index}`,
      file,
      hash:
        file.size > LOCAL_HASH_LIMIT_BYTES
          ? ({ status: 'skipped', reason: `Larger than ${formatBytes(LOCAL_HASH_LIMIT_BYTES)} — hash at ingestion` } as const)
          : ({ status: 'hashing' } as const),
    }));
    setFiles((current) => [...current, ...staged]);
    for (const item of staged) {
      if (item.hash.status !== 'hashing') continue;
      sha256(item.file)
        .then((digest) => setFiles((current) => current.map((f) => (f.key === item.key ? { ...f, hash: { status: 'done', sha256: digest } } : f))))
        .catch((error: unknown) =>
          setFiles((current) =>
            current.map((f) =>
              f.key === item.key ? { ...f, hash: { status: 'error', reason: error instanceof Error ? error.message : 'Hashing failed' } } : f,
            ),
          ),
        );
    }
  }, []);

  return (
    <section id={id} aria-labelledby={`${id}-heading`} className="border border-ink-line bg-ink-900">
      <div className="flex items-center justify-between gap-3 border-b border-ink-line px-4 py-2.5">
        <h2 id={`${id}-heading`} className="font-mono text-micro uppercase text-fg-muted">Stage evidence</h2>
        <span className="font-mono text-micro uppercase text-signal-amber">Ingestion API not available</span>
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
              dragging ? 'border-signal-amber bg-signal-amber/[0.06]' : 'border-ink-strong bg-ink-950'
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
              Files stay in this browser. A SHA-256 digest is computed locally as a preview; nothing is uploaded.
            </p>
          </div>

          {files.length > 0 && (
            <ul className="mt-3 divide-y divide-ink-line border border-ink-line" aria-label="Staged files">
              {files.map((item) => (
                <li key={item.key} className="flex items-start gap-3 px-3 py-2.5">
                  <div className="min-w-0 flex-1">
                    <p className="truncate font-mono text-xs text-fg">{item.file.name}</p>
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
                      {item.hash.status === 'error' && <span className="text-signal-flag">Could not hash: {item.hash.reason}</span>}
                    </p>
                  </div>
                  <button
                    type="button"
                    aria-label={`Remove ${item.file.name}`}
                    onClick={() => setFiles((current) => current.filter((f) => f.key !== item.key))}
                    className="p-1 text-fg-subtle hover:text-signal-flag"
                  >
                    <Trash2 aria-hidden="true" className="h-3.5 w-3.5" />
                  </button>
                </li>
              ))}
            </ul>
          )}

          <div className="mt-3 flex flex-wrap items-center gap-3">
            <WsButton variant="primary" icon={Upload} disabled aria-describedby={`${id}-unavailable`}>
              Submit for ingestion
            </WsButton>
            <p id={`${id}-unavailable`} className="text-xs text-fg-subtle">
              Disabled: the backend ingestion endpoint is planned (backend/app/modules/ingestion).
            </p>
          </div>
        </div>

        <div className="lg:col-span-5">
          <p className="mb-2 flex items-center gap-2 font-mono text-micro uppercase text-fg-subtle">
            <Info aria-hidden="true" className="h-3.5 w-3.5" /> Canonical evidence types
          </p>
          <ul className="space-y-1.5">
            {SUPPORTED.map((item) => (
              <li key={item} className="font-mono text-xs text-fg-muted">{item}</li>
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
