/**
 * Pointing gesture maths for the overlay window (ADR 0013).
 *
 * Rust converts screen pixels to the overlay's CSS pixels and sends a
 * `PointRequest` (see app/src-tauri/src/overlay/mod.rs). These pure functions
 * turn that request plus the elapsed time into what to draw.
 */

export const POINT_EVENT = "overlay-point";

export type Point = { x: number; y: number };
export type CssRect = { x: number; y: number; width: number; height: number };

/** Payload of the `overlay-point` event. All positions are CSS pixels. */
export type PointRequest = {
  sequence: number;
  from: Point;
  target: CssRect;
  scale: number;
  flightMs: number;
  holdMs: number;
  fadeMs: number;
};

export type OverlayFrame = {
  cursor: Point;
  cursorOpacity: number;
  highlightOpacity: number;
  done: boolean;
};

/** How far the arc bows out, as a fraction of the straight-line distance. */
export const ARC_BEND = 0.2;

const clamp01 = (t: number) => (Number.isFinite(t) ? Math.min(1, Math.max(0, t)) : 1);

/** Slow start, fast middle, gentle landing. */
export function easeInOutCubic(t: number): number {
  const c = clamp01(t);
  return c < 0.5 ? 4 * c * c * c : 1 - Math.pow(-2 * c + 2, 3) / 2;
}

export function rectCenter(rect: CssRect): Point {
  return { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 };
}

/**
 * Control point of a quadratic Bézier from `from` to `to` that bows upwards
 * (towards smaller y), like a hand lifting off the desk. Never above y = 0,
 * so the arc stays on screen.
 */
export function arcControlPoint(from: Point, to: Point, bend = ARC_BEND): Point {
  const mid = { x: (from.x + to.x) / 2, y: (from.y + to.y) / 2 };
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const length = Math.hypot(dx, dy);
  if (length === 0) return mid;
  let nx = -dy / length;
  let ny = dx / length;
  if (ny > 0) {
    nx = -nx;
    ny = -ny;
  }
  return { x: mid.x + nx * length * bend, y: Math.max(0, mid.y + ny * length * bend) };
}

/** Position along the eased arc at linear progress `t` (0 to 1). */
export function pointOnArc(from: Point, to: Point, t: number, bend = ARC_BEND): Point {
  const s = easeInOutCubic(t);
  const control = arcControlPoint(from, to, bend);
  const a = (1 - s) * (1 - s);
  const b = 2 * (1 - s) * s;
  const c = s * s;
  return {
    x: a * from.x + b * control.x + c * to.x,
    y: a * from.y + b * control.y + c * to.y,
  };
}

/** What to draw `elapsedMs` after the gesture started: fly, hold, fade. */
export function overlayFrame(request: PointRequest, elapsedMs: number): OverlayFrame {
  const to = rectCenter(request.target);
  const flight = Math.max(1, request.flightMs);
  if (elapsedMs < flight) {
    return {
      cursor: pointOnArc(request.from, to, elapsedMs / flight),
      cursorOpacity: 1,
      highlightOpacity: 0,
      done: false,
    };
  }
  const holdEnd = flight + Math.max(0, request.holdMs);
  if (elapsedMs < holdEnd) {
    return { cursor: to, cursorOpacity: 1, highlightOpacity: 1, done: false };
  }
  const fade = Math.max(1, request.fadeMs);
  const progress = clamp01((elapsedMs - holdEnd) / fade);
  const opacity = 1 - progress;
  return { cursor: to, cursorOpacity: opacity, highlightOpacity: opacity, done: progress >= 1 };
}

/**
 * True when the scale Rust used differs from the page's devicePixelRatio,
 * which would put the cursor in the wrong place.
 */
export function scaleMismatch(requestScale: number, devicePixelRatio: number): boolean {
  return Math.abs(requestScale - devicePixelRatio) > 0.01;
}
