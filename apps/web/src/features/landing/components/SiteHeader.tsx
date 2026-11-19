import React, { useCallback, useEffect, useId, useRef, useState } from 'react';
import { Menu, X } from 'lucide-react';
import { Link } from '../../../router/Link';
import { ROUTES } from '../../../router/router';
import { NAV_ITEMS, SECTION_IDS } from '../data/siteContent';
import { useActiveSection } from '../hooks/useInView';
import { CtaLink, YuktiWordmark } from '../../../components/ui/primitives';

const NAV_SECTION_IDS = NAV_ITEMS.map((item) => item.href.slice(1));

/** Transparent over the hero; settles into an opaque charcoal bar once the page scrolls. */
function useScrolled(threshold = 24): boolean {
  const [scrolled, setScrolled] = useState(false);
  useEffect(() => {
    const onScroll = (): void => setScrolled(window.scrollY > threshold);
    onScroll();
    window.addEventListener('scroll', onScroll, { passive: true });
    return () => window.removeEventListener('scroll', onScroll);
  }, [threshold]);
  return scrolled;
}

export const SiteNavbar: React.FC = () => {
  const [open, setOpen] = useState(false);
  const active = useActiveSection(NAV_SECTION_IDS);
  const scrolled = useScrolled();
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

  const solid = scrolled || open;

  return (
    <header
      ref={headerRef}
      className={`fixed inset-x-0 top-0 z-50 transition-[background-color,border-color,backdrop-filter] duration-slow ease-out ${
        solid ? 'border-b border-ink-line bg-ink-950/85 backdrop-blur-md' : 'border-b border-transparent bg-transparent'
      }`}
    >
      <nav aria-label="Primary" className="mx-auto flex h-[4.5rem] max-w-wide items-center justify-between gap-6 px-gutter">
        <div className="flex items-center gap-5">
          <Link to={ROUTES.home} aria-label="YUKTI home" className="shrink-0 focus:outline-none focus-visible:ring-2 focus-visible:ring-crimson">
            <YuktiWordmark />
          </Link>
          <span aria-hidden="true" className="hidden h-7 w-px bg-gradient-to-b from-transparent via-crimson/70 to-transparent xl:block" />
          <p className="hidden max-w-[13rem] font-mono text-[0.625rem] uppercase leading-snug tracking-[0.14em] text-fg-subtle xl:block">
            Digital evidence intelligence &amp; crime reconstruction
          </p>
        </div>

        <ul className="hidden items-center lg:flex">
          {NAV_ITEMS.map((item) => {
            const isActive = active === item.href.slice(1);
            return (
              <li key={item.href}>
                <a
                  href={item.href}
                  aria-current={isActive ? 'location' : undefined}
                  className={`group relative block px-4 py-2 text-[0.875rem] tracking-[0.01em] transition-colors duration-base hover:text-fg-bright focus:outline-none focus-visible:text-fg-bright focus-visible:ring-1 focus-visible:ring-crimson ${
                    isActive ? 'text-fg-bright' : 'text-fg-muted'
                  }`}
                >
                  {item.label}
                  {/* a red thread drawn under the item */}
                  <span
                    aria-hidden="true"
                    className={`absolute inset-x-4 -bottom-0.5 h-px origin-left bg-crimson transition-transform duration-slow ease-out group-hover:scale-x-100 ${
                      isActive ? 'scale-x-100' : 'scale-x-0'
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
          <CtaLink href={ROUTES.workspace} variant="secondary" className="py-2.5">
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
          className="inline-flex h-10 w-10 items-center justify-center border border-paper/25 text-fg transition-colors hover:bg-paper/5 focus:outline-none focus-visible:ring-2 focus-visible:ring-crimson lg:hidden"
        >
          {open ? <X aria-hidden="true" className="h-5 w-5" /> : <Menu aria-hidden="true" className="h-5 w-5" />}
        </button>
      </nav>

      <div id={panelId} hidden={!open} className="border-t border-ink-line bg-ink-950 lg:hidden">
        <ul className="mx-auto max-w-site px-gutter py-2">
          {NAV_ITEMS.map((item, index) => (
            <li key={item.href} className="border-b border-ink-line last:border-b-0">
              <a
                href={item.href}
                onClick={() => close()}
                className="flex items-center justify-between py-4 font-serif text-2xl text-fg-bright"
              >
                {item.label}
                <span aria-hidden="true" className="font-mono text-micro text-crimson">
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
