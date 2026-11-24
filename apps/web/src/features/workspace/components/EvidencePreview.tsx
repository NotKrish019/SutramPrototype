import React from 'react';
import { AudioLines, FileScan, FileText, HardDrive, MessageSquare, PhoneCall, Receipt, Video, type LucideIcon } from 'lucide-react';
import type { EvidenceType } from '../../../types';
import { CctvStill } from '../../landing/board/artwork/CctvStill';
import { ACCESS_LOG_ROWS, LogPrintout, NETWORK_LOG_ROWS, TransactionSlip } from '../../landing/board/artwork/Documents';
import { PhoneEvidence } from '../../landing/board/artwork/PhoneEvidence';
import type { WorkspaceEvidence } from '../types';

/**
 * Original, fully synthetic artwork for the CASE-026 demonstration artifacts (shared with the public
 * site). Keyed by `case_id/evidence_id` so it can never be attached to real or API-sourced evidence —
 * anything not listed here gets a neutral type glyph instead of an invented picture.
 */
const SYNTHETIC_ARTWORK: Record<string, () => React.ReactNode> = {
  'CASE-026/EV-01': () => <CctvStill annotate />,
  'CASE-026/EV-02': () => (
    <PaperFrame>
      <LogPrintout title="Access control log" file="door_controller_export.csv" tag="EV-02" rows={ACCESS_LOG_ROWS} />
    </PaperFrame>
  ),
  'CASE-026/EV-03': () => (
    <div className="relative h-full bg-[radial-gradient(circle_at_50%_40%,#1c1b19,#0b0a09_75%)]">
      {/* 16:11 frame: 86% of its height at a 9:17.5 phone ratio is 30.4% of its width. */}
      <div className="absolute left-1/2 top-[7%] aspect-[9/17.5] w-[30.4%] -translate-x-1/2">
        <PhoneEvidence compact className="h-full w-full" />
      </div>
    </div>
  ),
  'CASE-026/EV-04': () => (
    <PaperFrame>
      <TransactionSlip />
    </PaperFrame>
  ),
  'CASE-026/EV-05': () => (
    <PaperFrame>
      <LogPrintout title="AP association log" file="warehouse_ap_assoc.log" tag="EV-05" rows={NETWORK_LOG_ROWS} />
    </PaperFrame>
  ),
};

const PaperFrame: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <div className="flex h-full items-center justify-center bg-[radial-gradient(circle_at_50%_40%,#1c1b19,#0b0a09_75%)] px-[8%]">
    <div className="w-full -rotate-1 shadow-[0_14px_24px_-10px_rgba(0,0,0,.9)]">{children}</div>
  </div>
);

const TYPE_GLYPH: Record<EvidenceType, LucideIcon> = {
  CCTV: Video,
  CDR: PhoneCall,
  CHAT_EXPORT: MessageSquare,
  DOCUMENT_SCAN: FileScan,
  DISK_IMAGE: HardDrive,
  AUDIO_RECORDING: AudioLines,
  FINANCIAL_LEDGER: Receipt,
};

function extensionOf(filename: string): string {
  const dot = filename.lastIndexOf('.');
  return dot > 0 ? filename.slice(dot + 1).toUpperCase() : 'FILE';
}

export function hasSyntheticArtwork(evidence: Pick<WorkspaceEvidence, 'case_id' | 'evidence_id'>): boolean {
  return `${evidence.case_id}/${evidence.evidence_id}` in SYNTHETIC_ARTWORK;
}

/**
 * Visual stand-in for an artifact. Synthetic demonstration artifacts show their (synthetic) artwork;
 * everything else shows a type glyph and file extension — never a fabricated preview.
 */
export const EvidencePreview: React.FC<{
  evidence: WorkspaceEvidence;
  /** Only synthetic repositories may show demonstration artwork. */
  synthetic: boolean;
  className?: string;
}> = ({ evidence, synthetic, className = '' }) => {
  const artwork = synthetic ? SYNTHETIC_ARTWORK[`${evidence.case_id}/${evidence.evidence_id}`] : undefined;
  // The API can report source types the UI has no glyph for (e.g. TABULAR_CSV); fall back instead of crashing.
  const Glyph: LucideIcon = (evidence.source_type && TYPE_GLYPH[evidence.source_type]) || FileText;
  return (
    <div
      className={`relative aspect-[16/11] overflow-hidden bg-ink-950 ${className}`}
      role="img"
      aria-label={artwork ? `Synthetic preview of ${evidence.evidence_id}` : `${evidence.source_type_label} file, ${extensionOf(evidence.original_filename)}`}
    >
      {artwork ? (
        <div aria-hidden="true" className="h-full w-full">
          {artwork()}
        </div>
      ) : (
        <div aria-hidden="true" className="flex h-full flex-col items-center justify-center gap-2 bg-[radial-gradient(circle_at_50%_40%,#1a1815,#0b0a09_75%)] text-fg-subtle">
          <Glyph strokeWidth={1.2} className="h-8 w-8" />
          <span className="font-mono text-micro uppercase">{extensionOf(evidence.original_filename)}</span>
        </div>
      )}
      {artwork && (
        <span className="pointer-events-none absolute bottom-1.5 right-1.5 bg-ink-950/80 px-1.5 py-0.5 font-mono text-[0.5625rem] uppercase tracking-[0.12em] text-paper">
          Synthetic
        </span>
      )}
    </div>
  );
};
