<script setup lang="ts">
import { ref, reactive, onMounted, onBeforeUnmount, computed, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  getCurrentWindow,
  LogicalSize,
  LogicalPosition,
} from "@tauri-apps/api/window";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import {
  enable as autostartEnable,
  disable as autostartDisable,
  isEnabled as autostartIsEnabled,
} from "@tauri-apps/plugin-autostart";
import StatsPanel from "./components/statistics/StatsPanel.vue";
import ResetCreditsCard from "./components/usage/ResetCreditsCard.vue";
import CapsuleThemePanel from "./components/appearance/CapsuleThemePanel.vue";
import { useUiTheme } from "./composables/useUiTheme";
import { useLocale } from "./composables/useLocale";
import { useUsageHistory } from "./composables/useUsageHistory";
import { useGlobalShortcut } from "./composables/useGlobalShortcut";
import type {
  ConsumeResetResult,
  ResetCreditsSummary,
  UsageData,
  WindowData,
} from "./types/usage";
import {
  capsuleThemeOptions,
  parseCapsuleTheme,
  type CapsuleTheme,
} from "./types/capsule-theme";
import {
  evaluateConsumptionSpeed,
  type ConsumptionSpeedState,
} from "./utils/consumption-speed";

// —— 整窗按住拖动（移动超过阈值才启动拖动，单击仍触发点击） ——
const DRAG_THRESHOLD = 4;
const CAPSULE_WIDTH = 140;
const CAPSULE_HEIGHT = 34;
let dragTracking: { sx: number; sy: number; started: boolean } | null = null;

function startWindowDrag() {
  try {
    getCurrentWindow().startDragging();
  } catch {
    // 非 Tauri 环境忽略
  }
}

function cleanupDrag() {
  window.removeEventListener("mousemove", onDragMove);
  window.removeEventListener("mouseup", onDragUp);
  dragTracking = null;
}

function onDragMove(e: MouseEvent) {
  if (!dragTracking || dragTracking.started) return;
  if (
    Math.hypot(e.clientX - dragTracking.sx, e.clientY - dragTracking.sy) >
    DRAG_THRESHOLD
  ) {
    dragTracking.started = true;
    startWindowDrag();
    cleanupDrag();
  }
}

function onDragUp() {
  cleanupDrag();
}

function onCapsuleMouseDown(e: MouseEvent) {
  if (e.button !== 0) return;
  dragTracking = { sx: e.clientX, sy: e.clientY, started: false };
  window.addEventListener("mousemove", onDragMove);
  window.addEventListener("mouseup", onDragUp);
}

const usage = ref<UsageData | null>(null);
const loading = ref(false);
const lastError = ref<string | null>(null);
const lastRefresh = ref("--");
const resetCredits = ref<ResetCreditsSummary | null>(null);
const resetCreditsLoading = ref(false);
const resetCreditsError = ref<string | null>(null);
let pendingResetKey: string | null = null;

// 当前左侧/右侧是否正在显示重置时间
const showLeftReset = ref(false);
const showRightReset = ref(false);
let leftTimer: number | undefined;
let rightTimer: number | undefined;

// —— 可配置项（localStorage 持久化）——
const opacity = ref(Number(localStorage.getItem("opacity") ?? "0.72"));
const fontSize = ref(Number(localStorage.getItem("fontSize") ?? "13"));
type CapsuleColorValues = {
  background: string;
  leftFill: string;
  rightFill: string;
};
type CapsuleThemeColorOverrides = Partial<
  Record<CapsuleTheme, CapsuleColorValues>
>;

const storedCapsuleStyle = localStorage.getItem("capsuleStyle");
const capsuleStyle = ref<CapsuleTheme>(parseCapsuleTheme(storedCapsuleStyle));

function normalizeStoredColor(value: unknown): string {
  return typeof value === "string" && /^#[\da-f]{6}$/i.test(value)
    ? value
    : "";
}

function readCapsuleThemeColors(): CapsuleThemeColorOverrides {
  try {
    const stored = JSON.parse(
      localStorage.getItem("capsuleThemeColors") ?? "{}",
    ) as Record<string, Partial<CapsuleColorValues>>;
    return Object.fromEntries(
      capsuleThemeOptions.map(({ id }) => {
        const colors = stored[id] ?? {};
        return [
          id,
          {
            background: normalizeStoredColor(colors.background),
            leftFill: normalizeStoredColor(colors.leftFill),
            rightFill: normalizeStoredColor(colors.rightFill),
          },
        ];
      }),
    ) as CapsuleThemeColorOverrides;
  } catch {
    return {};
  }
}

const capsuleThemeColors = reactive<CapsuleThemeColorOverrides>(
  readCapsuleThemeColors(),
);
const legacyCapsuleColors = {
  background: normalizeStoredColor(localStorage.getItem("capsuleBackgroundColor")),
  leftFill: normalizeStoredColor(localStorage.getItem("capsuleLeftFillColor")),
  rightFill: normalizeStoredColor(localStorage.getItem("capsuleRightFillColor")),
};
if (Object.values(legacyCapsuleColors).some(Boolean)) {
  const currentThemeColors = capsuleThemeColors[capsuleStyle.value] ?? {
    background: "",
    leftFill: "",
    rightFill: "",
  };
  capsuleThemeColors[capsuleStyle.value] = {
    background: currentThemeColors.background || legacyCapsuleColors.background,
    leftFill: currentThemeColors.leftFill || legacyCapsuleColors.leftFill,
    rightFill: currentThemeColors.rightFill || legacyCapsuleColors.rightFill,
  };
  localStorage.setItem("capsuleThemeColors", JSON.stringify(capsuleThemeColors));
  ["capsuleBackgroundColor", "capsuleLeftFillColor", "capsuleRightFillColor"]
    .forEach((key) => localStorage.removeItem(key));
}

const capsuleColors = reactive<CapsuleColorValues>({
  ...(capsuleThemeColors[capsuleStyle.value] ?? {
    background: "",
    leftFill: "",
    rightFill: "",
  }),
});
const capsuleBackgroundStyle = computed(() =>
  capsuleColors.background
    ? { backgroundColor: capsuleColors.background, backgroundImage: "none" }
    : {},
);
const leftFillColorStyle = computed(() =>
  capsuleColors.leftFill
    ? { backgroundColor: capsuleColors.leftFill, backgroundImage: "none" }
    : {},
);
const rightFillColorStyle = computed(() =>
  capsuleColors.rightFill
    ? { backgroundColor: capsuleColors.rightFill, backgroundImage: "none" }
    : {},
);
const { uiTheme, applyTheme, setUiTheme } = useUiTheme();
const { locale, setLocale, t } = useLocale();
const activeTab = ref("appearance");
const { historyPoints, appendUsage } = useUsageHistory({
  syncAcrossWindows: true,
});
function recordHistory() {
  if (!usage.value || usage.value.error) return;
  appendUsage(usage.value);
}
async function onQuit() {
  await invoke("quit_app");
}

const alwaysOnTop = ref(localStorage.getItem("alwaysOnTop") !== "false");
const snapEnabled = ref(localStorage.getItem("snapEnabled") !== "false");
const displayMode = ref(localStorage.getItem("displayMode") ?? "remaining"); // remaining | used
const showUsageValues = ref(
  localStorage.getItem("showUsageValues") !== "false",
);
const refreshMin = ref(Number(localStorage.getItem("refreshMin") ?? "5"));
const resetShowSec = ref(Number(localStorage.getItem("resetShowSec") ?? "5"));
const historyRetentionDays = ref(
  Number(localStorage.getItem("historyRetentionDays") ?? "30"),
);
const notificationsEnabled = ref(
  localStorage.getItem("notificationsEnabled") === "true",
);
const notificationThreshold = ref(
  Number(localStorage.getItem("notificationThreshold") ?? "20"),
);
const capsuleVisible = ref(localStorage.getItem("capsuleVisible") !== "false");
type TrayIconMode = "logo" | "usage";
const trayIconMode = ref<TrayIconMode>(
  localStorage.getItem("trayIconMode") === "usage" ? "usage" : "logo",
);
const systemMessage = ref("");
const autostart = ref(false);
let unlistenEvents: UnlistenFn[] = [];

const {
  shortcut: globalShortcut,
  enabled: shortcutEnabled,
  capture: captureGlobalShortcut,
  registerCurrent: applyGlobalShortcut,
  reset: resetGlobalShortcut,
  setEnabled: setShortcutEnabled,
} = useGlobalShortcut({
  defaultShortcut: "Ctrl+Shift+U",
  onTriggered: async () => {
    const win = getCurrentWindow();
    if (await win.isVisible()) {
      capsuleVisible.value = false;
      localStorage.setItem("capsuleVisible", "false");
      await win.hide();
    } else {
      capsuleVisible.value = true;
      localStorage.setItem("capsuleVisible", "true");
      await win.show();
      await win.setFocus();
    }
  },
});

// —— 显示值（支持剩余/已用模式切换）——
function pickPct(w: WindowData | undefined): number | null {
  if (!w) return null;
  const v = displayMode.value === "used" ? w.used_percent : w.remaining_percent;
  return v === null || v === undefined ? null : v;
}

const leftValue = computed(() => {
  const v = pickPct(usage.value?.five_hour);
  return v === null ? "--" : String(Math.round(v));
});

const rightValue = computed(() => {
  const v = pickPct(usage.value?.weekly);
  return v === null ? "--" : String(Math.round(v));
});

const speedColors: Record<ConsumptionSpeedState, string> = {
  unknown: "#94a3b8",
  idle: "#55d6a8",
  steady: "#55a7ff",
  fast: "#f5a524",
  critical: "#ff5d68",
};
const speedLabels: Record<ConsumptionSpeedState, string> = {
  unknown: "速度评估中",
  idle: "消耗平缓",
  steady: "消耗正常",
  fast: "消耗偏快",
  critical: "消耗过快",
};

const fiveHourSpeed = computed(() =>
  evaluateConsumptionSpeed(historyPoints.value, "fiveHour"),
);
const weeklySpeed = computed(() =>
  evaluateConsumptionSpeed(historyPoints.value, "weekly"),
);
const speedIndicatorStyle = computed(() => ({
  "--speed-left": speedColors[fiveHourSpeed.value.state],
  "--speed-right": speedColors[weeklySpeed.value.state],
}));

function formatSpeedHint(label: string, percentPerHour: number | null): string {
  return percentPerHour === null
    ? label
    : `${label} · ${percentPerHour.toFixed(1)}%/小时`;
}

/** 把重置时间戳格式化为准确时间：当天显示 HH:MM，跨天显示 M/D HH:MM */
function fmtClock(epochSec: number | null): string {
  if (!epochSec) return "--";
  const d = new Date(epochSec * 1000);
  const now = new Date();
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  const sameDay =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate();
  if (sameDay) return `${hh}:${mm}`;
  return `${d.getMonth() + 1}/${d.getDate()}`;
}

function fmtDateTime(epochSec: number): string {
  return new Intl.DateTimeFormat(locale.value, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: locale.value === "en-US",
  }).format(new Date(epochSec * 1000));
}

/** 5 小时窗口始终显示具体重置时刻，跨天也不退化为日期。 */
function fmtTimeOnly(epochSec: number | null): string {
  if (!epochSec) return "--";
  return new Date(epochSec * 1000).toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}

// 左侧点击：显示 5 小时重置时间，N 秒后恢复
function onLeftClick() {
  if (usage.value?.error) return;
  showLeftReset.value = !showLeftReset.value;
  clearTimer("left");
  if (showLeftReset.value) {
    leftTimer = window.setTimeout(() => {
      showLeftReset.value = false;
    }, resetShowSec.value * 1000);
  }
}

// 右侧点击：显示周重置时间，N 秒后恢复
function onRightClick() {
  if (usage.value?.error) return;
  showRightReset.value = !showRightReset.value;
  clearTimer("right");
  if (showRightReset.value) {
    rightTimer = window.setTimeout(() => {
      showRightReset.value = false;
    }, resetShowSec.value * 1000);
  }
}

function clearTimer(which: "left" | "right") {
  if (which === "left" && leftTimer) {
    window.clearTimeout(leftTimer);
    leftTimer = undefined;
  }
  if (which === "right" && rightTimer) {
    window.clearTimeout(rightTimer);
    rightTimer = undefined;
  }
}

const leftDisplay = computed(() => {
  if (usage.value?.error) return "ERR";
  const w = usage.value?.five_hour;
  if (!w) return "--";
  if (showLeftReset.value) return fmtTimeOnly(w.reset_at);
  return leftValue.value;
});

const rightDisplay = computed(() => {
  if (usage.value?.error) return "ERR";
  const w = usage.value?.weekly;
  if (!w) return "--";
  if (showRightReset.value) return fmtClock(w.reset_at);
  return rightValue.value;
});

const leftTitle = computed(() => {
  const w = usage.value?.five_hour;
  const reset = w?.reset_at ? `5h 重置于 ${fmtClock(w.reset_at)}` : "5h 用量";
  return `${reset} · ${formatSpeedHint(speedLabels[fiveHourSpeed.value.state], fiveHourSpeed.value.percentPerHour)}`;
});
const rightTitle = computed(() => {
  const w = usage.value?.weekly;
  const reset = w?.reset_at
    ? `${t("周重置于")} ${fmtDateTime(w.reset_at)}`
    : "周用量";
  return `${reset} · ${formatSpeedHint(speedLabels[weeklySpeed.value.state], weeklySpeed.value.percentPerHour)}`;
});

// 剩余量填充宽度：按当前显示模式取对应百分比
const leftFillWidth = computed(() => {
  const v = pickPct(usage.value?.five_hour);
  return v === null ? "0%" : `${Math.max(0, Math.min(100, v))}%`;
});
const rightFillWidth = computed(() => {
  const v = pickPct(usage.value?.weekly);
  return v === null ? "0%" : `${Math.max(0, Math.min(100, v))}%`;
});

// —— 数据刷新 ——
async function refresh() {
  if (loading.value) return;
  loading.value = true;
  try {
    const data = await invoke<UsageData>("fetch_usage");
    usage.value = data;
    lastError.value = data.error ?? null;
    await updateTrayIcon(data);
    if (!data.error) await maybeNotifyLowUsage(data);
  } catch (e) {
    lastError.value = String(e);
    usage.value = null;
  } finally {
    loading.value = false;
  }
  lastRefresh.value = new Date().toLocaleTimeString("zh-CN", { hour12: false });
  recordHistory();
  void refreshResetCredits();
}

async function refreshResetCredits() {
  if (resetCreditsLoading.value) return;
  resetCreditsLoading.value = true;
  resetCreditsError.value = null;
  try {
    resetCredits.value = await invoke<ResetCreditsSummary>(
      "fetch_reset_credits",
    );
  } catch (reason) {
    resetCreditsError.value = String(reason);
  } finally {
    resetCreditsLoading.value = false;
  }
}

async function consumeResetCredit(creditId: string | null) {
  if (resetCreditsLoading.value) return;
  resetCreditsLoading.value = true;
  resetCreditsError.value = null;
  pendingResetKey ??= crypto.randomUUID();
  try {
    const result = await invoke<ConsumeResetResult>("consume_reset_credit", {
      idempotencyKey: pendingResetKey,
      creditId,
    });
    if (result.outcome === "reset" || result.outcome === "alreadyRedeemed") {
      pendingResetKey = null;
      systemMessage.value = "限额已重置，用量数据已更新";
    } else if (result.outcome === "nothingToReset") {
      pendingResetKey = null;
      systemMessage.value = "当前限额尚未达到可重置条件";
    } else {
      pendingResetKey = null;
      systemMessage.value = "当前没有可用的限额重置机会";
    }
  } catch (reason) {
    resetCreditsError.value = `${String(reason)}；再次确认将安全重试本次操作`;
  } finally {
    resetCreditsLoading.value = false;
  }
  await refresh();
}

let interval: number | undefined;
function restartInterval() {
  if (interval) window.clearInterval(interval);
  interval = window.setInterval(refresh, refreshMin.value * 60 * 1000);
}
watch(refreshMin, (value) => {
  localStorage.setItem("refreshMin", String(value));
  restartInterval();
});

// —— 外观 ——
function applyWindowSettings() {
  document.body.style.opacity = String(opacity.value);
  document.documentElement.style.setProperty(
    "--num-size",
    fontSize.value + "px",
  );
  applyTheme();
}

async function updateTrayIcon(data = usage.value) {
  if (!data) return;
  try {
    await invoke("update_tray_icon", {
      mode: trayIconMode.value,
      fiveHour: data.five_hour.remaining_percent,
      weekly: data.weekly.remaining_percent,
      fiveHourResetAfterSeconds: data.five_hour.reset_after_seconds,
      weeklyResetAfterSeconds: data.weekly.reset_after_seconds,
    });
  } catch (reason) {
    systemMessage.value = String(reason);
  }
}

function onTrayIconModeChange(mode: TrayIconMode) {
  trayIconMode.value = mode;
  localStorage.setItem("trayIconMode", mode);
  void updateTrayIcon();
}

async function maybeNotifyLowUsage(data: UsageData) {
  if (!notificationsEnabled.value) return;
  const remaining = data.five_hour.remaining_percent;
  if (remaining == null || remaining > notificationThreshold.value) return;
  const cycle = String(data.five_hour.reset_at ?? "unknown");
  if (localStorage.getItem("lastNotifiedCycle") === cycle) return;
  let allowed = await isPermissionGranted();
  if (!allowed) allowed = (await requestPermission()) === "granted";
  if (!allowed) return;
  sendNotification({
    title: "Codex 用量提醒",
    body: `5 小时额度仅剩 ${Math.round(remaining)}%`,
  });
  localStorage.setItem("lastNotifiedCycle", cycle);
}

async function onShortcutChange() {
  const nextEnabled = !shortcutEnabled.value;
  try {
    await setShortcutEnabled(nextEnabled);
    systemMessage.value = nextEnabled
      ? `快捷键已启用：${globalShortcut.value}`
      : "快捷键已停用";
  } catch (reason) {
    systemMessage.value = `快捷键注册失败：${String(reason)}`;
  }
}

async function onShortcutKeydown(event: KeyboardEvent) {
  event.preventDefault();
  event.stopPropagation();
  try {
    const value = await captureGlobalShortcut(event);
    systemMessage.value = value
      ? `快捷键已更新：${value}`
      : "请按下 Ctrl、Alt 或 Meta 与其他按键的组合";
  } catch (reason) {
    systemMessage.value = `快捷键注册失败：${String(reason)}`;
  }
}

async function onNotificationChange() {
  if (notificationsEnabled.value) {
    let allowed = await isPermissionGranted();
    if (!allowed) allowed = (await requestPermission()) === "granted";
    if (!allowed) {
      notificationsEnabled.value = false;
      systemMessage.value = "系统通知权限未授予";
    }
  }
  localStorage.setItem(
    "notificationsEnabled",
    String(notificationsEnabled.value),
  );
}
function onNotificationThresholdChange() {
  localStorage.setItem(
    "notificationThreshold",
    String(notificationThreshold.value),
  );
}
function onRetentionChange() {
  localStorage.setItem(
    "historyRetentionDays",
    String(historyRetentionDays.value),
  );
}

async function startLogin() {
  systemMessage.value = "已打开 Codex 登录窗口，登录完成后请点击立即刷新";
  try {
    await invoke("start_codex_login");
  } catch (reason) {
    systemMessage.value = String(reason);
  }
}

async function runDiagnostics() {
  systemMessage.value = "正在诊断…";
  try {
    systemMessage.value =
      (await invoke<string>("diagnose_codex")) || "诊断完成，未发现异常";
  } catch (reason) {
    systemMessage.value = `诊断失败：${String(reason)}`;
  }
}

interface UpdateInfo {
  current: string;
  latest: string | null;
  url: string | null;
  error: string | null;
}
async function checkForUpdates() {
  systemMessage.value = "正在检查更新…";
  const info = await invoke<UpdateInfo>("check_update");
  systemMessage.value = info.error
    ? `检查失败：${info.error}`
    : info.latest && info.latest.replace(/^v/, "") !== info.current
      ? `发现新版本 ${info.latest}：${info.url ?? "请访问项目主页"}`
      : `当前已是最新版本 v${info.current}`;
}

function onTabKeydown(event: KeyboardEvent) {
  if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
  const tabs = [
    "appearance",
    "capsule",
    "behavior",
    "stats",
    "reset",
    "system",
  ];
  const current = tabs.indexOf(activeTab.value);
  const next =
    (current + (event.key === "ArrowRight" ? 1 : -1) + tabs.length) %
    tabs.length;
  activeTab.value = tabs[next];
  (event.currentTarget as HTMLElement)
    .querySelectorAll<HTMLButtonElement>("[role=tab]")
    [next]?.focus();
  event.preventDefault();
}

function onOpacityChange() {
  localStorage.setItem("opacity", String(opacity.value));
  applyWindowSettings();
}
function onFontSizeChange() {
  localStorage.setItem("fontSize", String(fontSize.value));
  applyWindowSettings();
}

function onCapsuleColorChange(
  color: keyof typeof capsuleColors,
  event: Event,
) {
  if (!(event.target instanceof HTMLInputElement)) return;
  capsuleColors[color] = event.target.value;
  capsuleThemeColors[capsuleStyle.value] = { ...capsuleColors };
  localStorage.setItem("capsuleThemeColors", JSON.stringify(capsuleThemeColors));
}

function resetCapsuleColors() {
  capsuleColors.background = "";
  capsuleColors.leftFill = "";
  capsuleColors.rightFill = "";
  delete capsuleThemeColors[capsuleStyle.value];
  localStorage.setItem("capsuleThemeColors", JSON.stringify(capsuleThemeColors));
  ["capsuleBackgroundColor", "capsuleLeftFillColor", "capsuleRightFillColor"]
    .forEach((key) => localStorage.removeItem(key));
}

function setCapsuleStyle(style: CapsuleTheme) {
  capsuleStyle.value = style;
  localStorage.setItem("capsuleStyle", style);
  const colors = capsuleThemeColors[style];
  capsuleColors.background = colors?.background ?? "";
  capsuleColors.leftFill = colors?.leftFill ?? "";
  capsuleColors.rightFill = colors?.rightFill ?? "";
}

// —— 行为 ——
async function applyAlwaysOnTop() {
  try {
    await getCurrentWindow().setAlwaysOnTop(alwaysOnTop.value);
  } catch {}
}
function onAlwaysOnTopChange() {
  localStorage.setItem("alwaysOnTop", String(alwaysOnTop.value));
  applyAlwaysOnTop();
}

async function applySnap() {
  try {
    await invoke("set_snap_enabled", { enabled: snapEnabled.value });
  } catch {}
}
function onSnapChange() {
  localStorage.setItem("snapEnabled", String(snapEnabled.value));
  applySnap();
}

async function setCapsuleVisibility(visible: boolean) {
  capsuleVisible.value = visible;
  localStorage.setItem("capsuleVisible", String(visible));
  const win = getCurrentWindow();
  if (visible) {
    await win.show();
    await win.setFocus();
  } else {
    await win.hide();
  }
}

function onCapsuleVisibilityChange() {
  void setCapsuleVisibility(!capsuleVisible.value);
}

function onDisplayModeChange() {
  localStorage.setItem("displayMode", displayMode.value);
}

function onShowUsageValuesChange() {
  showUsageValues.value = !showUsageValues.value;
  localStorage.setItem("showUsageValues", String(showUsageValues.value));
}

function onResetShowSecChange() {
  localStorage.setItem("resetShowSec", String(resetShowSec.value));
  // 若正在显示重置时间，立即用新时长重新计时
  if (showLeftReset.value) {
    clearTimer("left");
    leftTimer = window.setTimeout(() => {
      showLeftReset.value = false;
    }, resetShowSec.value * 1000);
  }
  if (showRightReset.value) {
    clearTimer("right");
    rightTimer = window.setTimeout(() => {
      showRightReset.value = false;
    }, resetShowSec.value * 1000);
  }
}

// —— 系统：开机自启 ——
async function initAutostart() {
  try {
    autostart.value = await autostartIsEnabled();
  } catch {
    autostart.value = localStorage.getItem("autostart") === "true";
  }
}
async function onAutostartChange() {
  try {
    if (autostart.value) {
      await autostartEnable();
    } else {
      await autostartDisable();
    }
    localStorage.setItem("autostart", String(autostart.value));
  } catch {
    // 操作失败时回滚开关状态
    autostart.value = !autostart.value;
  }
}

// —— 面板 ——
const showPanel = ref(false);
const PANEL_DEFAULT_WIDTH = 360;
const PANEL_DEFAULT_HEIGHT = 380;
const PANEL_MIN_WIDTH = 320;
const PANEL_MIN_HEIGHT = 280;
const storedPanelW = Number(
  localStorage.getItem("panelW") ?? PANEL_DEFAULT_WIDTH,
);
const storedPanelH = Number(
  localStorage.getItem("panelH") ?? PANEL_DEFAULT_HEIGHT,
);
const panelW = ref(
  Math.max(
    PANEL_MIN_WIDTH,
    Number.isFinite(storedPanelW) ? storedPanelW : PANEL_DEFAULT_WIDTH,
  ),
);
const panelH = ref(
  Math.max(
    PANEL_MIN_HEIGHT,
    Number.isFinite(storedPanelH) ? storedPanelH : PANEL_DEFAULT_HEIGHT,
  ),
);
let panelResize: {
  sx: number;
  sy: number;
  x: number;
  y: number;
  w: number;
  h: number;
} | null = null;

function onPanelDragStart(event: MouseEvent) {
  if (event.button !== 0 || (event.target as HTMLElement).closest("button"))
    return;
  startWindowDrag();
}

function onPanelResizeStart(e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  panelResize = {
    sx: e.screenX,
    sy: e.screenY,
    x: window.screenX,
    y: window.screenY,
    w: panelW.value,
    h: panelH.value,
  };
  window.addEventListener("mousemove", onPanelResizeMove);
  window.addEventListener("mouseup", onPanelResizeEnd);
}
function onPanelResizeMove(e: MouseEvent) {
  if (!panelResize) return;
  const dx = e.screenX - panelResize.sx;
  const dy = e.screenY - panelResize.sy;
  panelW.value = Math.max(PANEL_MIN_WIDTH, panelResize.w - dx);
  panelH.value = Math.max(PANEL_MIN_HEIGHT, panelResize.h + dy);
  const appliedDx = panelResize.w - panelW.value;
  const win = getCurrentWindow();
  void win.setSize(
    new LogicalSize(panelW.value, CAPSULE_HEIGHT + panelH.value),
  );
  void win.setPosition(
    new LogicalPosition(panelResize.x + appliedDx, panelResize.y),
  );
}
function onPanelResizeEnd() {
  localStorage.setItem("panelW", String(panelW.value));
  localStorage.setItem("panelH", String(panelH.value));
  window.removeEventListener("mousemove", onPanelResizeMove);
  window.removeEventListener("mouseup", onPanelResizeEnd);
  panelResize = null;
}

const panelAlignRight = ref(false);
let panelGeometryChanging = false;

async function togglePanel() {
  if (panelGeometryChanging) return;
  panelGeometryChanging = true;
  const win = getCurrentWindow() as any;
  try {
    if (!showPanel.value) {
      // 用 screenX 判断（Tauri webview 中 screenX = 窗口在屏幕上的 x）
      const winX = window.screenX;
      const winY = window.screenY;
      const shouldAlignRight = winX + panelW.value > window.screen.availWidth;
      panelAlignRight.value = shouldAlignRight;
      const targetX = shouldAlignRight
        ? winX - (panelW.value - CAPSULE_WIDTH)
        : winX;
      await Promise.all([
        win.setSize(new LogicalSize(panelW.value, CAPSULE_HEIGHT + panelH.value)),
        win.setPosition(new LogicalPosition(targetX, winY)),
      ]);
      showPanel.value = true;
    } else {
      const winX = window.screenX;
      const winY = window.screenY;
      const targetX = panelAlignRight.value
        ? winX + (panelW.value - CAPSULE_WIDTH)
        : winX;
      await Promise.all([
        win.setSize(new LogicalSize(CAPSULE_WIDTH, CAPSULE_HEIGHT)),
        win.setPosition(new LogicalPosition(targetX, winY)),
      ]);
      showPanel.value = false;
      panelAlignRight.value = false;
    }
  } finally {
    panelGeometryChanging = false;
  }
}

function onContextMenu(e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  togglePanel();
}

// —— 恢复默认 ——
function resetDefaults() {
  opacity.value = 0.72;
  fontSize.value = 13;
  capsuleColors.background = "";
  capsuleColors.leftFill = "";
  capsuleColors.rightFill = "";
  capsuleThemeOptions.forEach(({ id }) => delete capsuleThemeColors[id]);
  capsuleStyle.value = "flat";
  uiTheme.value = "dark";
  setLocale("zh-CN");
  alwaysOnTop.value = true;
  snapEnabled.value = true;
  displayMode.value = "remaining";
  showUsageValues.value = true;
  refreshMin.value = 5;
  resetShowSec.value = 5;
  historyRetentionDays.value = 30;
  notificationsEnabled.value = false;
  notificationThreshold.value = 20;
  capsuleVisible.value = true;
  trayIconMode.value = "logo";
  panelW.value = PANEL_DEFAULT_WIDTH;
  panelH.value = PANEL_DEFAULT_HEIGHT;
  [
    "opacity",
    "fontSize",
    "capsuleThemeColors",
    "capsuleBackgroundColor",
    "capsuleLeftFillColor",
    "capsuleRightFillColor",
    "capsuleStyle",
    "uiTheme",
    "appLocale",
    "alwaysOnTop",
    "snapEnabled",
    "displayMode",
    "showUsageValues",
    "refreshMin",
    "resetShowSec",
    "historyRetentionDays",
    "notificationsEnabled",
    "notificationThreshold",
    "capsuleVisible",
    "trayIconMode",
    "panelW",
    "panelH",
  ].forEach((k) => localStorage.removeItem(k));
  applyWindowSettings();
  applyAlwaysOnTop();
  applySnap();
  restartInterval();
  void updateTrayIcon();
  void setCapsuleVisibility(true);
  void resetGlobalShortcut().catch((reason) => {
    systemMessage.value = `快捷键恢复失败：${String(reason)}`;
  });
  if (autostart.value) {
    autostart.value = false;
    onAutostartChange();
  }
  getCurrentWindow().setSize(
    new LogicalSize(PANEL_DEFAULT_WIDTH, CAPSULE_HEIGHT + PANEL_DEFAULT_HEIGHT),
  );
}

onMounted(() => {
  applyWindowSettings();
  applyAlwaysOnTop();
  applySnap();
  initAutostart();
  refresh();
  restartInterval();
  void applyGlobalShortcut().catch((reason) => {
    systemMessage.value = `快捷键注册失败：${String(reason)}`;
  });
  if (!capsuleVisible.value) void getCurrentWindow().hide();
  void listen("tray-refresh", () => void refresh()).then((unlisten) =>
    unlistenEvents.push(unlisten),
  );
  void listen("tray-show", () => {
    capsuleVisible.value = true;
    localStorage.setItem("capsuleVisible", "true");
  }).then((unlisten) => unlistenEvents.push(unlisten));
  void listen("tray-open-settings", () => {
    capsuleVisible.value = true;
    localStorage.setItem("capsuleVisible", "true");
    if (!showPanel.value) void togglePanel();
  }).then((unlisten) => unlistenEvents.push(unlisten));
  void listen<TrayIconMode>("tray-icon-mode-change", ({ payload }) => {
    onTrayIconModeChange(payload);
  }).then((unlisten) => unlistenEvents.push(unlisten));
  void getCurrentWindow()
    .onFocusChanged(({ payload }) => {
      if (payload && lastError.value?.includes("登录已过期")) void refresh();
    })
    .then((unlisten) => unlistenEvents.push(unlisten));
});

onBeforeUnmount(() => {
  if (interval) window.clearInterval(interval);
  clearTimer("left");
  clearTimer("right");
  unlistenEvents.forEach((unlisten) => unlisten());
});
</script>

<template>
  <div
    class="capsule-shell"
    :class="{ 'capsule-shell-right': panelAlignRight }"
    :style="speedIndicatorStyle"
    @mousedown="onCapsuleMouseDown"
    @contextmenu="onContextMenu"
  >
    <div
      class="capsule"
      :class="[{ error: lastError }, `capsule-${capsuleStyle}`]"
    >
      <div
        class="half left"
        :style="capsuleBackgroundStyle"
        @click="onLeftClick"
        :title="leftTitle"
      >
        <div
          class="fill"
          :style="{ width: leftFillWidth, ...leftFillColorStyle }"
        ></div>
        <span
          v-show="showUsageValues || showLeftReset"
          class="num"
          :class="{ dim: showLeftReset }"
          >{{ leftDisplay }}</span
        >
      </div>
      <div
        class="half right"
        :style="capsuleBackgroundStyle"
        @click="onRightClick"
        :title="rightTitle"
      >
        <div
          class="fill fill--red"
          :style="{ width: rightFillWidth, ...rightFillColorStyle }"
        ></div>
        <span
          v-show="showUsageValues || showRightReset"
          class="num"
          :class="{ dim: showRightReset }"
          >{{ rightDisplay }}</span
        >
      </div>
    </div>
    <div class="speed-indicators" aria-hidden="true">
      <span class="speed-indicator speed-indicator--left"></span>
      <span class="speed-indicator speed-indicator--right"></span>
    </div>
  </div>

  <div
    v-if="showPanel"
    class="panel"
    :class="{ 'align-right': panelAlignRight }"
    :style="{ width: panelW + 'px', height: panelH + 'px' }"
    @contextmenu.prevent
  >
    <div class="panel-head" @mousedown="onPanelDragStart">
      <div class="panel-heading">
        <span class="panel-title">{{ t("设置") }}</span>
        <span class="panel-description">{{ t("个性化用量胶囊") }}</span>
      </div>
      <div class="panel-account">
        <span class="panel-sub">{{
          lastError ? t("数据异常") : (usage?.plan || t("未登录")).toUpperCase()
        }}</span>
        <span
          v-if="usage?.account && !lastError"
          class="panel-account-name"
          :title="usage.account"
          >{{ usage.account }}</span
        >
      </div>
      <button
        class="panel-close"
        :title="t('关闭设置')"
        :aria-label="t('关闭设置')"
        @click="togglePanel()"
      >
        ×
      </button>
    </div>

    <div class="settings-shell">
      <div
        class="tabs"
        role="tablist"
        :aria-label="t('设置分类')"
        @keydown="onTabKeydown"
      >
        <button
          class="tab"
          :class="{ on: activeTab === 'appearance' }"
          role="tab"
          :aria-selected="activeTab === 'appearance'"
          @click="activeTab = 'appearance'"
        >
          <svg
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <circle cx="12" cy="12" r="9" />
            <path d="M12 3v18M3 12h18" /></svg
          ><span>{{ t("外观") }}</span>
        </button>
        <button
          class="tab"
          :class="{ on: activeTab === 'capsule' }"
          role="tab"
          :aria-selected="activeTab === 'capsule'"
          @click="activeTab = 'capsule'"
        >
          <svg
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <rect x="3" y="7" width="18" height="10" rx="5" />
            <path d="M12 7v10" /></svg
          ><span>{{ t("胶囊主题") }}</span>
        </button>
        <button
          class="tab"
          :class="{ on: activeTab === 'behavior' }"
          role="tab"
          :aria-selected="activeTab === 'behavior'"
          @click="activeTab = 'behavior'"
        >
          <svg
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <circle cx="12" cy="12" r="3" />
            <path
              d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H2.8v-4H3a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1A1.7 1.7 0 0 0 9 4.6 1.7 1.7 0 0 0 10 3V2.8h4V3a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1Z"
            /></svg
          ><span>{{ t("行为") }}</span>
        </button>
        <button
          class="tab"
          :class="{ on: activeTab === 'stats' }"
          role="tab"
          :aria-selected="activeTab === 'stats'"
          @click="activeTab = 'stats'"
        >
          <svg
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <path d="M4 20V10M10 20V4M16 20v-7M22 20H2" /></svg
          ><span>{{ t("统计") }}</span>
        </button>
        <button
          class="tab"
          :class="{ on: activeTab === 'reset' }"
          role="tab"
          :aria-selected="activeTab === 'reset'"
          @click="activeTab = 'reset'"
        >
          <svg
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <path d="M20 7h-7a5 5 0 1 0 4.6 7" />
            <path d="m17 3 3 4-3 4" /></svg
          ><span>{{ t("重置") }}</span>
        </button>
        <button
          class="tab"
          :class="{ on: activeTab === 'system' }"
          role="tab"
          :aria-selected="activeTab === 'system'"
          @click="activeTab = 'system'"
        >
          <svg
            aria-hidden="true"
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <circle cx="12" cy="12" r="9" />
            <path d="M12 11v5M12 8h.01" /></svg
          ><span>{{ t("系统") }}</span>
        </button>
      </div>

      <div class="panel-body">
        <template v-if="activeTab === 'appearance'">
          <div class="row">
            <span class="row-name">{{ t("界面主题") }}</span>
            <div class="seg" aria-label="界面主题">
              <button
                :class="{ on: uiTheme === 'light' }"
                @click="setUiTheme('light')"
              >
                {{ t("明亮") }}
              </button>
              <button
                :class="{ on: uiTheme === 'dark' }"
                @click="setUiTheme('dark')"
              >
                {{ t("暗黑") }}
              </button>
              <button
                :class="{ on: uiTheme === 'system' }"
                @click="setUiTheme('system')"
              >
                {{ t("跟随系统") }}
              </button>
            </div>
          </div>
          <div class="row">
            <span class="row-name">{{ t("语言") }}</span>
            <div class="seg" aria-label="Language">
              <button
                :class="{ on: locale === 'zh-CN' }"
                @click="setLocale('zh-CN')"
              >
                中文
              </button>
              <button
                :class="{ on: locale === 'en-US' }"
                @click="setLocale('en-US')"
              >
                English
              </button>
            </div>
          </div>
          <div class="row">
            <span class="row-name">{{ t("透明度") }}</span>
            <input
              type="range"
              min="0.3"
              max="1"
              step="0.05"
              v-model.number="opacity"
              @input="onOpacityChange"
            />
            <span class="row-val">{{ Math.round(opacity * 100) }}%</span>
          </div>
          <div class="row">
            <span class="row-name">{{ t("字号") }}</span>
            <input
              type="range"
              min="9"
              max="20"
              step="1"
              v-model.number="fontSize"
              @input="onFontSizeChange"
            />
            <span class="row-val">{{ fontSize }}px</span>
          </div>
          <div class="row">
            <span class="row-name">{{ t("显示用量数值") }}</span>
            <button
              class="switch"
              :class="{ on: showUsageValues }"
              @click="onShowUsageValuesChange()"
            >
              <span class="knob"></span>
            </button>
          </div>
          <div class="group-label color-group-heading">
            <span>{{ t("胶囊颜色") }}</span>
            <button class="color-reset" @click="resetCapsuleColors">
              {{ t("恢复主题默认颜色") }}
            </button>
          </div>
          <label class="row color-row" for="capsule-background-color">
            <span class="row-name">{{ t("胶囊底色") }}</span>
            <span class="color-control">
              <input
                id="capsule-background-color"
                type="color"
                :value="capsuleColors.background || '#30323d'"
                @input="onCapsuleColorChange('background', $event)"
              />
              <span class="color-value">{{ capsuleColors.background || t("主题默认") }}</span>
            </span>
          </label>
          <label class="row color-row" for="capsule-left-fill-color">
            <span class="row-name">{{ t("左侧填充色") }}</span>
            <span class="color-control">
              <input
                id="capsule-left-fill-color"
                type="color"
                :value="capsuleColors.leftFill || '#4f70ef'"
                @input="onCapsuleColorChange('leftFill', $event)"
              />
              <span class="color-value">{{ capsuleColors.leftFill || t("主题默认") }}</span>
            </span>
          </label>
          <label class="row color-row" for="capsule-right-fill-color">
            <span class="row-name">{{ t("右侧填充色") }}</span>
            <span class="color-control">
              <input
                id="capsule-right-fill-color"
                type="color"
                :value="capsuleColors.rightFill || '#e54870'"
                @input="onCapsuleColorChange('rightFill', $event)"
              />
              <span class="color-value">{{ capsuleColors.rightFill || t("主题默认") }}</span>
            </span>
          </label>
        </template>

        <template v-if="activeTab === 'capsule'">
          <CapsuleThemePanel
            :model-value="capsuleStyle"
            :theme-colors="capsuleThemeColors"
            :left-value="leftDisplay"
            :right-value="rightDisplay"
            :translate="t"
            @select="setCapsuleStyle"
          />
        </template>

        <template v-if="activeTab === 'behavior'">
          <div class="group-label">{{ t("行为") }}</div>
          <div class="row">
            <span class="row-name">{{ t("窗口置顶") }}</span>
            <button
              class="switch"
              :class="{ on: alwaysOnTop }"
              @click="
                alwaysOnTop = !alwaysOnTop;
                onAlwaysOnTopChange();
              "
            >
              <span class="knob"></span>
            </button>
          </div>
          <div class="row">
            <span class="row-name">{{ t("贴边吸附") }}</span>
            <button
              class="switch"
              :class="{ on: snapEnabled }"
              @click="
                snapEnabled = !snapEnabled;
                onSnapChange();
              "
            >
              <span class="knob"></span>
            </button>
          </div>
          <div class="row">
            <span class="row-name">{{ t("显示模式") }}</span>
            <div class="seg">
              <button
                :class="{ on: displayMode === 'remaining' }"
                @click="
                  displayMode = 'remaining';
                  onDisplayModeChange();
                "
              >
                {{ t("剩余") }}
              </button>
              <button
                :class="{ on: displayMode === 'used' }"
                @click="
                  displayMode = 'used';
                  onDisplayModeChange();
                "
              >
                {{ t("已用") }}
              </button>
            </div>
          </div>
          <div class="row">
            <span class="row-name">{{ t("重置显示时长") }}</span>
            <select
              v-model.number="resetShowSec"
              @change="onResetShowSecChange()"
            >
              <option
                v-for="seconds in [3, 5, 8, 10]"
                :key="seconds"
                :value="seconds"
              >
                {{ locale === "en-US" ? `${seconds} sec` : `${seconds} 秒` }}
              </option>
            </select>
          </div>

          <div class="group-label">{{ t("数据") }}</div>
          <div class="row">
            <span class="row-name">{{ t("自动刷新") }}</span>
            <select v-model.number="refreshMin">
              <option
                v-for="minutes in [1, 2, 5, 10, 30]"
                :key="minutes"
                :value="minutes"
              >
                {{ locale === "en-US" ? `${minutes} min` : `${minutes} 分钟` }}
              </option>
            </select>
          </div>
          <div class="row">
            <span class="row-name">{{ t("上次刷新") }}</span>
            <span class="row-val">{{ lastRefresh }}</span>
            <button class="btn-mini" :disabled="loading" @click="refresh()">
              {{ loading ? t("刷新中…") : t("立即刷新") }}
            </button>
          </div>
          <div class="row">
            <span class="row-name">{{ t("历史保留") }}</span>
            <select
              v-model.number="historyRetentionDays"
              @change="onRetentionChange"
            >
              <option
                v-for="days in [7, 30, 90, 365]"
                :key="days"
                :value="days"
              >
                {{
                  days === 365
                    ? locale === "en-US"
                      ? "1 year"
                      : "1 年"
                    : locale === "en-US"
                      ? `${days} days`
                      : `${days} 天`
                }}
              </option>
            </select>
          </div>
        </template>

        <template v-if="activeTab === 'stats'">
          <StatsPanel
            :history-points="historyPoints"
            :ui-theme="uiTheme"
            @open-history="invoke('open_history')"
          />
        </template>

        <template v-if="activeTab === 'reset'">
          <div class="group-label">{{ t("限额重置") }}</div>
          <ResetCreditsCard
            :summary="resetCredits"
            :loading="resetCreditsLoading"
            :error="resetCreditsError"
            @refresh="refreshResetCredits"
            @consume="consumeResetCredit"
          />
        </template>

        <template v-if="activeTab === 'system'">
          <div class="group-label">{{ t("系统") }}</div>
          <div class="row row-flat">
            <span class="row-name">{{ t("显示胶囊") }}</span>
            <button
              class="switch"
              :class="{ on: capsuleVisible }"
              @click="onCapsuleVisibilityChange()"
            >
              <span class="knob"></span>
            </button>
          </div>
          <div class="row row-flat">
            <span class="row-name">{{ t("开机自启") }}</span>
            <button
              class="switch"
              :class="{ on: autostart }"
              @click="
                autostart = !autostart;
                onAutostartChange();
              "
            >
              <span class="knob"></span>
            </button>
          </div>
          <div class="row row-flat">
            <span class="row-name">{{ t("托盘图标") }}</span>
            <div class="seg" aria-label="托盘图标样式">
              <button
                :class="{ on: trayIconMode === 'logo' }"
                @click="onTrayIconModeChange('logo')"
              >
                Logo
              </button>
              <button
                :class="{ on: trayIconMode === 'usage' }"
                @click="onTrayIconModeChange('usage')"
              >
                {{ t("用量环") }}
              </button>
            </div>
          </div>
          <div class="row row-flat">
            <span class="row-name">{{ t("低额度通知") }}</span>
            <button
              class="switch"
              :class="{ on: notificationsEnabled }"
              @click="
                notificationsEnabled = !notificationsEnabled;
                onNotificationChange();
              "
            >
              <span class="knob"></span>
            </button>
          </div>
          <div v-if="notificationsEnabled" class="row row-flat">
            <span class="row-name">{{ t("提醒阈值") }}</span>
            <select
              v-model.number="notificationThreshold"
              @change="onNotificationThresholdChange"
            >
              <option :value="10">10%</option>
              <option :value="20">20%</option>
              <option :value="30">30%</option>
            </select>
          </div>
          <div class="row row-flat">
            <span class="row-name">{{ t("全局快捷键") }}</span>
            <input
              class="shortcut-input"
              :value="globalShortcut"
              :aria-label="t('全局快捷键')"
              readonly
              @keydown="onShortcutKeydown"
            />
            <button
              class="switch"
              :class="{ on: shortcutEnabled }"
              @click="onShortcutChange()"
            >
              <span class="knob"></span>
            </button>
          </div>

          <div class="group-label">{{ t("诊断与更新") }}</div>
          <div class="row row-flat system-actions">
            <button class="btn-mini" @click="startLogin">
              {{ t("重新登录") }}
            </button>
            <button class="btn-mini" @click="runDiagnostics">
              {{ t("连接诊断") }}
            </button>
            <button class="btn-mini" @click="checkForUpdates">
              {{ t("检查更新") }}
            </button>
          </div>
          <p v-if="systemMessage" class="system-message">{{ systemMessage }}</p>

          <div class="group-label">{{ t("关于") }}</div>
          <div class="row row-flat">
            <span class="row-name">{{ t("版本") }}</span
            ><span class="row-val">v1.0.0</span>
          </div>
          <div class="row row-flat privacy-row">
            <div class="privacy-heading">
              <span class="row-name">{{ t("隐私") }}</span>
              <span class="privacy-badge">{{ t("仅本地处理") }}</span>
            </div>
            <p class="privacy-summary">
              {{
                t(
                  "Codex Capsule 不提供账号系统，也不会收集、出售或同步你的使用数据。",
                )
              }}
            </p>
            <ul class="privacy-list">
              <li
                v-for="item in [
                  [
                    '认证信息',
                    '仅在本机读取 Codex 登录凭据；访问令牌不会显示在界面或写入历史记录。',
                  ],
                  [
                    '网络请求',
                    '仅用于向 ChatGPT 官方接口获取额度与用量统计，不会发送给第三方服务。',
                  ],
                  [
                    '本地数据',
                    '偏好设置和额度采样保存在本机，可通过“恢复默认”或“清除历史”删除。',
                  ],
                  [
                    'CSV 导出',
                    '只有你主动选择保存位置时才会生成文件，应用不会自动上传导出内容。',
                  ],
                  ['遥测', '应用不包含广告、用户追踪或后台遥测。'],
                ]"
                :key="item[0]"
              >
                <strong>{{ t(item[0]) }}</strong
                ><span>{{ t(item[1]) }}</span>
              </li>
            </ul>
          </div>
          <div class="row row-flat">
            <span class="row-name">{{ t("许可证") }}</span
            ><span class="row-val">MIT</span>
          </div>
        </template>
      </div>
    </div>

    <div class="panel-foot">
      <button class="btn-mini btn-secondary" @click="resetDefaults()">
        {{ t("恢复默认") }}
      </button>
      <button class="btn-mini btn-danger" @click="onQuit()">
        {{ t("退出应用") }}
      </button>
    </div>

    <div class="resize-handle" @mousedown="onPanelResizeStart"></div>
  </div>
</template>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
  user-select: none;
}

html,
body,
#app {
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: transparent;
}

body {
  font-family: "Segoe UI", "Microsoft YaHei", system-ui, sans-serif;
  -webkit-font-smoothing: antialiased;
}

.capsule {
  position: relative;
  display: flex;
  align-items: stretch;
  width: 100px;
  height: 34px;
  flex-shrink: 0;
  gap: 3px;
  padding: 1px;
  background: transparent;
  border: 0;
  border-radius: 999px;
  box-shadow: none;
  backdrop-filter: none;
  isolation: isolate;
  overflow: hidden;
  cursor: default;
  transform-origin: center;
  transition:
    border-color 0.2s ease,
    box-shadow 0.2s ease;
}

.capsule-shell {
  position: relative;
  display: grid;
  place-items: center;
  width: 140px;
  height: 34px;
}

.capsule::before {
  content: "";
  position: absolute;
  z-index: 4;
  inset: 1px 8px auto;
  height: 1px;
  background: linear-gradient(
    90deg,
    transparent,
    rgba(255, 255, 255, 0.38),
    transparent
  );
  pointer-events: none;
  display: none;
}

.capsule::after {
  content: "";
  position: absolute;
  z-index: 4;
  top: 6px;
  bottom: 6px;
  left: 50%;
  width: 1px;
  background: linear-gradient(
    180deg,
    transparent,
    rgba(255, 255, 255, 0.3),
    transparent
  );
  box-shadow: 1px 0 0 rgba(0, 0, 0, 0.18);
  pointer-events: none;
  display: none;
}

.half {
  position: relative;
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 0;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 999px;
  background: linear-gradient(
    180deg,
    rgba(49, 51, 63, 0.98),
    rgba(24, 25, 32, 0.98)
  );
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.2),
    0 2px 7px rgba(0, 0, 0, 0.28);
  transition: background-color 0.18s ease;
}

/* 填充层：按当前显示模式从中间分隔线向外填充 */
.fill {
  position: absolute;
  right: 0;
  top: 0;
  bottom: 0;
  width: 0%;
  background: linear-gradient(
    90deg,
    rgba(79, 70, 229, 0.94),
    rgba(139, 92, 246, 0.96)
  );
  box-shadow: inset -1px 0 0 rgba(255, 255, 255, 0.15);
  transition: width 0.45s cubic-bezier(0.22, 1, 0.36, 1);
}

.fill--red {
  left: 0;
  right: auto;
  background: linear-gradient(
    90deg,
    rgba(255, 138, 101, 0.96),
    rgba(225, 29, 72, 0.95)
  );
  box-shadow: inset 1px 0 0 rgba(255, 255, 255, 0.13);
}

/* 单一外壳；主题只改变左右进度填充。 */
.capsule {
  gap: 0;
  padding: 0;
  background: transparent;
  border: 1px solid rgba(255, 255, 255, 0.2);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.22),
    inset 0 -1px 0 rgba(0, 0, 0, 0.24),
    0 3px 10px rgba(0, 0, 0, 0.3);
}

.capsule::before,
.capsule::after {
  display: block;
}
.capsule .half {
  border: 0;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}

.capsule-pixel {
  overflow: hidden;
  border: 0;
  border-radius: 0;
  padding: 4px 4px 5px;
  background: #090b0e;
  clip-path: polygon(
    8% 0,
    92% 0,
    92% 12%,
    96% 12%,
    96% 24%,
    100% 24%,
    100% 76%,
    96% 76%,
    96% 88%,
    92% 88%,
    92% 100%,
    8% 100%,
    8% 88%,
    4% 88%,
    4% 76%,
    0 76%,
    0 24%,
    4% 24%,
    4% 12%,
    8% 12%
  );
  image-rendering: pixelated;
  filter: drop-shadow(0 3px 0 rgba(0, 0, 0, 0.52));
  box-shadow: none;
}

.capsule-pixel::before {
  display: none;
}

.capsule-pixel::after {
  display: none;
}

.capsule-pixel .half {
  background: linear-gradient(
    180deg,
    #a6afbc 0,
    #7e8998 22%,
    #606a78 58%,
    #48515e 100%
  );
}
.capsule-pixel .left {
  flex: 52;
  clip-path: polygon(
    6% 0,
    100% 0,
    100% 100%,
    6% 100%,
    6% 88%,
    0 88%,
    0 12%,
    6% 12%
  );
}

.capsule-pixel .right {
  flex: 48;
  clip-path: polygon(
    0 0,
    94% 0,
    94% 12%,
    100% 12%,
    100% 88%,
    94% 88%,
    94% 100%,
    0 100%
  );
}

.capsule-pixel .fill {
  background-color: #287cf5;
  background-image: linear-gradient(
    180deg,
    #83c4ff 0,
    #4b97f8 22%,
    #2f7df4 62%,
    #255ed8 100%
  );
  box-shadow:
    inset 0 2px #b9e4ff,
    inset 0 -3px rgba(11, 48, 139, 0.52);
}

.capsule-pixel .fill--red {
  background-color: #20252e;
  background-image: linear-gradient(
    180deg,
    #596170 0,
    #3d4552 24%,
    #292f3a 58%,
    #171b22 100%
  );
  box-shadow:
    inset 0 2px rgba(255, 255, 255, 0.22),
    inset 0 -3px rgba(0, 0, 0, 0.48);
}

.capsule-pixel .num {
  font-family: Tiny5, Arial, "Microsoft YaHei", sans-serif;
  font-size: calc(var(--num-size, 13px) * 1.08);
  font-weight: 400;
  letter-spacing: 0;
  text-shadow: 1px 1px 0 rgba(14, 20, 33, 0.72);
}

.capsule-flat {
  border: 0;
  background: transparent;
  box-shadow: none;
}

.capsule-flat .fill {
  background: #3787f7;
  box-shadow: none;
}
.capsule-flat .fill--red {
  background: #111318;
  box-shadow: none;
}
.capsule-flat::before,
.capsule-flat::after {
  display: none;
}
.capsule-flat .num {
  font-family: Manrope, "Microsoft YaHei", sans-serif;
  font-weight: 700;
  letter-spacing: 0;
  text-shadow: none;
}

.capsule-skeuomorphic {
  border-color: rgba(149, 164, 188, 0.82);
  background: transparent;
  box-shadow:
    inset 0 2px 3px rgba(255, 255, 255, 0.48),
    inset 0 -3px 5px rgba(0, 0, 0, 0.52);
}

.capsule-skeuomorphic .fill {
  background: linear-gradient(180deg, #b9efff, #2964f1 62%, #3420c8);
  box-shadow: inset 0 2px 2px rgba(255, 255, 255, 0.46);
}
.capsule-skeuomorphic .fill--red {
  background: linear-gradient(180deg, #596175, #171b28);
}
.capsule-skeuomorphic .num {
  font-family: Sora, "Microsoft YaHei", sans-serif;
  font-weight: 700;
  letter-spacing: -0.3px;
  text-shadow: 0 2px 2px rgba(0, 0, 0, 0.6);
}

.capsule-neon {
  border: 2px solid transparent;
  background:
    linear-gradient(transparent, transparent) padding-box,
    linear-gradient(100deg, #42dfff 0%, #985cff 50%, #ff62c7 100%) border-box;
  box-shadow: inset 0 0 8px rgba(19, 28, 85, 0.7);
}

.capsule-neon::after {
  top: 0;
  bottom: 0;
  left: 50%;
  background: #f06bff;
  box-shadow: 0 0 5px #e053ff;
}

.capsule-neon .fill {
  background: linear-gradient(90deg, #073d72, #302070);
  box-shadow: inset 0 0 8px rgba(53, 220, 255, 0.33);
}
.capsule-neon .fill--red {
  background: linear-gradient(90deg, #38196d, #711452);
  box-shadow: inset 0 0 8px rgba(255, 79, 203, 0.33);
}
.capsule-neon .num {
  color: #fff;
  font-family: Oxanium, "Microsoft YaHei", sans-serif;
  font-weight: 600;
  letter-spacing: 0.35px;
  text-shadow:
    0 0 4px #86eaff,
    0 0 8px #945cff;
}

.speed-indicators {
  position: absolute;
  z-index: 12;
  inset: 0;
  pointer-events: none;
}

.speed-indicator {
  position: absolute;
  top: 50%;
  width: 12px;
  height: 12px;
  border: 1px solid rgba(8, 11, 16, 0.86);
  border-radius: 50%;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.56);
  transform: translateY(-50%);
}

.speed-indicator--left {
  left: 0;
  background: var(--speed-left);
}
.speed-indicator--right {
  right: 0;
  background: var(--speed-right);
}

.capsule-flat + .speed-indicators {
  left: 20px;
  right: 20px;
}

.capsule-flat + .speed-indicators .speed-indicator {
  display: none;
}

.capsule-flat + .speed-indicators::before,
.capsule-flat + .speed-indicators::after {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 50%;
  border: 2px solid;
  content: "";
  pointer-events: none;
}

.capsule-flat + .speed-indicators::before {
  left: 0;
  border-color: var(--speed-left);
  border-right: 0;
  border-radius: 999px 0 0 999px;
}

.capsule-flat + .speed-indicators::after {
  right: 0;
  border-color: var(--speed-right);
  border-left: 0;
  border-radius: 0 999px 999px 0;
}

.capsule-pixel .speed-indicator {
  border-radius: 50%;
  box-shadow:
    0 0 0 1px #090b0e,
    0 1px 1px rgba(0, 0, 0, 0.5);
}

.num {
  position: absolute;
  z-index: 5;
  inset: 0;
  display: grid;
  place-items: center;
  width: 100%;
  box-sizing: border-box;
  font-size: var(--num-size, 13px);
  font-weight: 750;
  color: #f8fafc;
  letter-spacing: -0.2px;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.4);
  font-variant-numeric: tabular-nums;
  line-height: 1;
  text-align: center;
  pointer-events: none;
  transition:
    transform 0.18s ease,
    opacity 0.15s ease;
}

@media (prefers-reduced-motion: reduce) {
  .capsule,
  .fill,
  .half,
  .num {
    animation: none;
    transition-duration: 0.01ms;
  }
}

.num.dim {
  font-size: calc(var(--num-size, 13px) * 0.78);
  font-weight: 600;
  color: rgba(248, 250, 252, 0.92);
  opacity: 1;
  white-space: nowrap;
  padding: 0 3px;
}

/* —— 设置面板 —— */
.panel {
  --panel-control-bg: rgba(255, 255, 255, 0.06);
  --panel-control-active: rgba(255, 255, 255, 0.1);
  position: fixed;
  top: 34px;
  left: 0;
  z-index: 10;
  display: flex;
  flex-direction: column;
  background: var(--panel-bg, rgba(24, 24, 24, 0.98));
  border: 1px solid var(--panel-border, rgba(255, 255, 255, 0.1));
  border-radius: 12px;
  margin-top: 6px;
  box-shadow:
    0 16px 40px rgba(0, 0, 0, 0.42),
    inset 0 1px 0 rgba(255, 255, 255, 0.04);
  backdrop-filter: blur(20px);
  overflow: hidden;
}

.panel-head {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 56px;
  padding: 11px 13px 10px 14px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  flex-shrink: 0;
  cursor: grab;
  user-select: none;
}

.panel-head:active {
  cursor: grabbing;
}

.panel-heading {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.panel-title {
  font-size: 15px;
  font-weight: 700;
  color: var(--panel-title, #f4f6fb);
  letter-spacing: 0.2px;
}

.panel-description {
  color: var(--panel-val, #7f8799);
  font-size: 11px;
  white-space: nowrap;
}

.panel-sub {
  flex: 1;
  font-size: 10px;
  color: var(--panel-accent, #7d98f5);
  text-align: right;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.panel-close {
  width: 26px;
  height: 26px;
  line-height: 24px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: #9aa1b5;
  font-size: 17px;
  cursor: pointer;
  padding: 0;
  flex-shrink: 0;
}

.panel-close:hover {
  background: rgba(255, 255, 255, 0.12);
  color: #fff;
}

.settings-shell {
  display: flex;
  flex: 1;
  min-height: 0;
}

.panel-account {
  display: grid;
  flex: 1;
  min-width: 0;
  justify-items: end;
  gap: 2px;
}

.panel-account .panel-sub {
  width: 100%;
}

.panel-account-name {
  overflow: hidden;
  max-width: 150px;
  color: var(--panel-val, #7f8799);
  font-size: 10px;
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
  user-select: text;
}

.tabs {
  display: flex;
  flex: 0 0 92px;
  flex-direction: column;
  gap: 2px;
  padding: 10px 8px;
  border-right: 1px solid rgba(255, 255, 255, 0.07);
}

.tab {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: 8px;
  min-width: 0;
  padding: 8px 9px;
  border: none;
  background: transparent;
  color: var(--panel-val, #7f8799);
  font-size: 12px;
  cursor: pointer;
  border-radius: 6px;
  transition:
    color 0.15s ease,
    background 0.15s ease;
}

.tab svg {
  flex-shrink: 0;
}

.tab:hover {
  background: rgba(255, 255, 255, 0.055);
}

.tab.on {
  background: var(--panel-bg-active, rgba(255, 255, 255, 0.09));
  color: var(--panel-text, #fff);
  font-weight: 600;
}

.panel-body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 12px 14px 14px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.panel-body::-webkit-scrollbar {
  width: 5px;
}

.panel-body::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.18);
  border-radius: 3px;
}

.group-label {
  font-size: 10px;
  font-weight: 700;
  color: var(--panel-val, #8f8f8f);
  letter-spacing: 0.08em;
  margin-top: 7px;
  padding: 7px 2px 2px;
  text-transform: uppercase;
}

.group-label:first-child {
  margin-top: 2px;
  border-top: none;
}

.row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 42px;
  padding: 8px 2px;
  font-size: 13px;
  color: #d8dce6;
  background: transparent;
  border: 0;
  border-bottom: 1px solid rgba(255, 255, 255, 0.055);
  border-radius: 0;
}

.row.row-flat {
  padding-inline: 2px;
  background: transparent;
  border-color: transparent;
}

.row-name {
  flex: 1 1 auto;
  color: var(--panel-text, #cfd4df);
  line-height: 1.35;
}

.row-val {
  margin-left: auto;
  font-size: 11px;
  color: var(--panel-val, #8f97a8);
  min-width: 30px;
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.row input[type="range"] {
  flex: 1;
  height: 4px;
  accent-color: #6b8af0;
  min-width: 0;
}

.color-group-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.color-reset {
  padding: 2px 0;
  border: 0;
  background: transparent;
  color: var(--panel-accent, #7d98f5);
  font: inherit;
  font-size: 10px;
  cursor: pointer;
}

.color-reset:hover {
  color: var(--panel-title, #f4f6fb);
}

.color-row {
  cursor: pointer;
}

.color-control {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-left: auto;
}

.color-control input[type="color"] {
  width: 28px;
  height: 24px;
  padding: 2px;
  border: 1px solid var(--panel-border, rgba(255, 255, 255, 0.14));
  border-radius: 5px;
  background: var(--panel-control-bg, rgba(255, 255, 255, 0.06));
  cursor: pointer;
}

.color-control input[type="color"]::-webkit-color-swatch-wrapper {
  padding: 1px;
}

.color-control input[type="color"]::-webkit-color-swatch {
  border: 0;
  border-radius: 3px;
}

.color-value {
  min-width: 76px;
  color: var(--panel-val, #8f97a8);
  font: 10px/1.2 ui-monospace, Consolas, monospace;
  text-align: right;
}

/* 开关 */
.switch {
  width: 34px;
  height: 20px;
  margin-left: auto;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.14);
  border: 1px solid rgba(255, 255, 255, 0.18);
  position: relative;
  cursor: pointer;
  padding: 0;
  flex-shrink: 0;
  transition: background 0.15s ease;
}

.switch .knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: #fff;
  transition: left 0.15s ease;
}

.switch.on {
  background: #f1f1f1;
  border-color: #f1f1f1;
}

.switch.on .knob {
  background: #181818;
}

.switch.on .knob {
  left: 16px;
}

/* 分段选择 */
.seg {
  display: flex;
  border: 1px solid rgba(255, 255, 255, 0.11);
  border-radius: 7px;
  overflow: hidden;
  flex-shrink: 0;
  margin-left: auto;
}

.seg button {
  background: transparent;
  border: none;
  color: #9aa1b5;
  font-size: 11px;
  padding: 5px 11px;
  cursor: pointer;
  line-height: 12px;
}

.seg button.on {
  background: rgba(255, 255, 255, 0.13);
  color: #fff;
}

/* 下拉选择 */
.shortcut-input {
  width: 112px;
  margin-left: auto;
  padding: 6px 8px;
  border: 1px solid rgba(255, 255, 255, 0.16);
  border-radius: 6px;
  outline: none;
  background: rgba(255, 255, 255, 0.07);
  color: var(--panel-text, #d8dce6);
  font: inherit;
  font-size: 10px;
  text-align: center;
  cursor: pointer;
}

.shortcut-input:hover,
.shortcut-input:focus {
  border-color: var(--panel-accent, #7d98f5);
  background: rgba(255, 255, 255, 0.11);
}

.panel select {
  background: rgba(255, 255, 255, 0.1);
  color: #d8dce6;
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 4px;
  min-width: 88px;
  margin-left: auto;
  font-size: 11px;
  padding: 6px 8px;
  outline: none;
  flex-shrink: 0;
}

.panel select option {
  background: #242936;
  color: #d8dce6;
}

/* 小按钮 */
.btn-mini {
  background: rgba(255, 255, 255, 0.07);
  color: var(--panel-text, #ddd);
  border: 1px solid rgba(255, 255, 255, 0.11);
  border-radius: 6px;
  font-size: 11px;
  padding: 7px 10px;
  cursor: pointer;
  flex-shrink: 0;
}

.btn-mini:hover {
  background: rgba(255, 255, 255, 0.12);
  color: #fff;
}

.chart {
  width: 100%;
  height: 100px;
  margin: 8px 0;
}

.chart-hint {
  padding: 20px 0;
  text-align: center;
  color: var(--panel-val, #666);
  font-size: 12px;
}

.kpi-row {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px;
  margin: 4px 0;
}
.open-detail-row {
  display: flex;
  justify-content: flex-end;
  margin: 2px 0 4px;
}
.kpi {
  background: rgba(107, 138, 240, 0.1);
  border: 1px solid rgba(107, 138, 240, 0.25);
  border-radius: 8px;
  padding: 9px 6px;
  text-align: center;
}
.kpi-val {
  font-size: 13px;
  font-weight: 700;
  color: #fff;
  font-variant-numeric: tabular-nums;
  line-height: 1.1;
}
.kpi-lbl {
  font-size: 7px;
  color: var(--panel-val, #888);
  margin-top: 2px;
}

.privacy-row {
  display: block;
  min-height: 0;
  padding-block: 8px;
}

.privacy-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.privacy-badge {
  color: #72c894;
  font-size: 9px;
  letter-spacing: 0.4px;
}

.privacy-summary {
  margin: 7px 0 8px;
  color: var(--panel-text, #cfd4df);
  font-size: 11px;
  line-height: 1.55;
}

.privacy-list {
  display: grid;
  gap: 7px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.privacy-list li {
  display: grid;
  grid-template-columns: 56px minmax(0, 1fr);
  gap: 8px;
  color: var(--panel-val, #888);
  font-size: 10px;
  line-height: 1.5;
}

.privacy-list strong {
  color: var(--panel-text, #cfd4df);
  font-weight: 600;
}

.system-actions {
  flex-wrap: wrap;
  justify-content: flex-end;
}
.system-message {
  color: var(--panel-val, #8f97a8);
  font-size: 10px;
  line-height: 1.5;
  padding: 2px;
  word-break: break-all;
  user-select: text;
}

.chart-axis {
  display: flex;
  justify-content: space-between;
  font-size: 10px;
  color: var(--panel-val, #666);
  margin-bottom: 8px;
}

.panel-foot {
  padding: 9px 12px 11px;
  gap: 8px;
  display: flex;
  justify-content: flex-end;
  flex-shrink: 0;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  background: rgba(0, 0, 0, 0.08);
}

.btn-secondary {
  margin-right: auto;
  color: var(--panel-secondary-text, var(--panel-text, #ddd));
  border-color: var(--panel-secondary-border, rgba(255, 255, 255, 0.14));
  background: var(--panel-secondary-bg, rgba(255, 255, 255, 0.07));
  transition:
    color 0.16s ease,
    background 0.16s ease,
    border-color 0.16s ease,
    box-shadow 0.16s ease,
    transform 0.16s ease;
}

.btn-secondary:hover {
  color: var(--panel-secondary-hover-text, #fff);
  border-color: var(--panel-secondary-hover-border, rgba(255, 255, 255, 0.25));
  background: var(--panel-secondary-hover, rgba(255, 255, 255, 0.14));
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.16);
  transform: translateY(-1px);
}

.btn-secondary:active {
  box-shadow: none;
  transform: translateY(0);
}

.btn-danger {
  color: var(--panel-danger-text, #ef9a9a);
  border-color: var(--panel-danger-border, rgba(239, 100, 100, 0.28));
  background: var(--panel-danger-bg, rgba(239, 100, 100, 0.08));
  transition:
    color 0.16s ease,
    background 0.16s ease,
    border-color 0.16s ease,
    box-shadow 0.16s ease,
    transform 0.16s ease;
}

.btn-danger:hover {
  color: var(--panel-danger-hover-text, #ffd4d4);
  border-color: var(--panel-danger-hover-border, rgba(239, 100, 100, 0.48));
  background: var(--panel-danger-hover, rgba(239, 100, 100, 0.18));
  box-shadow: 0 4px 12px rgba(168, 45, 45, 0.2);
  transform: translateY(-1px);
}
.btn-danger:active {
  background: var(--panel-danger-active, rgba(239, 100, 100, 0.24));
  box-shadow: none;
  transform: translateY(0);
}

.tab:focus-visible,
.panel button:focus-visible,
.panel select:focus-visible,
.panel input:focus-visible {
  outline: 2px solid var(--panel-accent, #6b8af0);
  outline-offset: 2px;
}

.capsule-shell-right {
  position: absolute;
  right: 0;
}

.panel.align-right {
  left: auto;
  right: 0;
}

.resize-handle {
  position: absolute;
  left: 2px;
  bottom: 2px;
  width: 20px;
  height: 20px;
  cursor: nesw-resize;
  background: linear-gradient(
    225deg,
    transparent 50%,
    rgba(255, 255, 255, 0.25) 50%
  );
}

html[data-ui-theme="light"] .panel {
  --panel-bg: rgba(250, 250, 249, 0.99);
  --panel-border: rgba(20, 28, 42, 0.12);
  --panel-title: #1d2433;
  --panel-text: #344054;
  --panel-val: #667085;
  --panel-accent: #1f2937;
  --panel-bg-active: rgba(17, 24, 39, 0.07);
  --panel-control-bg: #eceef1;
  --panel-control-active: #ffffff;
  --panel-danger-text: #b42318;
  --panel-danger-border: rgba(180, 35, 24, 0.24);
  --panel-danger-bg: rgba(180, 35, 24, 0.06);
  --panel-danger-hover: rgba(180, 35, 24, 0.12);
  --panel-danger-hover-text: #8f1d14;
  --panel-danger-hover-border: rgba(180, 35, 24, 0.38);
  --panel-danger-active: rgba(180, 35, 24, 0.18);
  --panel-secondary-text: #475467;
  --panel-secondary-border: rgba(20, 28, 42, 0.16);
  --panel-secondary-bg: rgba(20, 28, 42, 0.035);
  --panel-secondary-hover-text: #1d2939;
  --panel-secondary-hover-border: rgba(20, 28, 42, 0.24);
  --panel-secondary-hover: rgba(20, 28, 42, 0.09);
  box-shadow:
    0 16px 40px rgba(16, 24, 40, 0.18),
    inset 0 1px 0 rgba(255, 255, 255, 0.72);
}

html[data-ui-theme="light"] .panel-head,
html[data-ui-theme="light"] .tabs,
html[data-ui-theme="light"] .panel-foot {
  border-color: rgba(20, 28, 42, 0.09);
}

html[data-ui-theme="light"] .row {
  color: #344054;
  background: transparent;
  border-color: rgba(20, 28, 42, 0.07);
}

html[data-ui-theme="light"] .row.row-flat {
  background: transparent;
  border-color: transparent;
}
html[data-ui-theme="light"] .tab:hover {
  background: rgba(20, 28, 42, 0.05);
}
html[data-ui-theme="light"] .panel-close {
  color: #667085;
}
html[data-ui-theme="light"] .panel-close:hover {
  background: rgba(20, 28, 42, 0.08);
  color: #1d2433;
}
html[data-ui-theme="light"] .switch {
  background: rgba(20, 28, 42, 0.12);
  border-color: rgba(20, 28, 42, 0.16);
}
html[data-ui-theme="light"] .switch.on {
  background: #242424;
  border-color: #242424;
}
html[data-ui-theme="light"] .switch.on .knob {
  background: #fff;
}
html[data-ui-theme="light"] .seg {
  border-color: rgba(20, 28, 42, 0.18);
}
html[data-ui-theme="light"] .seg button {
  color: #667085;
}
html[data-ui-theme="light"] .seg button.on {
  background: #242424;
  color: #fff;
}
html[data-ui-theme="light"] .panel select {
  background: #fff;
  color: #344054;
  border-color: rgba(20, 28, 42, 0.16);
}
html[data-ui-theme="light"] .panel select option {
  background: #fff;
  color: #344054;
}
html[data-ui-theme="light"] .shortcut-input {
  background: #fff;
  color: #344054;
  border-color: rgba(20, 28, 42, 0.16);
}
html[data-ui-theme="light"] .shortcut-input:hover,
html[data-ui-theme="light"] .shortcut-input:focus {
  background: #f7f7f6;
  border-color: rgba(20, 28, 42, 0.36);
}
html[data-ui-theme="light"] .panel-foot {
  background: rgba(20, 28, 42, 0.025);
}
html[data-ui-theme="light"] .panel .btn-mini:not(.btn-secondary):not(.btn-danger) {
  color: #475467;
  border-color: rgba(20, 28, 42, 0.14);
  background: #eceef1;
}
html[data-ui-theme="light"] .panel .btn-mini:not(.btn-secondary):not(.btn-danger):hover {
  color: #1d2939;
  border-color: rgba(20, 28, 42, 0.22);
  background: #e2e5e9;
}
html[data-ui-theme="light"] .panel .btn-mini:not(.btn-secondary):not(.btn-danger):active {
  background: #d9dde3;
}
html[data-ui-theme="light"] .panel .btn-mini:not(.btn-secondary):not(.btn-danger):disabled {
  color: #98a2b3;
  cursor: not-allowed;
}
html[data-ui-theme="light"] .kpi-val {
  color: #1d2433;
}
html[data-ui-theme="light"] .panel-body::-webkit-scrollbar-thumb {
  background: rgba(20, 28, 42, 0.18);
}
html[data-ui-theme="light"] .resize-handle {
  background: linear-gradient(
    225deg,
    transparent 50%,
    rgba(20, 28, 42, 0.2) 50%
  );
}
</style>
