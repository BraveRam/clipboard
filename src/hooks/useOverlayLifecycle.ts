import { useEffect, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";

interface Options {
  onOpened: () => void;
  onFocus?: () => void;
  onClosed?: () => void;
  hideOnBlur?: boolean;
}

/**
 * Wires window focus/blur and the `overlay:opened` event so the UI can
 * reset selection, focus the input on show, and (optionally) hide when
 * the overlay loses focus.
 */
export function useOverlayLifecycle({
  onOpened,
  onFocus,
  onClosed,
  hideOnBlur = true,
}: Options) {
  const openedAtRef = useRef<number>(0);

  useEffect(() => {
    const win = getCurrentWindow();
    let unlistenOpened: undefined | (() => void);
    let unlistenFocus: undefined | (() => void);

    (async () => {
      unlistenOpened = await listen("overlay:opened", () => {
        openedAtRef.current = Date.now();
        onOpened();
      });

      unlistenFocus = await win.onFocusChanged(({ payload: focused }) => {
        if (focused) {
          if (onFocus) {
            onFocus();
          } else {
            onOpened();
          }
        } else {
          onClosed?.();
          const timeSinceOpen = Date.now() - openedAtRef.current;
          // During window presentation and compositor seat transfer, focus can briefly
          // be false. Ignore blur events during the initial 600ms grace period.
          if (hideOnBlur && timeSinceOpen > 600) {
            win.hide().catch(() => {});
          }
        }
      });
    })();

    const handleWindowFocus = () => {
      if (onFocus) {
        onFocus();
      } else {
        onOpened();
      }
    };
    window.addEventListener("focus", handleWindowFocus);

    return () => {
      window.removeEventListener("focus", handleWindowFocus);
      unlistenOpened?.();
      unlistenFocus?.();
    };
  }, [onOpened, onFocus, onClosed, hideOnBlur]);
}
