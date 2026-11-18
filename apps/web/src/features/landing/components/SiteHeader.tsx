import React, { useCallback, useEffect, useId, useRef, useState } from 'react';
import { ArrowRight, Menu, X } from 'lucide-react';
import { Link } from '../../../router/Link';
import { ROUTES } from '../../../router/router';
import { NAV_ITEMS, SECTION_IDS } from '../data/siteContent';
import { useActiveSection } from '../hooks/useInView';
import { CtaLink, SutramWordmark } from '../../../components/ui/primitives';

const NAV_SECTION_IDS = NAV_ITEMS.map((item) => item.href.slice(1));

export const SiteAnnouncementBar: React.FC = () => (
  <div className="border-b border-ink-line bg-ink-950">
    <div className="mx-auto flex max-w-site items-center justify-center gap-3 px-gutter py-2.5 font-mono text-micro uppercase text-fg-muted">
      <span aria-hidden="true" className="h-1.5 w-1.5 bg-signal-amber" />
      <span className="truncate">
        Introducing SUTRAM <span className="hidden sm:inline">— Digital Evidence Intelligence</span>
      </span>
      <a
        href={`#${SECTION_IDS.workbench}`}
        className="group hidden items-center gap-1 text-fg underline-offset-4 hover:underline sm:inline-flex"
      >
        See the workbench
        <ArrowRight aria-hidden="true" className="h-3 w-3 transition-transform duration-fast group-hover:translate-x-0.5" />
      </a>
    </div>
  </div>
);

export const SiteNavbar: React.FC = () => {
  const [open, setOpen] = useState(false);
  const active = useActiveSection(NAV_SECTION_IDS);
  const panelId = useId();
  const toggleRef = useRef<HTMLButtonElement>(null);
  const headerRef = useRef<HTMLElement>(null);

  const close = useCallback((restoreFocus = false) => {
    setOpen(false);
    if (restoreFocus) toggleRef.current?.focus();
  }, []);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') close(true);
    };
    const onPointer = (event: PointerEvent): void => {
      if (headerRef.current && !headerRef.current.contains(event.target as Node)) close();
    };
    const desktop = window.matchMedia?.('(min-width: 1024px)');
    const onDesktop = (event: MediaQueryListEvent): void => {
      if (event.matches) close();
    };
    document.addEventListener('keydown', onKey);
    document.addEventListener('pointerdown', onPointer);
    desktop?.addEventListener?.('change', onDesktop);
    return () => {
      document.removeEventListener('keydown', onKey);
      document.removeEventListener('pointerdown', onPointer);
      desktop?.removeEventListener?.('change', onDesktop);
    };
  }, [open, close]);

  return (
    <header ref={headerRef} className="sticky top-0 z-50 border-b border-ink-line bg-ink-950/90 backdrop-blur-md">
      <nav aria-label="Primary" className="mx-auto flex h-16 max-w-site items-center justify-between gap-6 px-gutter">
        <Link to={ROUTES.home} aria-label="SUTRAM home" className="shrink-0 focus:outline-none focus-visible:ring-2 focus-visible:ring-signal-amber">
          <SutramWordmark />
        </Link>

        <ul className="hidden items-center gap-1 lg:flex">
          {NAV_ITEMS.map((item) => {
            const isActive = active === item.href.slice(1);
            return (
              <li key={item.href}>
                <a
                  href={item.href}
                  aria-current={isActive ? 'location' : undefined}
                  className={`relative px-3 py-2 font-mono text-[0.8125rem] uppercase tracking-[0.04em] transition-colors duration-fast hover:text-fg focus:outline-none focus-visible:text-fg focus-visible:ring-1 focus-visible:ring-signal-amber ${
                    isActive ? 'text-fg' : 'text-fg-muted'
                  }`}
                >
                  {item.label}
                  <span
                    aria-hidden="true"
                    className={`absolute inset-x-3 -bottom-[13px] h-px bg-signal-amber transition-opacity duration-base ${
                      isActive ? 'opacity-100' : 'opacity-0'
                    }`}
                  />
                </a>
              </li>
            );
          })}
        </ul>

        <div className="hidden items-center gap-2 lg:flex">
          <CtaLink href={`#${SECTION_IDS.platform}`} variant="ghost" className="px-3">
            Explore Platform
          </CtaLink>
          <CtaLink href={ROUTES.workspace} variant="secondary" className="py-2">
            Open Workspace
          </CtaLink>
        </div>

        <button
          ref={toggleRef}
          type="button"
          aria-expanded={open}
          aria-controls={panelId}
          aria-label={open ? 'Close menu' : 'Open menu'}
          onClick={() => setOpen((value) => !value)}
          className="inline-flex h-10 w-10 items-center justify-center border border-ink-strong text-fg transition-colors hover:bg-white/5 focus:outline-none focus-visible:ring-2 focus-visible:ring-signal-amber lg:hidden"
        >
          {open ? <X aria-hidden="true" className="h-5 w-5" /> : <Menu aria-hidden="true" className="h-5 w-5" />}
        </button>
      </nav>

      <div
        id={panelId}
        hidden={!open}
        className="border-t border-ink-line bg-ink-950 lg:hidden"
      >
        <ul className="mx-auto max-w-site px-gutter py-2">
          {NAV_ITEMS.map((item, index) => (
            <li key={item.href} className="border-b border-ink-line last:border-b-0">
              <a
                href={item.href}
                onClick={() => close()}
                className="flex items-center justify-between py-4 font-display text-2xl font-semibold uppercase text-fg"
              >
                {item.label}
                <span aria-hidden="true" className="font-mono text-micro text-fg-subtle">
                  0{index + 1}
                </span>
              </a>
            </li>
          ))}
        </ul>
        <div className="mx-auto grid max-w-site gap-2 px-gutter pb-6 sm:grid-cols-2">
          <CtaLink href={`#${SECTION_IDS.platform}`} variant="secondary" onClick={() => close()}>
            Explore Platform
          </CtaLink>
          <CtaLink href={ROUTES.workspace} variant="primary" arrow onClick={() => close()}>
            Open Workspace
          </CtaLink>
        </div>
      </div>
    </header>
  );
};
