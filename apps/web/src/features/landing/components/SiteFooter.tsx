import React from 'react';
import { ArrowUpRight } from 'lucide-react';
import { useBackendHealth, type ConnectionStatus } from '../../../hooks/useBackendHealth';
import { Link } from '../../../router/Link';
import { ROUTES } from '../../../router/router';
import { PROJECT_REPOSITORY_URL, SECTION_IDS } from '../data/siteContent';
import { CtaLink, YuktiWordmark } from '../../../components/ui/primitives';
import { Fingerprint } from '../board/artwork/Fingerprint';
import { Pushpin } from '../board/materials';
import { ThreadCanvas, type ThreadLink } from '../board/ThreadCanvas';
import { useInView } from '../hooks/useInView';

const FOOTER_STORY_LINKS = [
  { label: 'Evidence sources', href: `#${SECTION_IDS.capabilities}` },
  { label: 'Correlation', href: `#${SECTION_IDS.correlation}` },
  { label: 'Reconstruction', href: `#${SECTION_IDS.reconstruction}` },
  { label: 'Timeline', href: `#${SECTION_IDS.timeline}` },
  { label: 'Contradictions', href: `#${SECTION_IDS.contradictions}` },
  { label: 'Provenance', href: `#${SECTION_IDS.provenance}` },
];

const STATUS_COPY: Record<ConnectionStatus, { label: string; dot: string }> = {
  checking: { label: 'Checking backend API', dot: 'bg-paper animate-pulse' },
  connected: { label: 'Backend API connected', dot: 'bg-signal-ok' },
  offline: { label: 'Backend API offline', dot: 'bg-crimson' },
};

const FOOTER_LINK =
  'text-sm text-fg-muted transition-colors duration-fast hover:text-fg-bright focus:outline-none focus-visible:text-crimson';

/** Live status from the existing health monitor — a real signal, not decoration. */
const ApiStatus: React.FC = () => {
  const { status } = useBackendHealth();
  const copy = STATUS_COPY[status];
  return (
    <Link to={ROUTES.workspace} className="inline-flex items-center gap-2 font-mono text-micro uppercase text-fg-muted hover:text-fg">
      <span aria-hidden="true" className={`h-1.5 w-1.5 rounded-full ${copy.dot}`} />
      <span role="status">{copy.label}</span>
    </Link>
  );
};

/**
 * Threads converge from the edges of the board onto the pin above the CTA. Edge anchors
 * sit below the headline so no string ever crosses the type; positions are measured live.
 */
const EDGE_ANCHORS: { id: string; style: React.CSSProperties }[] = [
  { id: 'cta-l1', style: { left: 0, top: '60%' } },
  { id: 'cta-l2', style: { left: 0, top: '84%' } },
  { id: 'cta-b1', style: { left: '22%', top: '100%' } },
  { id: 'cta-r1', style: { right: 0, top: '56%' } },
  { id: 'cta-r2', style: { right: 0, top: '80%' } },
  { id: 'cta-b2', style: { left: '76%', top: '100%' } },
];

const CTA_LINKS: ThreadLink[] = EDGE_ANCHORS.map((anchor, i) => ({ id: anchor.id, from: anchor.id, to: 'cta-pin', delay: i * 140 }));

export const FinalCTASection: React.FC = () => {
  const [ref, visible] = useInView<HTMLDivElement>({ rootMargin: '0px 0px -20% 0px' });
  return (
    <section aria-labelledby="cta-heading" className="board-surface relative overflow-hidden border-b border-ink-line">
      <div aria-hidden="true" className="board-vignette" />
      <div ref={ref} className="relative">
        {EDGE_ANCHORS.map((anchor) => (
          <span key={anchor.id} aria-hidden="true" data-pin-id={anchor.id} className="absolute h-0 w-0" style={anchor.style} />
        ))}
        <ThreadCanvas links={CTA_LINKS} drawn={visible} sagAmount={0.08} pulse={false} />
        <div aria-hidden="true" className="pointer-events-none absolute left-1/2 top-[52%] h-[30rem] w-[25rem] -translate-x-1/2 -translate-y-1/2 text-paper opacity-[0.12]">
          <Fingerprint variant="line" />
        </div>
        <div className="relative mx-auto flex max-w-wide flex-col items-center px-gutter py-36 text-center sm:py-44">
          <p className="mb-8 font-mono text-[0.6875rem] uppercase tracking-[0.2em] text-fg-subtle">
            <span className="text-crimson">09</span> · Every piece of evidence. Every connection. One investigation.
          </p>
          <p aria-hidden="true" className="serif-tight font-serif text-[clamp(5rem,16vw,13rem)] font-semibold leading-[0.85] tracking-[0.02em] text-fg-bright [text-shadow:0_30px_80px_rgba(0,0,0,.9)]">
            YUKTI
          </p>
          <h2 id="cta-heading" className="mt-8 max-w-2xl font-serif text-editorial-md font-light text-fg-bright">
            Bring every piece of evidence <em className="italic text-crimson">into the bigger picture.</em>
          </h2>
          <div className="relative mt-12">
            <Pushpin pinId="cta-pin" className="-top-5 left-1/2 -translate-x-1/2" />
            <CtaLink href={ROUTES.workspace} arrow className="px-7 py-4">
              Open Workspace
            </CtaLink>
          </div>
        </div>
      </div>
    </section>
  );
};

export const SiteFooter: React.FC = () => (
  <footer id={SECTION_IDS.about} aria-labelledby="footer-heading" className="bg-ink-950">
    <h2 id="footer-heading" className="sr-only">
      About YUKTI
    </h2>
    <div className="mx-auto grid max-w-wide gap-12 px-gutter py-16 md:grid-cols-12">
      <div className="md:col-span-5">
        <YuktiWordmark />
        <p className="mt-6 max-w-sm font-serif text-xl leading-snug text-fg-bright">
          Every piece of evidence. Every connection. <em className="italic text-crimson">One investigation.</em>
        </p>
        <p className="mt-5 max-w-sm text-sm leading-relaxed text-fg-muted">Digital Evidence Intelligence &amp; Crime Reconstruction Platform.</p>
        <p className="mt-3 max-w-sm text-sm leading-relaxed text-fg-subtle">
          Built for SUTRAM 2026 — ISEA-IGDTUW National Hackathon · synthetic data only.
        </p>
      </div>

      <nav aria-label="Story" className="md:col-span-3">
        <p className="mb-4 font-mono text-[0.625rem] uppercase tracking-[0.16em] text-fg-subtle">The platform</p>
        <ul className="space-y-2.5">
          {FOOTER_STORY_LINKS.map((link) => (
            <li key={link.href}>
              <a href={link.href} className={FOOTER_LINK}>
                {link.label}
              </a>
            </li>
          ))}
        </ul>
      </nav>

      <nav aria-label="Project" className="md:col-span-4">
        <p className="mb-4 font-mono text-[0.625rem] uppercase tracking-[0.16em] text-fg-subtle">Project</p>
        <ul className="space-y-2.5">
          <li>
            <Link to={ROUTES.workspace} className={FOOTER_LINK}>
              Investigator workspace
            </Link>
          </li>
          <li>
            <a href={`#${SECTION_IDS.technology}`} className={FOOTER_LINK}>
              Technology &amp; build status
            </a>
          </li>
          <li>
            <a href={PROJECT_REPOSITORY_URL} target="_blank" rel="noopener noreferrer" className={`${FOOTER_LINK} inline-flex items-center gap-1`}>
              Repository
              <ArrowUpRight aria-hidden="true" className="h-3.5 w-3.5" />
              <span className="sr-only">(opens in a new tab)</span>
            </a>
          </li>
        </ul>
      </nav>
    </div>

    <div className="border-t border-ink-line">
      <div className="mx-auto flex max-w-wide flex-col gap-4 px-gutter py-6 sm:flex-row sm:items-center sm:justify-between">
        <p className="font-mono text-micro uppercase text-fg-subtle">© 2026 YUKTI project team</p>
        <ApiStatus />
      </div>
    </div>
  </footer>
);
