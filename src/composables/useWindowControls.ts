import { getCurrentWindow } from "@tauri-apps/api/window";

export function useWindowControls() {
  const appWindow = getCurrentWindow();
  return {
    minimizeWindow: () => appWindow.minimize(),
    toggleMaximizeWindow: () => appWindow.toggleMaximize(),
    closeWindow: () => appWindow.close(),
  };
}
