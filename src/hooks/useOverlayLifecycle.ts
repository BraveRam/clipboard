import { useEffect, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { subscriptionScope } from "../lib/subscriptions";
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
    const scope = subscriptionScope();

    (async () => {
      await scope.add(() => listen("overlay:opened", () => {
        if (!scope.active) return;
        openedAtRef.current = Date.now();
        onOpened();
      }));

      await scope.add(() => win.onFocusChanged(({ payload: focused }) => {
        if (!scope.active) return;
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
      }));
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
      scope.dispose();
    };
  }, [onOpened, onFocus, onClosed, hideOnBlur]);
}
