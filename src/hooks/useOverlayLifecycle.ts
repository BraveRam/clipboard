import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";

interface Options {
  onOpened: () => void;
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
  onClosed,
  hideOnBlur = true,
}: Options) {
  useEffect(() => {
    const win = getCurrentWindow();
    let unlistenOpened: undefined | (() => void);
    let unlistenFocus: undefined | (() => void);
    let unlistenBlur: undefined | (() => void);

    (async () => {
      unlistenOpened = await listen("overlay:opened", () => onOpened());

      unlistenFocus = await win.onFocusChanged(({ payload: focused }) => {
        if (focused) {
          onOpened();
        } else {
          onClosed?.();
          if (hideOnBlur) {
            win.hide().catch(() => {});
          }
        }
      });
      void unlistenBlur;
    })();

    return () => {
      unlistenOpened?.();
      unlistenFocus?.();
    };
  }, [onOpened, onClosed, hideOnBlur]);
}
