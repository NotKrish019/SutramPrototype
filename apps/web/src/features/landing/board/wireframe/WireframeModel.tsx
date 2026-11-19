import React, { useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
import { useOnScreen, useReducedMotion } from '../hooks';
import { MARKERS, project, renderScene, type Camera, type Projected } from './model';

export interface WireframeModelProps {
  width?: number;
  height?: number;
  initialYaw?: number;
  initialPitch?: number;
  distance?: number;
  focal?: number;
  /** Slow continuous rotation while on screen (never under reduced motion). */
  autoRotate?: boolean;
  /** Drag / arrow keys rotate the model. */
  interactive?: boolean;
  showRoute?: boolean;
  showMarkers?: boolean;
  /** Small marker labels next to each sensor. */
  showLabels?: boolean;
  selectedMarker?: string | null;
  onSelectMarker?: (id: string) => void;
  label: string;
  className?: string;
}

const PITCH_MIN = 0.22;
const PITCH_MAX = 1.1;
const AUTO_SPEED = 0.11; // rad/s

function pointAlong(points: Projected[], t: number): Projected | null {
  if (points.length < 2) return null;
  const lengths: number[] = [];
  let total = 0;
  for (let i = 1; i < points.length; i++) {
    const l = Math.hypot(points[i]!.x - points[i - 1]!.x, points[i]!.y - points[i - 1]!.y);
    lengths.push(l);
    total += l;
  }
  let target = total * t;
  for (let i = 0; i < lengths.length; i++) {
    const l = lengths[i]!;
    if (target <= l) {
      const a = points[i]!;
      const b = points[i + 1]!;
      const k = l === 0 ? 0 : target / l;
      return { x: a.x + (b.x - a.x) * k, y: a.y + (b.y - a.y) * k, depth: a.depth };
    }
    target -= l;
  }
  return points[points.length - 1]!;
}

/**
 * Interactive wireframe reconstruction of the synthetic Warehouse 7 site, with
 * sensor markers and the reconstructed movement path in crimson. Rendered with
 * a perspective projector into SVG — real 3D geometry, no WebGL required.
 */
export const WireframeModel: React.FC<WireframeModelProps> = ({
  width = 800,
  height = 560,
  initialYaw = -0.62,
  initialPitch = 0.6,
  distance = 62,
  focal = 820,
  autoRotate = false,
  interactive = false,
  showRoute = true,
  showMarkers = true,
  showLabels = true,
  selectedMarker = null,
  onSelectMarker,
  label,
  className = '',
}) => {
  const id = useId().replace(/:/g, '');
  const [yaw, setYaw] = useState(initialYaw);
  const [pitch, setPitch] = useState(initialPitch);
  const [tracer, setTracer] = useState(1);
  const [dragging, setDragging] = useState(false);
  const [hovering, setHovering] = useState(false);
  const reduced = useReducedMotion();
  const [ref, onScreen] = useOnScreen<HTMLDivElement>('120px');
  const drag = useRef<{ x: number; y: number; yaw: number; pitch: number } | null>(null);

  const camera: Camera = useMemo(
    () => ({ yaw, pitch, distance, focal, width, height }),
    [yaw, pitch, distance, focal, width, height],
  );
  const scene = useMemo(() => renderScene(camera), [camera]);

  // Continuous motion: slow rotation + a tracer travelling the route. Only while visible.
  useEffect(() => {
    if (reduced || !onScreen || dragging) return;
    let frame = 0;
    let last = performance.now();
    const tick = (now: number): void => {
      const dt = Math.min(0.05, (now - last) / 1000);
      last = now;
      if (autoRotate && !hovering) setYaw((y) => y + dt * AUTO_SPEED);
      if (showRoute) setTracer((t) => (t + dt * 0.16) % 1);
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [reduced, onScreen, dragging, autoRotate, hovering, showRoute]);

  const onPointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      if (!interactive || (event.target as Element).closest('[data-marker]')) return;
      drag.current = { x: event.clientX, y: event.clientY, yaw, pitch };
      event.currentTarget.setPointerCapture?.(event.pointerId);
      setDragging(true);
    },
    [interactive, yaw, pitch],
  );

  const onPointerMove = useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    const start = drag.current;
    if (!start) return;
    setYaw(start.yaw + (event.clientX - start.x) * 0.008);
    setPitch(Math.min(PITCH_MAX, Math.max(PITCH_MIN, start.pitch + (event.clientY - start.y) * 0.004)));
  }, []);

  const endDrag = useCallback(() => {
    drag.current = null;
    setDragging(false);
  }, []);

  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>): void => {
    if (!interactive || event.target !== event.currentTarget) return;
    const step = 0.12;
    if (event.key === 'ArrowLeft') setYaw((y) => y - step);
    else if (event.key === 'ArrowRight') setYaw((y) => y + step);
    else if (event.key === 'ArrowUp') setPitch((p) => Math.max(PITCH_MIN, p - step / 2));
    else if (event.key === 'ArrowDown') setPitch((p) => Math.min(PITCH_MAX, p + step / 2));
    else return;
    event.preventDefault();
  };

  const tracerPoint = showRoute && !reduced ? pointAlong(scene.routePoints, tracer) : null;
  const routeStart = scene.routePoints[0];
  const routeEnd = scene.routePoints[scene.routePoints.length - 1];

  return (
    <div
      ref={ref}
      className={`relative select-none ${interactive ? (dragging ? 'cursor-grabbing' : 'cursor-grab') : ''} ${className}`}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onPointerEnter={() => setHovering(true)}
      onPointerLeave={() => setHovering(false)}
      onKeyDown={onKeyDown}
      tabIndex={interactive ? 0 : undefined}
      role={interactive ? 'group' : undefined}
      aria-roledescription={interactive ? '3D model' : undefined}
      aria-label={interactive ? `${label}. Drag or use arrow keys to rotate.` : undefined}
      style={{ touchAction: interactive ? 'pan-y' : undefined }}
    >
      <svg viewBox={`0 0 ${width} ${height}`} className="block h-full w-full" role="img" aria-label={label}>
        <defs>
          <filter id={`glow-${id}`} x="-20%" y="-20%" width="140%" height="140%">
            <feGaussianBlur stdDeviation="5" />
          </filter>
          <radialGradient id={`pool-${id}`} cx="0.5" cy="0.55" r="0.55">
            <stop offset="0" stopColor="#8FA9C4" stopOpacity="0.1" />
            <stop offset="1" stopColor="#8FA9C4" stopOpacity="0" />
          </radialGradient>
        </defs>
        <rect width={width} height={height} fill={`url(#pool-${id})`} />
        <path d={scene.floor} stroke="#8FA9C4" strokeOpacity="0.09" strokeWidth="1" fill="none" />
        {/* far → near so nearer structure overdraws */}
        {[2, 1, 0].map((band) => (
          <g key={band} fill="none" strokeLinecap="round">
            <path d={scene.studs[band]} stroke="#8FA9C4" strokeOpacity={[0.55, 0.36, 0.18][band]} strokeWidth="0.9" />
            <path d={scene.walls[band]} stroke="#dbe5ef" strokeOpacity={[0.92, 0.6, 0.3][band]} strokeWidth={[1.6, 1.25, 1][band]} />
          </g>
        ))}

        {showRoute && (
          <g fill="none" strokeLinecap="round" strokeLinejoin="round">
            <path d={scene.route} stroke="#ff2a3d" strokeWidth="9" strokeOpacity="0.45" filter={`url(#glow-${id})`} />
            <path d={scene.route} stroke="#ff3b4a" strokeWidth="2.6" />
            <path d={scene.route} stroke="#ffd7da" strokeWidth="1" strokeDasharray="2 9" />
            {routeStart && <circle cx={routeStart.x} cy={routeStart.y} r="5" fill="#ff3b4a" stroke="#ffe3e5" strokeWidth="1.5" />}
            {routeEnd && (
              <>
                <circle cx={routeEnd.x} cy={routeEnd.y} r="13" fill="#ff3b4a" fillOpacity="0.18" />
                <circle cx={routeEnd.x} cy={routeEnd.y} r="6" fill="#ff3b4a" stroke="#ffe3e5" strokeWidth="1.5" />
              </>
            )}
            {tracerPoint && <circle cx={tracerPoint.x} cy={tracerPoint.y} r="4" fill="#fff" filter={`url(#glow-${id})`} />}
            {tracerPoint && <circle cx={tracerPoint.x} cy={tracerPoint.y} r="2.4" fill="#fff" />}
          </g>
        )}

        {showMarkers &&
          MARKERS.map((marker) => {
            const p = scene.markers[marker.id]!;
            const ground = project([marker.position[0], 0, marker.position[2]], camera);
            const active = selectedMarker === marker.id;
            const interactiveMarker = Boolean(onSelectMarker);
            return (
              <g
                key={marker.id}
                data-marker={marker.id}
                role={interactiveMarker ? 'button' : undefined}
                tabIndex={interactiveMarker ? 0 : undefined}
                aria-pressed={interactiveMarker ? active : undefined}
                aria-label={interactiveMarker ? `${marker.label}, evidence ${marker.evidenceId}` : undefined}
                onClick={interactiveMarker ? () => onSelectMarker?.(marker.id) : undefined}
                onKeyDown={
                  interactiveMarker
                    ? (event) => {
                        if (event.key === 'Enter' || event.key === ' ') {
                          event.preventDefault();
                          onSelectMarker?.(marker.id);
                        }
                      }
                    : undefined
                }
                className={interactiveMarker ? 'cursor-pointer outline-none [&:focus-visible>circle.ring]:stroke-white' : undefined}
              >
                <line x1={p.x} y1={p.y} x2={ground.x} y2={ground.y} stroke={active ? '#ff3b4a' : '#dbe5ef'} strokeOpacity="0.5" strokeDasharray="2 3" />
                <ellipse cx={ground.x} cy={ground.y} rx="7" ry="2.6" fill="none" stroke={active ? '#ff3b4a' : '#dbe5ef'} strokeOpacity="0.5" />
                {active && <circle cx={p.x} cy={p.y} r="12" fill="#ff3b4a" className="pulse-node" />}
                <circle cx={p.x} cy={p.y} r="16" fill="transparent" />
                <circle className="ring" cx={p.x} cy={p.y} r="7.5" fill="#0b0c0e" stroke={active ? '#ff3b4a' : '#dbe5ef'} strokeWidth="1.6" />
                <circle cx={p.x} cy={p.y} r="2.6" fill={active ? '#ff3b4a' : '#dbe5ef'} />
                {showLabels && (
                  <text
                    x={p.x + 12}
                    y={p.y - 9}
                    fontFamily="JetBrains Mono, monospace"
                    fontSize="11"
                    letterSpacing="0.06em"
                    fill={active ? '#ff6b76' : '#c4d0dc'}
                    className="svg-label"
                    style={{ stroke: '#0a0b0c' }}
                  >
                    {marker.id}
                  </text>
                )}
              </g>
            );
          })}
      </svg>
    </div>
  );
};
