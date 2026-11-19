/**
 * Synthetic spatial reconstruction of "Warehouse 7" (CASE-026 demonstration).
 * Units are metres: x → east, y → up, z → north. Purely illustrative geometry.
 *
 * The scene is rendered by a tiny perspective projector (below) into SVG paths,
 * which keeps the 3D model dependency-free, crisp at any resolution and cheap
 * enough to re-project every animation frame.
 */

export type Vec3 = readonly [number, number, number];
export type Vec2 = readonly [number, number];

export interface Segment {
  a: Vec3;
  b: Vec3;
  /** wall: structure · stud: wall mesh · floor: ground grid */
  kind: 'wall' | 'stud' | 'floor';
}

export interface SceneMarker {
  id: string;
  label: string;
  position: Vec3;
  kind: 'camera' | 'sensor' | 'access-point';
  /** Evidence this sensor produced in the synthetic case. */
  evidenceId: string;
  time: string;
  summary: string;
}

const H = 4.2; // wall height
const STUD = 1.6; // wall mesh spacing

type Wall = readonly [number, number, number, number]; // x1 z1 x2 z2

/** Floor plan: outer shell with Gate B opening on the south wall, plus interior rooms. */
export const WALLS: readonly Wall[] = [
  // south wall, split by Gate B (x 26–32)
  [0, 0, 26, 0],
  [32, 0, 40, 0],
  // east, north, west
  [40, 0, 40, 24],
  [40, 24, 0, 24],
  [0, 24, 0, 0],
  // interior: office block (west) and loading bay (north-east)
  [14, 0, 14, 10],
  [0, 14, 10, 14],
  [14, 14, 14, 24],
  [24, 16, 40, 16],
  [24, 16, 24, 21],
];

function wallSegments([x1, z1, x2, z2]: Wall): Segment[] {
  const out: Segment[] = [];
  const len = Math.hypot(x2 - x1, z2 - z1);
  const n = Math.max(1, Math.round(len / STUD));
  out.push({ a: [x1, 0, z1], b: [x2, 0, z2], kind: 'wall' });
  out.push({ a: [x1, H, z1], b: [x2, H, z2], kind: 'wall' });
  for (const y of [H / 3, (2 * H) / 3]) out.push({ a: [x1, y, z1], b: [x2, y, z2], kind: 'stud' });
  for (let i = 0; i <= n; i++) {
    const t = i / n;
    const x = x1 + (x2 - x1) * t;
    const z = z1 + (z2 - z1) * t;
    out.push({ a: [x, 0, z], b: [x, H, z], kind: i === 0 || i === n ? 'wall' : 'stud' });
  }
  return out;
}

function floorGrid(): Segment[] {
  const out: Segment[] = [];
  for (let x = -8; x <= 48; x += 4) out.push({ a: [x, 0, -10], b: [x, 0, 30], kind: 'floor' });
  for (let z = -10; z <= 30; z += 4) out.push({ a: [-8, 0, z], b: [48, 0, z], kind: 'floor' });
  return out;
}

export const SEGMENTS: readonly Segment[] = [...floorGrid(), ...WALLS.flatMap(wallSegments)];

/** Reconstructed movement path (approach → Gate B → interior access point). */
export const ROUTE: readonly Vec3[] = [
  [36, 0, -9],
  [31, 0, -4],
  [29, 0, 0],
  [29, 0, 6],
  [24, 0, 9],
  [19, 0, 11],
  [19, 0, 19],
];

export const MARKERS: readonly SceneMarker[] = [
  {
    id: 'CAM-03',
    label: 'CAM-03 · Gate B camera',
    position: [32.6, H + 0.4, -0.4],
    kind: 'camera',
    evidenceId: 'EV-01',
    time: '22:41:07',
    summary: 'Gate opens; vehicle with partial plate “…47” present. Camera clock — offset unverified.',
  },
  {
    id: 'ACS-B',
    label: 'ACS-B · Gate B controller',
    position: [25.4, 1.4, 0.2],
    kind: 'sensor',
    evidenceId: 'EV-02',
    time: '22:43:52',
    summary: 'Badge 4471 granted entry at Gate B (controller clock).',
  },
  {
    id: 'AP-WH7',
    label: 'AP-WH7 · Wi-Fi access point',
    position: [19, 3.6, 19.5],
    kind: 'access-point',
    evidenceId: 'EV-05',
    time: '22:44:10',
    summary: 'Device MAC matching D-01 associates with the warehouse access point.',
  },
];

const CENTER: Vec3 = [20, 1.6, 10];

export interface Camera {
  /** Rotation about the vertical axis, radians. */
  yaw: number;
  /** Downward tilt, radians. */
  pitch: number;
  /** Distance from the model centre, metres. */
  distance: number;
  /** Focal scale in output units. */
  focal: number;
  width: number;
  height: number;
}

export interface Projected {
  x: number;
  y: number;
  /** Camera-space depth (larger = farther). */
  depth: number;
}

export function project(p: Vec3, cam: Camera): Projected {
  const x0 = p[0] - CENTER[0];
  const y0 = p[1] - CENTER[1];
  const z0 = p[2] - CENTER[2];
  const cy = Math.cos(cam.yaw);
  const sy = Math.sin(cam.yaw);
  const x1 = x0 * cy - z0 * sy;
  const z1 = x0 * sy + z0 * cy;
  const cp = Math.cos(cam.pitch);
  const sp = Math.sin(cam.pitch);
  const y2 = y0 * cp - z1 * sp;
  const z2 = y0 * sp + z1 * cp;
  const depth = cam.distance + z2;
  const s = cam.focal / Math.max(depth, 0.1);
  return { x: cam.width / 2 + x1 * s, y: cam.height / 2 - y2 * s, depth };
}

export interface RenderedScene {
  /** Wall paths in three depth bands (near → far) for atmospheric falloff. */
  walls: [string, string, string];
  studs: [string, string, string];
  floor: string;
  route: string;
  routePoints: Projected[];
  markers: Record<string, Projected>;
}

const fmt = (n: number): string => n.toFixed(1);

/** Projects the whole scene into SVG path data for the given camera. */
export function renderScene(cam: Camera): RenderedScene {
  const walls: [string[], string[], string[]] = [[], [], []];
  const studs: [string[], string[], string[]] = [[], [], []];
  const floor: string[] = [];
  const near = cam.distance - 14;
  const far = cam.distance + 14;
  for (const seg of SEGMENTS) {
    const a = project(seg.a, cam);
    const b = project(seg.b, cam);
    const d = `M${fmt(a.x)} ${fmt(a.y)}L${fmt(b.x)} ${fmt(b.y)}`;
    if (seg.kind === 'floor') {
      floor.push(d);
      continue;
    }
    const depth = (a.depth + b.depth) / 2;
    const band = depth < near + (far - near) / 3 ? 0 : depth < near + (2 * (far - near)) / 3 ? 1 : 2;
    (seg.kind === 'wall' ? walls : studs)[band].push(d);
  }
  const routePoints = ROUTE.map((p) => project(p, cam));
  const route = routePoints.map((p, i) => `${i ? 'L' : 'M'}${fmt(p.x)} ${fmt(p.y)}`).join('');
  const markers: Record<string, Projected> = {};
  for (const m of MARKERS) markers[m.id] = project(m.position, cam);
  return {
    walls: [walls[0].join(''), walls[1].join(''), walls[2].join('')],
    studs: [studs[0].join(''), studs[1].join(''), studs[2].join('')],
    floor: floor.join(''),
    route,
    routePoints,
    markers,
  };
}
