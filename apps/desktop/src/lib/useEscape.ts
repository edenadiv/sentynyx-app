import { useEffect } from "react";

/** Close an overlay on Escape. Every full-screen scene should be dismissable
 *  from the keyboard — reaching for the mouse to leave a modal is friction. */
export function useEscape(onClose: () => void, active = true): void {
  useEffect(() => {
    if (!active) return;
    const h = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [onClose, active]);
}
