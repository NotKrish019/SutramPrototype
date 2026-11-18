import React from 'react';
import { DeliverablesSection } from '../features/landing/components/DeliverablesSection';
import { EvidenceSourcesSection } from '../features/landing/components/EvidenceSourcesSection';
import { HeroSection } from '../features/landing/components/HeroSection';
import { PlatformSection } from '../features/landing/components/PlatformSection';
import { ProvenanceSection } from '../features/landing/components/ProvenanceSection';
import { SiteAnnouncementBar, SiteNavbar } from '../features/landing/components/SiteHeader';
import { FinalCTASection, SiteFooter } from '../features/landing/components/SiteFooter';
import { WorkflowSection } from '../features/landing/components/WorkflowSection';
import { ProductPreview } from '../features/landing/components/workbench/ProductPreview';

/** Public product site (`/`). */
export const LandingPage: React.FC = () => (
  <div className="site-root min-h-screen bg-ink-950 text-fg">
    <a
      href="#main"
      className="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-[60] focus:bg-signal-amber focus:px-4 focus:py-2 focus:font-mono focus:text-xs focus:uppercase focus:text-ink-950"
    >
      Skip to content
    </a>
    <SiteAnnouncementBar />
    <SiteNavbar />
    <main id="main">
      <HeroSection />
      <EvidenceSourcesSection />
      <PlatformSection />
      <ProductPreview />
      <WorkflowSection />
      <ProvenanceSection />
      <DeliverablesSection />
      <FinalCTASection />
    </main>
    <SiteFooter />
  </div>
);
