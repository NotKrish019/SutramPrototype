/** Motion maths shared by the theatre scenes. All inputs and outputs are plain numbers. */

export const clamp = (v: number, lo = 0, hi = 1): number => Math.min(hi, Math.max(lo, v));

/** Progress of `p` through the window [a, b], clamped to 0..1. */
export const seg = (p: number, a: number, b: number): number => clamp((p - a) / Math.max(b - a, 1e-6));

export const lerp = (a: number, b: number, t: number): number => a + (b - a) * t;

/** Decelerating curve: fast start, soft landing (the brand rule: never bounce). */
export const easeOut = (t: number): number => 1 - Math.pow(1 - clamp(t), 3);
export const easeInOut = (t: number): number => {
  const x = clamp(t);
  return x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2;
};
export const easeOutExpo = (t: number): number => (t >= 1 ? 1 : 1 - Math.pow(2, -10 * clamp(t)));

/** Deterministic pseudo-random hex digit for the hash "decode" effect. */
export function hexNoise(index: number, frame: number): string {
  const x = Math.sin(index * 12.9898 + frame * 78.233) * 43758.5453;
  return '0123456789abcdef'[Math.floor((x - Math.floor(x)) * 16)]!;
}

/** Split text into lines of at most `max` characters on word boundaries. */
export function wrap(text: string, max: number, maxLines = 3): string[] {
  const words = text.split(/\s+/).filter(Boolean);
  const lines: string[] = [];
  let line = '';
  for (const word of words) {
    if ((line + ' ' + word).trim().length > max) {
      if (line) lines.push(line);
      line = word;
      if (lines.length === maxLines) break;
    } else {
      line = (line + ' ' + word).trim();
    }
  }
  if (line && lines.length < maxLines) lines.push(line);
  if (lines.length === maxLines && words.join(' ').length > lines.join(' ').length) {
    lines[maxLines - 1] = `${lines[maxLines - 1]!.replace(/[.,;:]?$/, '')}…`;
  }
  return lines;
}
