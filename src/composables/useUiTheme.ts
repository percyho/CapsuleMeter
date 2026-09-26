import { onBeforeUnmount, onMounted, shallowRef } from "vue";

export type UiTheme = "dark" | "light" | "system";

function readTheme(): UiTheme {
  const value = localStorage.getItem("uiTheme");
  return value === "dark" || value === "light" ? value : "system";
}

export function useUiTheme(options: { syncAcrossWindows?: boolean; onChange?: () => void } = {}) {
  const uiTheme = shallowRef<UiTheme>(readTheme());

  function applyTheme(theme = uiTheme.value) {
    uiTheme.value = theme;
    const resolved = theme === "system"
      ? (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
      : theme;
    document.documentElement.dataset.uiTheme = resolved;
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

  function onSystemThemeChange() {
    if (uiTheme.value === "system") {
      applyTheme("system");
      options.onChange?.();
    }
  }

  onMounted(() => {
    applyTheme();
    window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", onSystemThemeChange);
    if (options.syncAcrossWindows) window.addEventListener("storage", onStorage);
  });
  onBeforeUnmount(() => {
    window.removeEventListener("storage", onStorage);
    window.matchMedia("(prefers-color-scheme: dark)").removeEventListener("change", onSystemThemeChange);
  });

  return { uiTheme, applyTheme, setUiTheme };
}
