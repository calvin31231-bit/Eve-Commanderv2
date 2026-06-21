import { useEffect, useRef } from "react";

interface Star {
  x: number;
  y: number;
  r: number;
  base: number;
  amp: number;
  phase: number;
  speed: number;
  color: string;
}

/// A subtle, GPU-cheap twinkling starfield painted behind the whole app.
/// Density scales with viewport; honours prefers-reduced-motion (static).
export function Starfield() {
  const ref = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    let stars: Star[] = [];
    let raf = 0;

    function build() {
      const w = window.innerWidth;
      const h = window.innerHeight;
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      canvas!.width = w * dpr;
      canvas!.height = h * dpr;
      canvas!.style.width = `${w}px`;
      canvas!.style.height = `${h}px`;
      ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);

      const count = Math.min(Math.round((w * h) / 9000), 240);
      stars = Array.from({ length: count }, () => {
        const tint = Math.random();
        const color =
          tint < 0.82 ? "214,228,255" : tint < 0.92 ? "120,200,255" : "190,168,255";
        return {
          x: Math.random() * w,
          y: Math.random() * h,
          r: Math.random() * 1.2 + 0.3,
          base: Math.random() * 0.45 + 0.12,
          amp: Math.random() * 0.4 + 0.1,
          phase: Math.random() * Math.PI * 2,
          speed: Math.random() * 1.4 + 0.4,
          color,
        };
      });
    }

    function paint(now: number) {
      const w = window.innerWidth;
      const h = window.innerHeight;
      ctx!.clearRect(0, 0, w, h);
      for (const s of stars) {
        const tw = reduce ? s.base + s.amp * 0.5 : s.base + s.amp * Math.sin(now / 1000 * s.speed + s.phase);
        ctx!.globalAlpha = Math.max(0, Math.min(1, tw));
        ctx!.fillStyle = `rgb(${s.color})`;
        ctx!.beginPath();
        ctx!.arc(s.x, s.y, s.r, 0, Math.PI * 2);
        ctx!.fill();
      }
      ctx!.globalAlpha = 1;
      if (!reduce) raf = requestAnimationFrame(paint);
    }

    function onResize() {
      build();
      if (reduce) paint(0);
    }

    build();
    if (reduce) {
      paint(0);
    } else {
      raf = requestAnimationFrame(paint);
    }
    window.addEventListener("resize", onResize);
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", onResize);
    };
  }, []);

  return <canvas ref={ref} className="starfield" aria-hidden="true" />;
}
