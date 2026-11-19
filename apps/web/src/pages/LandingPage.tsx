import React from 'react';
import { ContradictionSection } from '../features/landing/components/ContradictionSection';
import { CorrelationBoard } from '../features/landing/components/CorrelationBoard';
import { EvidenceSourcesBoard } from '../features/landing/components/EvidenceSourcesBoard';
import { HeroBoard } from '../features/landing/components/HeroBoard';
import { ProvenanceChain } from '../features/landing/components/ProvenanceChain';
import { ReconstructionSection } from '../features/landing/components/ReconstructionSection';
import { SiteNavbar } from '../features/landing/components/SiteHeader';
import { FinalCTASection, SiteFooter } from '../features/landing/components/SiteFooter';
import { TechnologySection } from '../features/landing/components/TechnologySection';
import { TimelineSection } from '../features/landing/components/TimelineSection';
import { ProductPreview } from '../features/landing/components/workbench/ProductPreview';

/** Public product site (`/`): a cinematic evidence board that leads into the real workspace. */
export const LandingPage: React.FC = () => (
  <div className="site-root min-h-screen bg-ink-950 text-fg">
    <a
      href="#main"
      className="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-[80] focus:bg-crimson focus:px-4 focus:py-2 focus:font-mono focus:text-xs focus:uppercase focus:text-white"
    >
      Skip to content
    </a>
    <SiteNavbar />
    <main id="main">
      <HeroBoard />
      <EvidenceSourcesBoard />
      <CorrelationBoard />
      <ReconstructionSection />
      <TimelineSection />
      <ContradictionSection />
      <ProvenanceChain />
      <ProductPreview />
      <TechnologySection />
      <FinalCTASection />
    </main>
    <SiteFooter />
  </div>
);
