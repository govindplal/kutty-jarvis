import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { overlayFrame, POINT_EVENT, scaleMismatch, type PointRequest } from "../lib/pointing";

/** Space between the element's edge and the highlight ring, in CSS pixels. */
const RING_GAP = 3;

/**
 * Full-screen, transparent, click-through layer. On each `overlay-point`
 * event it flies a cursor to the target, rings the target, then fades.
 * Drawing goes straight to the DOM from requestAnimationFrame, so React does
 * not re-render on every frame.
 */
export function Overlay() {
  const cursorRef = useRef<HTMLDivElement>(null);
  const ringRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let frame = 0;

    function draw(request: PointRequest, elapsedMs: number): boolean {
      const cursor = cursorRef.current;
      const ring = ringRef.current;
      if (!cursor || !ring) return true;
      const state = overlayFrame(request, elapsedMs);
      cursor.style.transform = `translate(${state.cursor.x}px, ${state.cursor.y}px)`;
      cursor.style.opacity = String(state.cursorOpacity);
      ring.style.opacity = String(state.highlightOpacity);
      return state.done;
    }

    function start(request: PointRequest) {
      cancelAnimationFrame(frame);
      const ring = ringRef.current;
      if (ring) {
        const { x, y, width, height } = request.target;
        ring.style.left = `${x - RING_GAP}px`;
        ring.style.top = `${y - RING_GAP}px`;
        ring.style.width = `${width + 2 * RING_GAP}px`;
        ring.style.height = `${height + 2 * RING_GAP}px`;
      }
      const mismatch = scaleMismatch(request.scale, window.devicePixelRatio);
      document.documentElement.dataset.scaleMismatch = String(mismatch);
      if (mismatch) {
        console.warn(
          `overlay scale ${request.scale} differs from devicePixelRatio ${window.devicePixelRatio}`,
        );
      }
      const startedAt = performance.now();
      const step = (now: number) => {
        if (!draw(request, now - startedAt)) frame = requestAnimationFrame(step);
      };
      draw(request, 0);
      frame = requestAnimationFrame(step);
    }

    const stop = listen<PointRequest>(POINT_EVENT, (event) => start(event.payload));
    return () => {
      cancelAnimationFrame(frame);
      stop.then((unlisten) => unlisten());
    };
  }, []);

  return (
    <div className="overlay" aria-hidden="true">
      <div ref={ringRef} className="overlay-ring" data-testid="overlay-ring" />
      <div ref={cursorRef} className="overlay-cursor" data-testid="overlay-cursor">
        <svg width="28" height="28" viewBox="0 0 28 28">
          {/* Tip at (0, 0) so the element's transform is the point it touches. */}
          <path
            d="M1 1 L1 22 L7 16.5 L11 26 L15 24.3 L11 15 L19 15 Z"
            fill="#7c5cff"
            stroke="#ffffff"
            strokeWidth="1.6"
            strokeLinejoin="round"
          />
        </svg>
      </div>
    </div>
  );
}
