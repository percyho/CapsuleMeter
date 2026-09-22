import { onBeforeUnmount, onMounted, shallowRef } from "vue";

export type UiTheme = "dark" | "light";

function readTheme(): UiTheme {
  return localStorage.getItem("uiTheme") === "light" ? "light" : "dark";
}

export function useUiTheme(options: { syncAcrossWindows?: boolean; onChange?: () => void } = {}) {
  const uiTheme = shallowRef<UiTheme>(readTheme());

  function applyTheme(theme = uiTheme.value) {
    uiTheme.value = theme;
    document.documentElement.dataset.uiTheme = theme;
  }

  function setUiTheme(theme: UiTheme) {
    localStorage.setItem("uiTheme", theme);
    applyTheme(theme);
    options.onChange?.();
  }

  function onStorage(event: StorageEvent) {
    if (event.key !== "uiTheme") return;
    applyTheme(readTheme());
    options.onChange?.();
  }

  onMounted(() => {
    applyTheme();
    if (options.syncAcrossWindows) window.addEventListener("storage", onStorage);
  });
  onBeforeUnmount(() => window.removeEventListener("storage", onStorage));

  return { uiTheme, applyTheme, setUiTheme };
}
