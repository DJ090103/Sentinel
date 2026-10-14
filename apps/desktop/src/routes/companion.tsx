import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef } from "react";

import "@/companion/companion.css";
import { click, setHover, setLook, start, useAvatar } from "@/companion/engine";
import { Monitor } from "@/companion/Monitor";
import { NameTag } from "@/companion/NameTag";

const DRAG_PX = 4;

/**
 * The floating desktop companion, rendered in its own transparent window.
 *
 * Lives outside the app shell (no sidebar, no page transition). Click pokes it,
 * dragging moves the window, and the corner button turns it off.
 */
export function CompanionPage() {
  const { face, scale, look, pulse, name, avatar } = useAvatar();
  const press = useRef<{ x: number; y: number; dragging: boolean } | null>(null);

  useEffect(() => {
    document.documentElement.classList.add("companion");
    start();
    return () => document.documentElement.classList.remove("companion");
  }, []);

  const down = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    press.current = { x: e.screenX, y: e.screenY, dragging: false };
  };
  const move = (e: React.PointerEvent) => {
    const p = press.current;
    if (!p) {
      const r = e.currentTarget.getBoundingClientRect();
      const dx = (e.clientX - r.left) / r.width - 0.5;
      setLook(Math.abs(dx) < 0.1 ? 0 : Math.sign(dx), 0);
      return;
    }
    if (!p.dragging && Math.hypot(e.screenX - p.x, e.screenY - p.y) > DRAG_PX) {
      p.dragging = true;
      // The OS drags the window; we only learn where it ended up.
      void getCurrentWindow()
        .startDragging()
        .then(() => invoke("companion_moved"))
        .catch(() => undefined);
    }
  };
  const up = () => {
    const p = press.current;
    press.current = null;
    if (p && !p.dragging) click();
  };

  return (
    <div
      className="stage"
      onPointerDown={down}
      onPointerMove={move}
      onPointerUp={up}
      onPointerCancel={up}
      onPointerEnter={() => setHover(true)}
      onPointerLeave={() => setHover(false)}
    >
      <NameTag name={name} eyes={avatar.eyes} scale={scale} alpha={face.alpha} />
      <button
        className="x"
        aria-label="Turn off the companion"
        title="Turn off"
        onPointerDown={(e) => e.stopPropagation()}
        onClick={() => void invoke("hide_companion")}
      >
        ✕
      </button>
      <Monitor face={face} scale={scale} look={look} pulse={pulse} avatar={avatar} />
    </div>
  );
}
