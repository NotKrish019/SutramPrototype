import { useCallback, useEffect, useRef, useState } from 'react';

export interface Playhead {
  /** Milliseconds from the start of the whole playback. */
  time: number;
  playing: boolean;
  speed: number;
  play: () => void;
  pause: () => void;
  toggle: () => void;
  seek: (ms: number) => void;
  setSpeed: (speed: number) => void;
}

export function prefersReducedMotion(): boolean {
  return typeof window !== 'undefined' && typeof window.matchMedia === 'function'
    ? window.matchMedia('(prefers-reduced-motion: reduce)').matches
    : false;
}

/**
 * A requestAnimationFrame clock over `[0, total]`. Pauses at the end and while the tab is hidden
 * (rAF does not fire), so nothing runs in the background.
 */
export function usePlayhead(total: number, options: { autoplay?: boolean; initial?: number } = {}): Playhead {
  const [time, setTime] = useState(options.initial ?? 0);
  const [playing, setPlaying] = useState(options.autoplay ?? true);
  const [speed, setSpeed] = useState(1);
  const last = useRef<number | null>(null);
  const timeRef = useRef(time);
  timeRef.current = time;

  useEffect(() => {
    if (!playing) {
      last.current = null;
      return;
    }
    let frame = 0;
    const tick = (now: number): void => {
      const prev = last.current ?? now;
      last.current = now;
      const next = Math.min(timeRef.current + (now - prev) * speed, total);
      setTime(next);
      if (next >= total) {
        setPlaying(false);
        return;
      }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [playing, speed, total]);

  const play = useCallback(() => {
    if (timeRef.current >= total) setTime(0);
    setPlaying(true);
  }, [total]);
  const pause = useCallback(() => setPlaying(false), []);
  const toggle = useCallback(() => {
    setPlaying((value) => {
      if (!value && timeRef.current >= total) setTime(0);
      return !value;
    });
  }, [total]);
  const seek = useCallback((ms: number) => setTime(Math.min(Math.max(ms, 0), total)), [total]);

  return { time, playing, speed, play, pause, toggle, seek, setSpeed };
}
