import React from 'react';
import { ArrowUpRight } from 'lucide-react';
import { useBackendHealth, type ConnectionStatus } from '../../../hooks/useBackendHealth';
import { Link } from '../../../router/Link';
import { ROUTES } from '../../../router/router';
import { DELIVERABLES, PROJECT_REPOSITORY_URL, SECTION_IDS } from '../data/siteContent';
import { Container, CtaLink, Eyebrow, SutramWordmark } from '../../../components/ui/primitives';

const FOOTER_PLATFORM_LINKS = [
  { label: 'Evidence sources', href: `#${SECTION_IDS.sources}` },
  { label: 'Platform', href: `#${SECTION_IDS.platform}` },
  { label: 'Investigation workbench', href: `#${SECTION_IDS.workbench}` },
  { label: 'Workflow', href: `#${SECTION_IDS.workflow}` },
  { label: 'Traceability', href: `#${SECTION_IDS.provenance}` },
];

const STATUS_COPY: Record<ConnectionStatus, { label: string; dot: string }> = {
  checking: { label: 'Checking backend API', dot: 'bg-signal-amber animate-pulse' },
  connected: { label: 'Backend API connected', dot: 'bg-signal-ok' },
  offline: { label: 'Backend API offline', dot: 'bg-signal-flag' },
};

const FOOTER_LINK =
  'text-sm text-fg-muted transition-colors duration-fast hover:text-fg focus:outline-none focus-visible:text-signal-amber';

/** Live status from the existing health monitor — a real signal, not decoration. */
const ApiStatus: React.FC = () => {
  const { status } = useBackendHealth();
  const copy = STATUS_COPY[status];
  return (
    <Link to={ROUTES.workspace} className="inline-flex items-center gap-2 font-mono text-micro uppercase text-fg-muted hover:text-fg">
      <span aria-hidden="true" className={`h-1.5 w-1.5 ${copy.dot}`} />
      <span role="status">{copy.label}</span>
    </Link>
  );
};

export const FinalCTASection: React.FC = () => (
  <section aria-labelledby="cta-heading" className="relative overflow-hidden border-b border-ink-line">
    <div aria-hidden="true" className="pattern-dots absolute inset-0 opacity-50 [mask-image:radial-gradient(ellipse_at_center,transparent_18%,black_70%)]" />
    <svg aria-hidden="true" viewBox="0 0 1200 520" preserveAspectRatio="xMidYMid slice" className="absolute inset-0 h-full w-full">
      {[60, 150, 240, 330, 420, 470].map((y, i) => (
        <path
          key={y}
          d={`M0 ${y} C 300 ${y}, 380 260, 600 260 S 900 ${520 - y}, 1200 ${520 - y}`}
          fill="none"
          stroke={i === 2 ? '#FFB224' : '#EDEDEF'}
          strokeOpacity={i === 2 ? 0.55 : 0.12}
          className="flow-line flow-line--slow"
          style={{ animationDelay: `${i * -0.8}s` }}
        />
      ))}
    </svg>
    <Container className="relative py-28 text-center sm:py-36">
      <Eyebrow className="mb-6">Investigator decision support</Eyebrow>
      <h2 id="cta-heading" className="mx-auto max-w-5xl font-display text-display-lg uppercase text-fg">
        <span className="block font-light">Bring every piece of evidence</span>{' '}
        <span className="block font-bold text-signal-amber">into the bigger picture.</span>
      </h2>
      <p className="mx-auto mt-7 max-w-xl text-lg leading-relaxed text-fg-muted">
        SUTRAM helps transform disconnected digital artifacts into traceable investigative intelligence.
      </p>
      <div className="mt-10 flex flex-col justify-center gap-3 sm:flex-row">
        <CtaLink href={ROUTES.workspace} arrow>
          Explore SUTRAM
        </CtaLink>
        <CtaLink href={`#${SECTION_IDS.workbench}`} variant="secondary">
          View the Platform
        </CtaLink>
      </div>
    </Container>
  </section>
);

export const SiteFooter: React.FC = () => (
  <footer id={SECTION_IDS.about} aria-labelledby="footer-heading" className="bg-ink-950">
    <h2 id="footer-heading" className="sr-only">
      About SUTRAM
    </h2>
    <Container className="grid gap-12 py-16 md:grid-cols-12">
      <div className="md:col-span-5">
        <SutramWordmark />
        <p className="mt-5 max-w-sm text-sm leading-relaxed text-fg-muted">
Digital Evidence Intelligence &amp; Crime Reconstruction Platform.
        </p>
        <p className="mt-5 max-w-sm text-sm leading-relaxed text-fg-muted">
SUTRAM 2026 — ISEA-IGDTUW National Hackathon · synthetic data only.
        </p>
      </div>

      <nav aria-label="Platform" className="md:col-span-2">
        <Eyebrow className="mb-4">Platform</Eyebrow>
        <ul className="space-y-2.5">
          {FOOTER_PLATFORM_LINKS.map((link) => (
            <li key={link.href}>
              <a href={link.href} className={FOOTER_LINK}>
                {link.label}
              </a>
            </li>
          ))}
        </ul>
      </nav>

      <nav aria-label="Capabilities" className="md:col-span-3">
        <Eyebrow className="mb-4">Capabilities</Eyebrow>
        <ul className="space-y-2.5">
          {DELIVERABLES.map((item) => (
            <li key={item.id}>
              <a href={`#capability-${item.id}`} className={FOOTER_LINK}>
                {item.title}
              </a>
            </li>
          ))}
        </ul>
      </nav>

      <nav aria-label="Project" className="md:col-span-2">
        <Eyebrow className="mb-4">Project</Eyebrow>
        <ul className="space-y-2.5">
          <li>
            <Link to={ROUTES.workspace} className={FOOTER_LINK}>
              Investigator workspace
            </Link>
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
    </Container>

    <div className="border-t border-ink-line">
      <Container className="flex flex-col gap-4 py-6 sm:flex-row sm:items-center sm:justify-between">
        <p className="font-mono text-micro uppercase text-fg-subtle">© 2026 SUTRAM project team</p>
        <ApiStatus />
      </Container>
    </div>

    <div aria-hidden="true" className="overflow-hidden border-t border-ink-line">
      <p className="select-none whitespace-nowrap text-center font-display text-[22vw] font-bold uppercase leading-[0.78] tracking-[-0.04em] text-ink-800">
        Sutram
      </p>
    </div>
  </footer>
);
