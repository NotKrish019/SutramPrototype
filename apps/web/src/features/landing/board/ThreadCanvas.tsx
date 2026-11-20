import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Thread, sag, type ThreadState } from './materials';

export interface ThreadLink {
  id: string;
  from: string;
  to: string;
  kind?: 'observed' | 'inferred';
  state?: ThreadState;
  delay?: number;
  /** Short mono label placed along the thread. */
  label?: string;
  /** Where the label sits along the thread, 0 (from) … 1 (to). Defaults to the midpoint. */
  labelT?: number;
}

interface Measured {
  link: ThreadLink;
  d: string;
  mid: [number, number];
}

/** Anchor point for an element marked with `data-pin-id`: the centre of a pin head, else top-centre. */
function anchorOf(element: Element, origin: DOMRect): [number, number] {
  const rect = element.getBoundingClientRect();
  const isPin = element.tagName.toLowerCase() === 'svg';
  return [rect.left + rect.width / 2 - origin.left, rect.top + (isPin ? rect.height * 0.33 : rect.height / 2) - origin.top];
}

/**
 * Whether an anchor is displayed at the current breakpoint. Zero-size anchors are valid (edge tie-off
 * points), so this checks display, not size. Browsers without `checkVisibility` treat anchors as rendered.
 */
function isRendered(element: Element): boolean {
  return typeof element.checkVisibility === 'function' ? element.checkVisibility() : true;
}

/**
 * Draws red threads between elements carrying `data-pin-id` inside the same
 * positioned container. Positions are measured from the live layout, so threads
 * stay attached across every responsive composition. Must be the container's child.
 */
export const ThreadCanvas: React.FC<{
  links: ThreadLink[];
  drawn: boolean;
  sagAmount?: number;
  pulse?: boolean;
  className?: string;
  /** Change to force a re-measure (e.g. after a layout-affecting state change). */
  layoutKey?: string;
  /** Extra classes for thread labels, e.g. to hide them in stacked layouts. */
  labelClassName?: string;
}> = ({ links, drawn, sagAmount = 0.1, pulse = true, className = '', layoutKey = '', labelClassName = '' }) => {
  const svgRef = useRef<SVGSVGElement>(null);
  const [size, setSize] = useState<[number, number]>([0, 0]);
  const [measured, setMeasured] = useState<Measured[]>([]);
  const linksKey = links.map((l) => `${l.id}:${l.from}>${l.to}`).join('|');

  const measure = useCallback(() => {
    const svg = svgRef.current;
    const container = svg?.parentElement;
    if (!svg || !container) return;
    const origin = container.getBoundingClientRect();
    if (origin.width === 0) return;
    setSize([origin.width, origin.height]);
    const next: Measured[] = [];
    for (const link of links) {
      const a = container.querySelector(`[data-pin-id="${link.from}"]`);
      const b = container.querySelector(`[data-pin-id="${link.to}"]`);
      // Skip anchors hidden at this breakpoint: display:none measures as 0×0 at the viewport origin.
      if (!a || !b || !isRendered(a) || !isRendered(b)) continue;
      const [x1, y1] = anchorOf(a, origin);
      const [x2, y2] = anchorOf(b, origin);
      const d = sag(x1, y1, x2, y2, sagAmount);
      // Point on the same quadratic `sag` draws, so labels sit on the visible string.
      const cx = (x1 + x2) / 2;
      const cy = (y1 + y2) / 2 + Math.hypot(x2 - x1, y2 - y1) * sagAmount;
      const t = link.labelT ?? 0.5;
      const u = 1 - t;
      next.push({ link, d, mid: [u * u * x1 + 2 * u * t * cx + t * t * x2, u * u * y1 + 2 * u * t * cy + t * t * y2] });
    }
    setMeasured(next);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [linksKey, sagAmount]);

  useLayoutEffect(() => {
    measure();
    const container = svgRef.current?.parentElement;
    if (!container) return;
    const observer = typeof ResizeObserver !== 'undefined' ? new ResizeObserver(() => measure()) : null;
    observer?.observe(container);
    // Re-measure once entrance transforms and web fonts have settled.
    const timers = [400, 1400, 2600].map((ms) => window.setTimeout(measure, ms));
    void document.fonts?.ready.then(measure);
    // Anchors move when pinned items settle or lift; follow them once each transform transition ends.
    let frame = 0;
    const onTransitionEnd = (event: TransitionEvent): void => {
      if (event.propertyName !== 'transform' || frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        measure();
      });
    };
    container.addEventListener('transitionend', onTransitionEnd);
    return () => {
      observer?.disconnect();
      timers.forEach((t) => window.clearTimeout(t));
      container.removeEventListener('transitionend', onTransitionEnd);
      if (frame) cancelAnimationFrame(frame);
    };
  }, [measure, layoutKey]);

  // Items typically settle into place when the board is revealed, after the mount-time timers have run.
  useEffect(() => {
    if (!drawn) return;
    const timers = [0, 900, 1800].map((ms) => window.setTimeout(measure, ms));
    return () => timers.forEach((t) => window.clearTimeout(t));
  }, [drawn, measure]);

  const states = new Map(links.map((l) => [l.id, l]));

  return (
    <svg
      ref={svgRef}
      aria-hidden="true"
      className={`pointer-events-none absolute inset-0 h-full w-full overflow-visible ${className}`}
      viewBox={`0 0 ${Math.max(1, size[0])} ${Math.max(1, size[1])}`}
      data-drawn={drawn}
    >
      {measured.map(({ link, d }) => {
        const live = states.get(link.id) ?? link;
        return <Thread key={link.id} d={d} kind={live.kind} state={live.state} delay={live.delay} pulse={pulse} />;
      })}
      {measured.map(({ link, mid }) => {
        const live = states.get(link.id) ?? link;
        if (!live.label) return null;
        return (
          <text
            key={`l-${link.id}`}
            x={mid[0]}
            y={mid[1] - 6}
            textAnchor="middle"
            fontFamily="JetBrains Mono, monospace"
            fontSize="10"
            letterSpacing="0.08em"
            className={`svg-label transition-opacity duration-base ${labelClassName}`}
            style={{ stroke: '#0b0a09', strokeWidth: 5 }}
            fill={live.state === 'hot' ? '#ff6b76' : '#71717A'}
            opacity={!drawn ? 0 : live.state === 'dim' ? 0.2 : 1}
          >
            {live.label.toUpperCase()}
          </text>
        );
      })}
    </svg>
  );
};
