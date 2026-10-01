import { onBeforeUnmount, onMounted, shallowRef } from "vue";
import type { HistoryPoint, UsageData } from "../types/usage";

const STORAGE_KEY = "usageHistory";

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

  function pruneHistory(retentionDays: number) {
    const days = Number.isFinite(retentionDays) ? Math.max(7, retentionDays) : 30;
    const cutoff = Date.now() - days * 86_400_000;
    historyPoints.value = historyPoints.value.filter((point) => point.t > cutoff);
    localStorage.setItem(STORAGE_KEY, JSON.stringify(historyPoints.value));
  }

  function appendUsage(usage: UsageData, timestamp = Date.now()) {
    const retentionDays = Math.max(7, Number(localStorage.getItem("historyRetentionDays") ?? "30"));
    const retentionMs = retentionDays * 86_400_000;
    const next = [
      ...historyPoints.value,
      {
        t: timestamp,
        fiveHour: usage.five_hour?.remaining_percent,
        weekly: usage.weekly?.remaining_percent,
      },
    ].filter((point) => point.t > timestamp - retentionMs);
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

  return { historyPoints, reloadHistory, appendUsage, pruneHistory, clearHistory };
}
