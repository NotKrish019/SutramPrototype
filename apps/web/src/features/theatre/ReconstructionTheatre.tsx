import React, { useEffect, useMemo, useRef } from 'react';
import { ArrowRight, ChevronLeft, ChevronRight, Pause, Play, RotateCcw, X } from 'lucide-react';
import { Link } from '../../router/Link';
import { caseHref } from '../../router/router';
import { CliCommand } from '../cli-bridge/CliCommand';
import { useIsSynthetic } from '../workspace/data/WorkspaceDataContext';
import type { CaseDetail } from '../workspace/types';
import { CHAPTERS, CHAPTER_STARTS, TOTAL_DURATION, locate, type ChapterKey } from './chapters';
import { buildTheatreModel, formatWall, type TheatreModel } from './model';
import { BriefScene, ClockScene, ConflictScene, LinkScene, PlaceScene, SealScene, STAGE_H, STAGE_W, StageDefs, clockCursor, type SceneProps } from './scenes';
import { prefersReducedMotion, usePlayhead } from './usePlayhead';

const SCENES: Record<ChapterKey, React.FC<SceneProps>> = {
  seal: SealScene,
  clock: ClockScene,
  place: PlaceScene,
  link: LinkScene,
  conflict: ConflictScene,
  brief: BriefScene,
};

const SPEEDS = [0.5, 1, 2];

function formatClock(ms: number): string {
  const s = Math.floor(ms / 1000);
  return `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`;
}

/** The case-clock readout: what time it is "inside the evidence" at this point of the playback. */
function caseClock(model: TheatreModel, key: ChapterKey, p: number): { value: string; label: string } | null {
  if (model.events.length === 0) return null;
  if (key === 'clock') {
    const cursor = clockCursor(model, p);
    if (cursor !== null) return { value: formatWall(cursor), label: 'IST · scanning the normalised timeline' };
    return { value: formatWall(model.events[0]!.wall), label: 'IST · first recorded event' };
  }
  if (key === 'conflict' && model.conflict) {
    const e = model.events[model.conflict.eventIndices[0] ?? -1];
    if (e) return { value: formatWall(e.wall), label: 'IST · moment in dispute' };
  }
  const reached = Math.min(Math.floor(p * model.events.length), model.events.length - 1);
  return { value: formatWall(model.events[Math.max(reached, 0)]!.wall), label: 'IST · case clock' };
}

export const ReconstructionTheatre: React.FC<{ detail: CaseDetail }> = ({ detail }) => {
  const model = useMemo(() => buildTheatreModel(detail), [detail]);
  const synthetic = useIsSynthetic(detail.case);
  const reduced = useMemo(prefersReducedMotion, []);
  // Reduced motion: start paused with the first chapter fully revealed; chapters change on request.
  const playhead = usePlayhead(TOTAL_DURATION, { autoplay: !reduced, initial: reduced ? CHAPTERS[0]!.duration - 1 : 0 });
  const { index, progress } = locate(playhead.time);
  const chapter = CHAPTERS[index]!;
  const Scene = SCENES[chapter.key];
  const seconds = playhead.time / 1000;
  const clock = caseClock(model, chapter.key, progress);
  const finished = playhead.time >= TOTAL_DURATION;
  const rootRef = useRef<HTMLDivElement>(null);
  const caseId = detail.case.case_id;

  const goTo = (i: number): void => {
    const target = Math.min(Math.max(i, 0), CHAPTERS.length - 1);
    playhead.seek(reduced ? CHAPTER_STARTS[target]! + CHAPTERS[target]!.duration - 1 : CHAPTER_STARTS[target]!);
  };

  useEffect(() => {
    const onKey = (event: KeyboardEvent): void => {
      const target = event.target instanceof Element ? event.target : null;
      if (target && (target.closest('input, textarea, select, [contenteditable="true"]') || event.metaKey || event.ctrlKey || event.altKey)) return;
      if (event.key === ' ' || event.key === 'k') {
        if (target?.closest('button, a')) return;
        event.preventDefault();
        playhead.toggle();
      } else if (event.key === 'ArrowRight' || event.key === 'l') {
        event.preventDefault();
        goTo(index + 1);
      } else if (event.key === 'ArrowLeft' || event.key === 'j') {
        event.preventDefault();
        goTo(progress < 0.08 ? index - 1 : index);
      } else if (/^[1-6]$/.test(event.key)) {
        goTo(Number(event.key) - 1);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  const onScrub = (event: React.PointerEvent<HTMLDivElement>): void => {
    const rect = event.currentTarget.getBoundingClientRect();
    const f = Math.min(Math.max((event.clientX - rect.left) / rect.width, 0), 1);
    playhead.seek(f * TOTAL_DURATION);
  };

  const sceneOpacity = Math.min(progress / 0.05, 1);
  const [details, setDetails] = React.useState(false);

  return (
    <div ref={rootRef} className="theatre-root relative flex h-screen min-h-[36rem] flex-col overflow-hidden" aria-label="Reconstruction theatre">
      <div className="theatre-stage absolute inset-0" aria-hidden="true" />

      {/* Top bar: back, title, chapter progress, case clock */}
      <div className="relative z-10 flex items-center gap-4 px-5 pt-4 sm:px-8">
        <Link
          to={caseHref(caseId)}
          aria-label="Close theatre"
          className="inline-flex h-10 w-10 shrink-0 items-center justify-center rounded-full border border-white/10 bg-white/[0.04] text-fg-muted backdrop-blur hover:text-fg-bright focus:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
        >
          <X aria-hidden="true" className="h-4 w-4" />
        </Link>
        <div className="min-w-0">
          <p className="flex items-center gap-2 text-xs text-fg-subtle">
            <span className="theatre-rec h-1.5 w-1.5 rounded-full bg-crimson" aria-hidden="true" />
            Reconstruction{synthetic ? ' · synthetic case' : ''}
          </p>
          <h1 className="truncate text-base font-semibold text-fg-bright">{model.title}</h1>
        </div>
        <ol className="mx-auto hidden min-w-0 flex-1 items-start gap-2 px-6 md:flex lg:max-w-3xl" aria-label="Chapters">
          {CHAPTERS.map((c, i) => {
            const fill = i < index ? 1 : i === index ? progress : 0;
            return (
              <li key={c.key} className="min-w-0 flex-1">
                <button
                  type="button"
                  onClick={() => goTo(i)}
                  aria-current={i === index ? 'step' : undefined}
                  className="group block w-full text-left focus:outline-none"
                >
                  <span className="block h-[3px] overflow-hidden rounded-full bg-white/10">
                    <span className="block h-full rounded-full bg-crimson" style={{ width: `${fill * 100}%` }} />
                  </span>
                  <span className={`mt-2 block truncate text-xs transition-colors ${i === index ? 'text-fg-bright' : 'text-fg-subtle group-hover:text-fg-muted'}`}>
                    {c.short}
                  </span>
                </button>
              </li>
            );
          })}
        </ol>
        {clock && (
          <div className="ml-auto hidden text-right sm:block md:ml-0">
            <p className="font-mono text-2xl font-medium tabular-nums leading-none text-fg-bright">{clock.value}</p>
            <p className="mt-1 text-[0.7rem] text-fg-subtle">{clock.label}</p>
          </div>
        )}
      </div>

      {/* Stage */}
      <div className="relative min-h-0 flex-1">
        <svg
          role="img"
          aria-label={`${chapter.title}. ${chapter.what(model)}`}
          viewBox={`0 0 ${STAGE_W} ${STAGE_H}`}
          preserveAspectRatio="xMidYMid meet"
          className="absolute inset-0 h-full w-full"
        >
          <StageDefs />
          <g key={chapter.key} opacity={sceneOpacity}>
            <Scene model={model} p={progress} t={seconds} />
          </g>
        </svg>
        <div className="theatre-vignette pointer-events-none absolute inset-0" aria-hidden="true" />
      </div>

      {/* Caption + transport */}
      <div className="theatre-caption relative z-10 px-5 pb-5 pt-2 sm:px-8">
        <div className="mx-auto flex max-w-6xl flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
          <div key={chapter.key} className="theatre-narrate min-w-0 max-w-3xl" aria-live="polite">
            <p className="text-xs font-medium text-crimson">
              Chapter {chapter.number}
            </p>
            <h2 className="mt-1 text-[clamp(1.4rem,2.4vw,2rem)] font-semibold leading-tight tracking-[-0.02em] text-fg-bright">{chapter.title}</h2>
            <p className="mt-2 text-[0.95rem] leading-relaxed text-fg-muted">{chapter.what(model)}</p>
          </div>
          <div className="flex shrink-0 flex-wrap items-center gap-2 lg:justify-end">
            <CliCommand command={synthetic ? 'yukti demo' : chapter.cli(caseId)} compact className="max-w-[22rem] !rounded-lg" />
            <button
              type="button"
              onClick={() => setDetails((v) => !v)}
              aria-expanded={details}
              className="inline-flex h-9 items-center rounded-lg border border-white/10 bg-white/[0.04] px-3 text-xs font-medium text-fg-muted hover:text-fg-bright focus:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
            >
              Why it matters
            </button>
            <Link
              to={caseHref(caseId, finished ? 'reports' : chapter.section)}
              className={`inline-flex h-9 items-center gap-1.5 rounded-lg px-3 text-xs font-medium focus:outline-none focus-visible:ring-2 focus-visible:ring-white/40 ${
                finished ? 'bg-crimson text-white hover:bg-crimson-deep' : 'border border-white/10 bg-white/[0.04] text-fg-muted hover:text-fg-bright'
              }`}
            >
              {finished ? 'Judicial report' : 'Open this step'} <ArrowRight aria-hidden="true" className="h-3.5 w-3.5" />
            </Link>
          </div>
        </div>
        {details && (
          <div className="theatre-narrate mx-auto mt-3 max-w-6xl rounded-xl border border-white/10 bg-white/[0.03] px-4 py-3 text-sm leading-relaxed text-fg-muted backdrop-blur">
            {chapter.why}
          </div>
        )}

        <div className="mx-auto mt-4 flex max-w-6xl items-center gap-3">
          <button type="button" onClick={() => goTo(index - 1)} aria-label="Previous chapter" className="theatre-btn">
            <ChevronLeft aria-hidden="true" className="h-4 w-4" />
          </button>
          <button
            type="button"
            onClick={finished ? () => { playhead.seek(0); playhead.play(); } : playhead.toggle}
            aria-label={finished ? 'Replay from the start' : playhead.playing ? 'Pause' : 'Play'}
            className="theatre-btn theatre-btn--primary"
          >
            {finished ? <RotateCcw aria-hidden="true" className="h-4 w-4" /> : playhead.playing ? <Pause aria-hidden="true" className="h-4 w-4" /> : <Play aria-hidden="true" className="h-4 w-4 translate-x-px" />}
          </button>
          <button type="button" onClick={() => goTo(index + 1)} aria-label="Next chapter" className="theatre-btn">
            <ChevronRight aria-hidden="true" className="h-4 w-4" />
          </button>
          <div
            role="slider"
            tabIndex={0}
            aria-label="Playback position"
            aria-valuemin={0}
            aria-valuemax={Math.round(TOTAL_DURATION / 1000)}
            aria-valuenow={Math.round(playhead.time / 1000)}
            aria-valuetext={`${chapter.title}, ${formatClock(playhead.time)}`}
            onPointerDown={(event) => {
              event.currentTarget.setPointerCapture(event.pointerId);
              onScrub(event);
            }}
            onPointerMove={(event) => {
              if (event.currentTarget.hasPointerCapture(event.pointerId)) onScrub(event);
            }}
            onKeyDown={(event) => {
              if (event.key === 'ArrowRight') { event.preventDefault(); event.stopPropagation(); playhead.seek(playhead.time + 2000); }
              if (event.key === 'ArrowLeft') { event.preventDefault(); event.stopPropagation(); playhead.seek(playhead.time - 2000); }
            }}
            className="group relative mx-2 h-8 min-w-[8rem] flex-1 cursor-pointer focus:outline-none focus-visible:ring-1 focus-visible:ring-white/40"
          >
            <div className="absolute inset-x-0 top-1/2 h-1 -translate-y-1/2 overflow-hidden rounded-full bg-white/10">
              <div className="h-full rounded-full bg-gradient-to-r from-crimson-deep to-crimson" style={{ width: `${(playhead.time / TOTAL_DURATION) * 100}%` }} />
            </div>
            <div
              aria-hidden="true"
              className="absolute top-1/2 h-3.5 w-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-white shadow-[0_0_0_4px_rgba(229,36,59,0.35)] transition-transform group-hover:scale-110"
              style={{ left: `${(playhead.time / TOTAL_DURATION) * 100}%` }}
            />
          </div>
          <span className="whitespace-nowrap text-right font-mono text-xs tabular-nums text-fg-subtle">
            {formatClock(playhead.time)} / {formatClock(TOTAL_DURATION)}
          </span>
          <div className="hidden items-center gap-1 sm:flex" role="group" aria-label="Playback speed">
            {SPEEDS.map((s) => (
              <button
                key={s}
                type="button"
                onClick={() => playhead.setSpeed(s)}
                aria-pressed={playhead.speed === s}
                className={`h-7 rounded-md px-2 font-mono text-[0.7rem] tabular-nums focus:outline-none focus-visible:ring-1 focus-visible:ring-white/40 ${
                  playhead.speed === s ? 'bg-white/10 text-fg-bright' : 'text-fg-subtle hover:text-fg'
                }`}
              >
                {s}×
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};
