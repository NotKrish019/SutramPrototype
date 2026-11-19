import { useEffect, useRef, useState } from 'react';

/** Live `prefers-reduced-motion` preference. False where matchMedia is unavailable. */
export function useReducedMotion(): boolean {
  const query = '(prefers-reduced-motion: reduce)';
  const [reduced, setReduced] = useState<boolean>(
    () => typeof window !== 'undefined' && typeof window.matchMedia === 'function' && window.matchMedia(query).matches,
  );
  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return;
    const media = window.matchMedia(query);
    const onChange = (event: MediaQueryListEvent): void => setReduced(event.matches);
    media.addEventListener?.('change', onChange);
    return () => media.removeEventListener?.('change', onChange);
  }, []);
  return reduced;
}

/** Live media-query match. Returns `fallback` where matchMedia is unavailable (tests, SSR). */
export function useMediaQuery(query: string, fallback = false): boolean {
  const [matches, setMatches] = useState<boolean>(() =>
    typeof window !== 'undefined' && typeof window.matchMedia === 'function' ? window.matchMedia(query).matches : fallback,
  );
  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return;
    const media = window.matchMedia(query);
    setMatches(media.matches);
    const onChange = (event: MediaQueryListEvent): void => setMatches(event.matches);
    media.addEventListener?.('change', onChange);
    return () => media.removeEventListener?.('change', onChange);
  }, [query]);
  return matches;
}

/**
 * Tracks whether an element is currently on screen. Unlike `useInView` this
 * reports false where IntersectionObserver is unavailable, so it is used only
 * to *start* continuous work (animation loops), never to reveal content.
 */
export function useOnScreen<T extends Element>(rootMargin = '0px'): [React.RefObject<T>, boolean] {
  const ref = useRef<T>(null);
  const [onScreen, setOnScreen] = useState(false);
  useEffect(() => {
    const element = ref.current;
    if (!element || typeof window === 'undefined' || !('IntersectionObserver' in window)) return;
    const observer = new IntersectionObserver(([entry]) => setOnScreen(Boolean(entry?.isIntersecting)), { rootMargin });
    observer.observe(element);
    return () => observer.disconnect();
  }, [rootMargin]);
  return [ref, onScreen];
}

/**
 * Pointer parallax: writes `--px` / `--py` (−1…1) on the element so CSS
 * `.parallax-layer` children can shift by their own `--depth`. Fine pointers only;
 * disabled for reduced motion. Updates are batched to one per animation frame.
 */
export function usePointerParallax<T extends HTMLElement>(): React.RefObject<T> {
  const ref = useRef<T>(null);
  const reduced = useReducedMotion();
  useEffect(() => {
    const element = ref.current;
    if (!element || reduced) return;
    if (typeof window.matchMedia === 'function' && !window.matchMedia('(pointer: fine)').matches) return;
    let frame = 0;
    let nx = 0;
    let ny = 0;
    const apply = (): void => {
      frame = 0;
      element.style.setProperty('--px', nx.toFixed(3));
      element.style.setProperty('--py', ny.toFixed(3));
    };
    const onMove = (event: PointerEvent): void => {
      const rect = element.getBoundingClientRect();
      nx = ((event.clientX - rect.left) / rect.width) * 2 - 1;
      ny = ((event.clientY - rect.top) / rect.height) * 2 - 1;
      if (!frame) frame = requestAnimationFrame(apply);
    };
    const onLeave = (): void => {
      nx = 0;
      ny = 0;
      if (!frame) frame = requestAnimationFrame(apply);
    };
    element.addEventListener('pointermove', onMove);
    element.addEventListener('pointerleave', onLeave);
    return () => {
      element.removeEventListener('pointermove', onMove);
      element.removeEventListener('pointerleave', onLeave);
      if (frame) cancelAnimationFrame(frame);
    };
  }, [reduced]);
  return ref;
}

/** Deterministic PRNG (mulberry32) so generated artwork is identical on every render. */
export function seeded(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
