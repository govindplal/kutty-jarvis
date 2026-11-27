import { describe, expect, it } from "vitest";
import {
  arcControlPoint,
  easeInOutCubic,
  overlayFrame,
  pointOnArc,
  rectCenter,
  scaleMismatch,
  type PointRequest,
} from "./pointing";

const request: PointRequest = {
  sequence: 1,
  from: { x: 100, y: 500 },
  target: { x: 580, y: 280, width: 40, height: 40 },
  scale: 1.5,
  flightMs: 600,
  holdMs: 900,
  fadeMs: 300,
};

describe("easeInOutCubic", () => {
  it("starts at 0, ends at 1 and is symmetric around the middle", () => {
    expect(easeInOutCubic(0)).toBe(0);
    expect(easeInOutCubic(1)).toBe(1);
    expect(easeInOutCubic(0.5)).toBeCloseTo(0.5);
    expect(easeInOutCubic(0.25) + easeInOutCubic(0.75)).toBeCloseTo(1);
  });

  it("starts slowly and clamps out-of-range input", () => {
    expect(easeInOutCubic(0.1)).toBeLessThan(0.1);
    expect(easeInOutCubic(-3)).toBe(0);
    expect(easeInOutCubic(7)).toBe(1);
    expect(easeInOutCubic(Number.NaN)).toBe(1);
  });
});

describe("arc", () => {
  it("bows upwards whichever way the cursor travels", () => {
    const right = arcControlPoint({ x: 0, y: 400 }, { x: 400, y: 400 });
    const left = arcControlPoint({ x: 400, y: 400 }, { x: 0, y: 400 });
    expect(right).toEqual({ x: 200, y: 320 });
    expect(left).toEqual({ x: 200, y: 320 });
  });

  it("never goes above the top of the screen", () => {
    expect(arcControlPoint({ x: 0, y: 10 }, { x: 800, y: 10 }).y).toBe(0);
  });

  it("degenerates to a point when start and end coincide", () => {
    const p = { x: 50, y: 60 };
    expect(arcControlPoint(p, p)).toEqual(p);
    const q = pointOnArc(p, p, 0.4);
    expect(q.x).toBeCloseTo(50);
    expect(q.y).toBeCloseTo(60);
  });

  it("starts at the cursor, ends at the target and passes above the straight line", () => {
    const from = { x: 0, y: 400 };
    const to = { x: 400, y: 400 };
    expect(pointOnArc(from, to, 0)).toEqual(from);
    expect(pointOnArc(from, to, 1)).toEqual(to);
    const middle = pointOnArc(from, to, 0.5);
    expect(middle.x).toBeCloseTo(200);
    expect(middle.y).toBeCloseTo(360);
  });
});

describe("overlayFrame", () => {
  const center = rectCenter(request.target);

  it("flies from the cursor towards the target centre", () => {
    expect(center).toEqual({ x: 600, y: 300 });
    const start = overlayFrame(request, 0);
    expect(start.cursor).toEqual(request.from);
    expect(start.highlightOpacity).toBe(0);
    const mid = overlayFrame(request, 300);
    expect(mid.cursor.x).toBeGreaterThan(100);
    expect(mid.cursor.x).toBeLessThan(600);
    expect(mid.done).toBe(false);
  });

  it("holds on the target centre with the highlight on", () => {
    const frame = overlayFrame(request, 600 + 450);
    expect(frame).toEqual({ cursor: center, cursorOpacity: 1, highlightOpacity: 1, done: false });
  });

  it("fades out and then reports done", () => {
    const fading = overlayFrame(request, 600 + 900 + 150);
    expect(fading.cursorOpacity).toBeCloseTo(0.5);
    expect(fading.highlightOpacity).toBeCloseTo(0.5);
    expect(fading.done).toBe(false);
    const end = overlayFrame(request, 600 + 900 + 300);
    expect(end.cursorOpacity).toBe(0);
    expect(end.done).toBe(true);
  });

  it("copes with zero durations", () => {
    const instant = { ...request, flightMs: 0, holdMs: 0, fadeMs: 0 };
    expect(overlayFrame(instant, 5).done).toBe(true);
  });
});

describe("scaleMismatch", () => {
  it("flags a scale that disagrees with the page", () => {
    expect(scaleMismatch(1.5, 1.5)).toBe(false);
    expect(scaleMismatch(1.25, 1.25000001)).toBe(false);
    expect(scaleMismatch(1.5, 1)).toBe(true);
  });
});
