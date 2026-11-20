import React, { useMemo, useState } from 'react';
import { ArrowRight, Clapperboard, Film, Fingerprint, ScanLine, ShieldAlert } from 'lucide-react';
import { useWorkspaceRepository } from '../data/WorkspaceDataContext';
import { EmptyState, ReviewCategoryBadge } from '../components/ui';
import { formatBytes, formatDateTime, formatEventTime, formatUtc, shortHash } from '../format';
import type { CaseDetail, WorkspaceEvent, WorkspaceEvidence } from '../types';

/** Exhibits treated as video: declared CCTV/video type, or a video container file. */
function isVideo(evidence: WorkspaceEvidence): boolean {
  const type = (evidence.source_type ?? evidence.source_type_label ?? '').toUpperCase();
  return type.includes('CCTV') || type.includes('VIDEO') || /\.(mp4|mov|mkv|avi)$/i.test(evidence.original_filename);
}

const StageLabel: React.FC<{ step: string; children: React.ReactNode }> = ({ step, children }) => (
  <p className="mb-3 flex items-center gap-2 font-mono text-micro uppercase text-fg-subtle">
    <span className="flex h-4 w-4 items-center justify-center rounded-full border border-ink-strong text-[0.55rem] tabular-nums">{step}</span>
    {children}
  </p>
);

const Connector: React.FC = () => (
  <div aria-hidden="true" className="hidden items-center justify-center xl:flex">
    <svg width="40" height="12" viewBox="0 0 40 12" className="text-fg-subtle">
      <path d="M0 6 H34" stroke="currentColor" strokeWidth="1" pathLength={1} className="ws-draw" />
      <path d="M30 2 L36 6 L30 10" fill="none" stroke="currentColor" strokeWidth="1" />
    </svg>
  </div>
);

const FrameCard: React.FC<{ event: WorkspaceEvent; selected: boolean; onSelect: () => void }> = ({ event, selected, onSelect }) => {
  const frame = event.frame!;
  return (
    <button
      type="button"
      onClick={onSelect}
      aria-pressed={selected}
      className={`group w-full border p-3 text-left transition-colors duration-fast focus:outline-none focus-visible:ring-1 focus-visible:ring-signal-accent ${
        selected ? 'border-signal-live/70 bg-signal-live/[0.06]' : 'border-ink-line bg-ink-900 hover:border-ink-strong'
      }`}
    >
      <div className="mb-2 flex aspect-video items-center justify-center border border-ink-line bg-ink-950">
        <ScanLine aria-hidden="true" className="h-5 w-5 text-fg-subtle" />
        <span className="sr-only">Keyframe image stored at {frame.relative_path ?? 'derived storage'}</span>
      </div>
      <p className="flex items-baseline justify-between gap-2 font-mono text-micro uppercase text-fg-subtle">
        <span>Frame {frame.frame_index ?? '—'}</span>
        <span className="tabular-nums text-fg">PTS {frame.pts_seconds.toFixed(3)} s</span>
      </p>
      <p className="mt-1 truncate font-mono text-[0.7rem] tabular-nums text-fg-muted" title={frame.sha256}>
        {shortHash(frame.sha256)}
      </p>
    </button>
  );
};

const VideoExhibit: React.FC<{ evidence: WorkspaceEvidence; detail: CaseDetail; live: boolean }> = ({ evidence, detail, live }) => {
  const events = useMemo(
    () => detail.events.filter((event) => event.source_evidence_id === evidence.evidence_id),
    [detail.events, evidence.evidence_id],
  );
  const frameEvents = events.filter((event) => event.frame);
  const excluded = (detail.excluded_events ?? []).filter((item) => item.source_evidence_id === evidence.evidence_id);
  const reviews = detail.review_items.filter((item) => item.records.some((record) => record.evidence_id === evidence.evidence_id));
  const [selectedId, setSelectedId] = useState<string | null>(frameEvents[0]?.event_id ?? null);
  const selected = frameEvents.find((event) => event.event_id === selectedId) ?? null;
  const metadata = Object.entries(evidence.metadata);

  return (
    <article className="border border-ink-line bg-ink-900/70 shadow-lift ws-rise" aria-labelledby={`video-${evidence.evidence_id}`}>
      <header className="flex flex-wrap items-center justify-between gap-3 border-b border-ink-line px-5 py-4">
        <div className="flex min-w-0 items-center gap-3">
          <span className="flex h-9 w-9 items-center justify-center border border-ink-strong text-fg">
            <Film aria-hidden="true" className="h-4 w-4" />
          </span>
          <div className="min-w-0">
            <h2 id={`video-${evidence.evidence_id}`} className="truncate font-serif text-xl text-fg-bright">
              {evidence.source}
            </h2>
            <p className="truncate font-mono text-micro uppercase text-fg-subtle">
              {evidence.evidence_id} · {evidence.original_filename}
            </p>
          </div>
        </div>
        <span className="font-mono text-micro uppercase text-fg-muted">
          {frameEvents.length} frame{frameEvents.length === 1 ? '' : 's'} · {events.length} event{events.length === 1 ? '' : 's'}
        </span>
      </header>

      <div className="grid gap-5 p-5 xl:grid-cols-[minmax(0,1fr)_40px_minmax(0,1.4fr)_40px_minmax(0,1fr)]">
        {/* 1 · Source video */}
        <section aria-label="Source video">
          <StageLabel step="1">Source video</StageLabel>
          <dl className="space-y-2.5 text-sm">
            <div>
              <dt className="font-mono text-micro uppercase text-fg-subtle">SHA-256 at intake</dt>
              <dd className="mt-0.5 flex items-center gap-1.5 break-all font-mono text-xs tabular-nums text-fg" title={evidence.sha256_hash}>
                <Fingerprint aria-hidden="true" className="h-3.5 w-3.5 shrink-0 text-signal-ok" />
                {shortHash(evidence.sha256_hash)}
              </dd>
            </div>
            <div>
              <dt className="font-mono text-micro uppercase text-fg-subtle">Size · ingested</dt>
              <dd className="mt-0.5 text-fg-muted">
                {formatBytes(evidence.file_size_bytes)} · {formatDateTime(evidence.ingested_at)}
              </dd>
            </div>
            {metadata.slice(0, 4).map(([key, value]) => (
              <div key={key}>
                <dt className="font-mono text-micro uppercase text-fg-subtle">{key.replace(/_/g, ' ')}</dt>
                <dd className="mt-0.5 break-words text-fg-muted">{value}</dd>
              </div>
            ))}
            <div>
              <dt className="font-mono text-micro uppercase text-fg-subtle">Extracted at intake</dt>
              <dd className="mt-0.5 text-fg-muted">{evidence.extracted_summary}</dd>
            </div>
          </dl>
        </section>

        <Connector />

        {/* 2 · Keyframes */}
        <section aria-label="Extracted keyframes">
          <StageLabel step="2">Keyframes · SHA-256 per frame</StageLabel>
          {frameEvents.length > 0 ? (
            <>
              <ul className="grid grid-cols-2 gap-2 sm:grid-cols-3">
                {frameEvents.map((event) => (
                  <li key={event.event_id}>
                    <FrameCard event={event} selected={event.event_id === selectedId} onSelect={() => setSelectedId(event.event_id)} />
                  </li>
                ))}
              </ul>
              {selected?.frame && (
                <dl key={selected.event_id} className="mt-3 grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1.5 border-t border-ink-line pt-3 text-xs ws-inspector-enter">
                  <dt className="font-mono uppercase text-fg-subtle">Artifact</dt>
                  <dd className="break-all font-mono text-fg">{selected.frame.artifact_reference ?? 'Not reported'}</dd>
                  <dt className="font-mono uppercase text-fg-subtle">Frame hash</dt>
                  <dd className="break-all font-mono tabular-nums text-fg">{selected.frame.sha256}</dd>
                  <dt className="font-mono uppercase text-fg-subtle">Bound to</dt>
                  <dd className="break-all font-mono tabular-nums text-fg-muted">
                    {selected.frame.source_video_sha256 ? shortHash(selected.frame.source_video_sha256) : 'Not reported'}
                    {selected.frame.source_video_sha256 && selected.frame.source_video_sha256 === evidence.sha256_hash && (
                      <span className="ml-2 text-signal-ok">matches intake hash</span>
                    )}
                  </dd>
                  <dt className="font-mono uppercase text-fg-subtle">UTC anchor</dt>
                  <dd className="text-fg-muted">{selected.frame.anchor_basis ?? 'Not anchored (relative PTS only)'}</dd>
                </dl>
              )}
            </>
          ) : (
            <div className="border border-dashed border-ink-strong px-4 py-6">
              <p className="flex items-center gap-2 text-sm text-fg">
                <ScanLine aria-hidden="true" className="h-4 w-4 text-fg-subtle" />
                No frame records for this exhibit
              </p>
              <p className="mt-2 text-sm leading-relaxed text-fg-muted">
                {live
                  ? 'The backend reported no extracted keyframes for this video. Run the case reconstruction to extract them.'
                  : 'Keyframes are extracted by the YUKTI backend (I-frames, real presentation timestamps, a SHA-256 per frame) when a case is reconstructed live. Synthetic demonstration data contains none, so none are drawn.'}
              </p>
            </div>
          )}
          {excluded.length > 0 && (
            <div className="mt-4 border-l-2 border-signal-warn/70 pl-3">
              <p className="font-mono text-micro uppercase text-signal-warn">Excluded from the timeline · {excluded.length}</p>
              <ul className="mt-1.5 space-y-1 text-xs text-fg-muted">
                {excluded.map((item) => (
                  <li key={item.event_id}>
                    <span className="font-mono text-fg">{item.event_id}</span> · {item.reason}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </section>

        <Connector />

        {/* 3 · Timeline events */}
        <section aria-label="Video-derived timeline events">
          <StageLabel step="3">Timeline events</StageLabel>
          {events.length === 0 ? (
            <p className="text-sm text-fg-muted">No timeline events are derived from this exhibit.</p>
          ) : (
            <ol className="space-y-2">
              {events.map((event) => (
                <li key={event.event_id} className="border border-ink-line bg-ink-950/60 px-3 py-2.5">
                  <p className="text-sm text-fg">{event.title}</p>
                  <p className="mt-1 font-mono text-xs tabular-nums text-fg">
                    {formatEventTime(event.timestamp, event.timestamp_precision)}
                    <span className="ml-2 text-fg-subtle">{formatUtc(event.timestamp)}</span>
                  </p>
                  <p className="mt-1 font-mono text-[0.7rem] text-fg-subtle">
                    Source clock: <span className="text-fg-muted">{event.source_recorded_time}</span>
                  </p>
                  {(event.assumptions?.length ?? 0) > 0 && (
                    <p className="mt-1.5 text-[0.7rem] text-signal-warn">
                      {event.assumptions!.length} assumed value{event.assumptions!.length === 1 ? '' : 's'}
                    </p>
                  )}
                </li>
              ))}
            </ol>
          )}
        </section>
      </div>

      {reviews.length > 0 && (
        <footer className="border-t border-ink-line px-5 py-4">
          <p className="mb-2 flex items-center gap-2 font-mono text-micro uppercase text-fg-subtle">
            <ShieldAlert aria-hidden="true" className="h-3.5 w-3.5" /> Open questions about this footage
          </p>
          <ul className="space-y-2">
            {reviews.map((item) => (
              <li key={item.id} className="flex flex-wrap items-start gap-2 text-sm">
                <ReviewCategoryBadge category={item.category} />
                <span className="text-fg">{item.title}</span>
                {item.limitations[0] && <span className="basis-full text-xs text-fg-muted">{item.limitations[0]}</span>}
              </li>
            ))}
          </ul>
        </footer>
      )}
    </article>
  );
};

/** Frame-level video forensics: source video → keyframes → timeline events, with provenance. */
export const VideoIntelligence: React.FC<{ detail: CaseDetail }> = ({ detail }) => {
  const repository = useWorkspaceRepository();
  const videos = detail.evidence.filter(isVideo);

  if (videos.length === 0) {
    return (
      <EmptyState icon={Clapperboard} title="No video evidence in this case">
        Video exhibits (CCTV or other recordings) appear here once ingested, with their keyframes and the timeline events derived
        from them.
      </EmptyState>
    );
  }

  return (
    <div className="space-y-6 px-4 py-6 sm:px-6 lg:px-8">
      <p className="flex max-w-3xl flex-wrap items-center gap-x-2 gap-y-1 text-sm text-fg-muted ws-rise">
        <span className="text-fg">Source video</span>
        <ArrowRight aria-hidden="true" className="h-3.5 w-3.5 text-fg-subtle" />
        <span className="text-fg">keyframes bound to its hash</span>
        <ArrowRight aria-hidden="true" className="h-3.5 w-3.5 text-fg-subtle" />
        <span className="text-fg">events on the corrected timeline.</span>
        <span>Frames are never interpreted here; they are evidence references for investigators.</span>
      </p>
      {videos.map((evidence) => (
        <VideoExhibit key={evidence.evidence_id} evidence={evidence} detail={detail} live={repository.source === 'api'} />
      ))}
    </div>
  );
};
