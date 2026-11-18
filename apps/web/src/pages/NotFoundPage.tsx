import React from 'react';
import { CtaLink, Eyebrow, YuktiWordmark } from '../components/ui/primitives';
import { ROUTES } from '../router/router';

export const NotFoundPage: React.FC<{ pathname: string }> = ({ pathname }) => (
  <main className="site-root flex min-h-screen flex-col items-start justify-center gap-6 bg-ink-950 px-gutter text-fg sm:items-center sm:text-center">
    <YuktiWordmark />
    <Eyebrow>404 · Not found</Eyebrow>
    <h1 className="font-display text-display-md font-bold uppercase">No route at this path.</h1>
    <p className="max-w-md break-all font-mono text-sm text-fg-muted">{pathname}</p>
    <div className="flex flex-col gap-3 sm:flex-row">
      <CtaLink href={ROUTES.home} arrow>
        Back to YUKTI
      </CtaLink>
      <CtaLink href={ROUTES.workspace} variant="secondary">
        Open Workspace
      </CtaLink>
    </div>
  </main>
);
