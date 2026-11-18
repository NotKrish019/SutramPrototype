import React from 'react';
import { ArrowRight } from 'lucide-react';
import { SECTION_IDS } from '../data/siteContent';
import { HeroStack3D } from './HeroStack3D';
import { Container, CtaLink, Eyebrow } from '../../../components/ui/primitives';

const FLOW = ['Fragmented evidence', 'Connected intelligence', 'Clearer investigations'];

export const HeroSection: React.FC = () => (
  <section aria-labelledby="hero-heading" className="relative overflow-hidden border-b border-ink-line">
    <div aria-hidden="true" className="pattern-grid pointer-events-none absolute inset-0 opacity-60 [mask-image:linear-gradient(to_bottom,black,transparent_85%)]" />
    <Container className="relative grid items-center gap-8 pb-10 pt-12 sm:pt-16 lg:grid-cols-12 lg:gap-6 lg:pb-14 lg:pt-16">
      <div className="flex flex-col justify-center lg:col-span-5">
        <Eyebrow className="mb-6">Digital evidence intelligence</Eyebrow>
        <h1 id="hero-heading" className="font-display text-[clamp(2.5rem,4.6vw,3.7rem)] uppercase leading-[0.9] tracking-[-0.02em] text-fg">
          <span className="block font-light">Turn fragmented</span>{' '}
          <span className="block font-light">digital evidence</span>{' '}
          <span className="block font-bold">into connected</span>{' '}
          <span className="block font-bold text-signal-amber">intelligence.</span>
        </h1>
        <p className="mt-6 max-w-md text-lg leading-relaxed text-fg-muted">
          Correlate evidence across sources and reconstruct what happened — with every link traceable.
        </p>
        <div className="mt-8 flex flex-col gap-3 sm:flex-row">
          <CtaLink href={`#${SECTION_IDS.platform}`} arrow>
            Explore the Platform
          </CtaLink>
          <CtaLink href={`#${SECTION_IDS.workflow}`} variant="secondary">
            See How It Works
          </CtaLink>
        </div>
        <p className="mt-6 font-mono text-micro uppercase text-fg-subtle">Investigator decision support · Not an automated verdict</p>
      </div>

      <div className="min-w-0 lg:col-span-7">
        <HeroStack3D />
      </div>
    </Container>

    <div className="relative border-t border-ink-line">
      <Container>
        <ol className="flex flex-col gap-2 py-5 sm:flex-row sm:items-center sm:justify-between" aria-label="What SUTRAM does">
          {FLOW.map((item, index) => (
            <li key={item} className="flex items-center gap-3">
              <span className="font-mono text-micro text-fg-subtle">0{index + 1}</span>
              <span className={`font-display text-xl font-semibold uppercase leading-none sm:text-2xl ${index === 1 ? 'text-signal-amber' : 'text-fg'}`}>{item}.</span>
              {index < FLOW.length - 1 && <ArrowRight aria-hidden="true" className="ml-3 hidden h-4 w-4 text-fg-subtle sm:block" />}
            </li>
          ))}
        </ol>
      </Container>
    </div>
  </section>
);
