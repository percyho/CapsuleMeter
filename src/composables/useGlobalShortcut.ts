import { onBeforeUnmount, readonly, shallowRef } from "vue";
import { register, unregister } from "@tauri-apps/plugin-global-shortcut";

interface UseGlobalShortcutOptions {
  defaultShortcut: string;
  onTriggered: () => void | Promise<void>;
  storageKey?: string;
  enabledStorageKey?: string;
}

function shortcutFromKeyboardEvent(event: KeyboardEvent): string | null {
  const key = event.key.length === 1 ? event.key.toUpperCase() : event.key;
  if (["Control", "Shift", "Alt", "Meta"].includes(key)) return null;
  if (!event.ctrlKey && !event.metaKey && !event.altKey) return null;

  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.metaKey) parts.push("Super");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  parts.push(key === " " ? "Space" : key);
  return parts.join("+");
}

export function useGlobalShortcut(options: UseGlobalShortcutOptions) {
  const storageKey = options.storageKey ?? "globalShortcut";
  const enabledStorageKey = options.enabledStorageKey ?? "shortcutEnabled";
  const storedShortcut = localStorage.getItem(storageKey)?.trim();
  const shortcut = shallowRef(storedShortcut || options.defaultShortcut);
  const enabled = shallowRef(localStorage.getItem(enabledStorageKey) !== "false");
  let registeredShortcut: string | null = null;

  async function unregisterCurrent() {
    if (!registeredShortcut) return;
    try {
      await unregister(registeredShortcut);
    } finally {
      registeredShortcut = null;
    }
  }

  async function registerCurrent() {
    await unregisterCurrent();
    if (!enabled.value) return;
    try {
      await unregister(shortcut.value);
    } catch {
      // 开发热更新时可能遗留旧注册，忽略未注册状态。
    }
    await register(shortcut.value, async (event) => {
      if (event.state === "Pressed") await options.onTriggered();
    });
    registeredShortcut = shortcut.value;
  }

  async function setEnabled(value: boolean) {
    const previous = enabled.value;
    enabled.value = value;
    try {
      await registerCurrent();
      localStorage.setItem(enabledStorageKey, String(value));
    } catch (error) {
      enabled.value = previous;
      await registerCurrent().catch(() => {});
      throw error;
    }
  }

  async function setShortcut(value: string) {
    const previous = shortcut.value;
    shortcut.value = value;
    try {
      await registerCurrent();
      localStorage.setItem(storageKey, value);
    } catch (error) {
      shortcut.value = previous;
      await registerCurrent().catch(() => {});
      throw error;
    }
  }

  async function capture(event: KeyboardEvent): Promise<string | null> {
    const value = shortcutFromKeyboardEvent(event);
    if (!value) return null;
    await setShortcut(value);
    return value;
  }

  async function reset() {
    shortcut.value = options.defaultShortcut;
    enabled.value = true;
    localStorage.removeItem(storageKey);
    localStorage.removeItem(enabledStorageKey);
    await registerCurrent();
  }

  onBeforeUnmount(() => {
    void unregisterCurrent();
  });

  return {
    shortcut: readonly(shortcut),
    enabled: readonly(enabled),
    capture,
    registerCurrent,
    reset,
    setEnabled,
  };
}
