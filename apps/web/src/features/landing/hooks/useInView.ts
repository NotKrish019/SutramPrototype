import { useEffect, useRef, useState } from 'react';

/**
 * Reports when an element first enters the viewport. Content is visible by
 * default where IntersectionObserver is unavailable (tests, old browsers), so
 * the reveal is purely a progressive enhancement and never hides content.
 */
export function useInView<T extends Element>(
  options: { rootMargin?: string; once?: boolean } = {},
): [React.RefObject<T>, boolean] {
  const { rootMargin = '0px 0px -10% 0px', once = true } = options;
  const ref = useRef<T>(null);
  const supported = typeof window !== 'undefined' && 'IntersectionObserver' in window;
  const [inView, setInView] = useState(!supported);

  useEffect(() => {
    const element = ref.current;
    if (!supported || !element) return;
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (!entry) return;
        if (entry.isIntersecting) {
          setInView(true);
          if (once) observer.disconnect();
        } else if (!once) {
          setInView(false);
        }
      },
      { rootMargin },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [supported, rootMargin, once]);

  return [ref, inView];
}

/**
 * Tracks which of the given section ids is currently in view (scroll-spy).
 * Returns null when none is active or IntersectionObserver is unavailable.
 */
export function useActiveSection(ids: readonly string[]): string | null {
  const [active, setActive] = useState<string | null>(null);
  const key = ids.join('|');

  useEffect(() => {
    if (typeof window === 'undefined' || !('IntersectionObserver' in window)) return;
    const visible = new Map<string, number>();
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) visible.set(entry.target.id, entry.intersectionRatio);
          else visible.delete(entry.target.id);
        }
        const ordered = key.split('|').filter((id) => visible.has(id));
        setActive(ordered[0] ?? null);
      },
      { rootMargin: '-35% 0px -55% 0px', threshold: [0, 0.01] },
    );
    for (const id of key.split('|')) {
      const element = document.getElementById(id);
      if (element) observer.observe(element);
    }
    return () => observer.disconnect();
  }, [key]);

  return active;
}
