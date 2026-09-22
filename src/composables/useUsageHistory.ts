import { onBeforeUnmount, onMounted, shallowRef } from "vue";
import type { HistoryPoint, UsageData } from "../types/usage";

const STORAGE_KEY = "usageHistory";
const RETENTION_MS = 30 * 86_400_000;

export function readUsageHistory(): HistoryPoint[] {
  try {
    const value = JSON.parse(localStorage.getItem(STORAGE_KEY) || "[]");
    return Array.isArray(value) ? value : [];
  } catch {
    return [];
  }
}

export function useUsageHistory(options: { syncAcrossWindows?: boolean; onChange?: () => void } = {}) {
  const historyPoints = shallowRef<HistoryPoint[]>(readUsageHistory());

  function reloadHistory() {
    historyPoints.value = readUsageHistory();
  }

  function appendUsage(usage: UsageData, timestamp = Date.now()) {
    const next = [
      ...historyPoints.value,
      {
        t: timestamp,
        fiveHour: usage.five_hour?.remaining_percent,
        weekly: usage.weekly?.remaining_percent,
      },
    ].filter((point) => point.t > timestamp - RETENTION_MS);
    historyPoints.value = next;
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  }

  function clearHistory() {
    historyPoints.value = [];
    localStorage.removeItem(STORAGE_KEY);
  }

  function onStorage(event: StorageEvent) {
    if (event.key !== STORAGE_KEY) return;
    reloadHistory();
    options.onChange?.();
  }

  onMounted(() => {
    if (options.syncAcrossWindows) window.addEventListener("storage", onStorage);
  });
  onBeforeUnmount(() => window.removeEventListener("storage", onStorage));

  return { historyPoints, reloadHistory, appendUsage, clearHistory };
}
