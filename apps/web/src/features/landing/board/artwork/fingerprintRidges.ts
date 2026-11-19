import { seeded } from '../hooks';

/**
 * Procedurally generated, entirely synthetic fingerprint ridge pattern (a
 * whorl with ridge endings and a flattened delta region), in a 200×240 box.
 * Not derived from any real print. Computed once at module load.
 */

const CX = 98;
const CY = 112;

function insideFinger(x: number, y: number): boolean {
  // Fingertip silhouette: an egg shape, wider at the top.
  const dx = (x - 100) / 80;
  const dy = (y - 122) / (y < 122 ? 104 : 110);
  return dx * dx + dy * dy <= 1;
}

function clipToPaths(points: [number, number][]): string[] {
  const out: string[] = [];
  let current: string[] = [];
  const flush = (): void => {
    if (current.length > 2) out.push(`M${current.join(' L')}`);
    current = [];
  };
  for (const [x, y] of points) {
    if (insideFinger(x, y)) current.push(`${x.toFixed(1)} ${y.toFixed(1)}`);
    else flush();
  }
  flush();
  return out;
}

/** Split a ridge into pieces separated by small gaps (ridge endings). */
function breakRidge(points: [number, number][], rand: () => number): [number, number][][] {
  const pieces: [number, number][][] = [];
  let cursor = 0;
  while (cursor < points.length - 3) {
    const length = 26 + Math.floor(rand() * 60);
    pieces.push(points.slice(cursor, Math.min(points.length, cursor + length)));
    cursor += length + 2;
  }
  return pieces;
}

function buildRidges(): string[] {
  const rand = seeded(2603);
  const paths: string[] = [];
  const RINGS = 24;
  for (let ring = 1; ring <= RINGS; ring++) {
    const rx = ring * 3.3 + 1.5;
    const ry = ring * 3.9 + 2;
    const phase = rand() * Math.PI * 2;
    const steps = 48 + ring * 8;
    const start = rand() * Math.PI * 2;
    const pts: [number, number][] = [];
    for (let s = 0; s <= steps; s++) {
      const t = start + (Math.PI * 2 * s) / steps;
      const wobble = 1 + 0.018 * Math.sin(4 * t + phase) + 0.01 * Math.sin(9 * t + ring);
      // Lower half of outer rings flattens towards the delta.
      const flatten = Math.sin(t) > 0 ? 1 - Math.min(0.28, ring * 0.012) * Math.sin(t) : 1;
      // Inner core drifts slightly up-right, as in a real whorl.
      const drift = Math.max(0, 6 - ring) * 0.7;
      pts.push([CX + drift + Math.cos(t) * rx * wobble, CY - drift + Math.sin(t) * ry * wobble * flatten]);
    }
    for (const piece of breakRidge(pts, rand)) paths.push(...clipToPaths(piece));
  }
  // Lower arches beneath the whorl (towards the finger crease).
  for (let k = 0; k < 8; k++) {
    const y = 204 + k * 4.6;
    const pts: [number, number][] = [];
    for (let x = 14; x <= 186; x += 3) pts.push([x, y - Math.sin(((x - 14) / 172) * Math.PI) * (12 - k)]);
    for (const piece of breakRidge(pts, rand)) paths.push(...clipToPaths(piece));
  }
  return paths;
}

export const FINGERPRINT_RIDGES: readonly string[] = buildRidges();

/** Points flagged as minutiae on the scan overlay (ridge ending / bifurcation candidates). */
export const FINGERPRINT_MINUTIAE: readonly { x: number; y: number; id: string }[] = [
  { x: 76, y: 88, id: 'M-01' },
  { x: 130, y: 140, id: 'M-02' },
  { x: 66, y: 172, id: 'M-03' },
  { x: 136, y: 70, id: 'M-04' },
];
