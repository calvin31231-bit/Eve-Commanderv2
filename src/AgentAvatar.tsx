import type { CSSProperties } from "react";

export type Mood = "calm" | "happy" | "warning" | "critical" | "idle";

const MOOD_COLOR: Record<Mood, string> = {
  calm: "var(--accent)",
  happy: "var(--safe)",
  warning: "var(--caution)",
  critical: "var(--danger)",
  idle: "var(--neutral)",
};

/// "Aura" — the companion AI avatar. An animated core whose colour, pulse and
/// expression convey the app's current mood (driven by alerts / connection).
export function AgentAvatar({ mood, size = 56 }: { mood: Mood; size?: number }) {
  const style = { ["--mood" as string]: MOOD_COLOR[mood], width: size, height: size } as CSSProperties;
  // Critical "narrows" the eye for an intent expression; happy lifts it.
  const ry = mood === "critical" ? 3.4 : mood === "happy" ? 5.5 : 6.4;
  return (
    <div className={`agent agent-${mood}`} style={style} aria-hidden="true">
      <svg viewBox="0 0 64 64">
        <circle className="agent-ring" cx="32" cy="32" r="27" />
        <circle className="agent-disc" cx="32" cy="32" r="21" />
        <g className="agent-face">
          <ellipse className="agent-eye" cx="32" cy="32" rx="11" ry={ry} />
          <circle className="agent-pupil" cx="32" cy="32" r="3.1" />
        </g>
      </svg>
    </div>
  );
}
