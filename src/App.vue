<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, computed, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize, LogicalPosition } from "@tauri-apps/api/window";
import {
  enable as autostartEnable,
  disable as autostartDisable,
  isEnabled as autostartIsEnabled,
} from "@tauri-apps/plugin-autostart";
import StatsPanel from "./components/statistics/StatsPanel.vue";

// —— 整窗按住拖动（移动超过阈值才启动拖动，单击仍触发点击） ——
const DRAG_THRESHOLD = 4;
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

interface WindowData {
  remaining_percent: number | null;
  used_percent: number | null;
  reset_at: number | null;
  reset_after_seconds: number | null;
}

interface UsageData {
  plan: string | null;
  five_hour: WindowData;
  weekly: WindowData;
  error: string | null;
}

interface HistoryPoint {
  t: number;
  p?: number | null;
  fiveHour?: number | null;
  weekly?: number | null;
}

const usage = ref<UsageData | null>(null);
const loading = ref(true);
const lastError = ref<string | null>(null);
const lastRefresh = ref("--");

// 当前左侧/右侧是否正在显示重置时间
const showLeftReset = ref(false);
const showRightReset = ref(false);
let leftTimer: number | undefined;
let rightTimer: number | undefined;

// —— 可配置项（localStorage 持久化）——
const opacity = ref(Number(localStorage.getItem("opacity") ?? "0.72"));
const fontSize = ref(Number(localStorage.getItem("fontSize") ?? "13"));
const theme = ref(localStorage.getItem("theme") ?? "neon");
const activeTab = ref("appearance");
const historyPoints = ref<HistoryPoint[]>(JSON.parse(localStorage.getItem("usageHistory") || "[]"));
function recordHistory() {
  if (!usage.value) return;
  const now = Date.now();
  historyPoints.value.push({
    t: now,
    fiveHour: usage.value.five_hour?.remaining_percent,
    weekly: usage.value.weekly?.remaining_percent,
  });
  // 为统计页的月视图保留最近 30 天，同时兼容旧版 p 字段。
  const cutoff = now - 30 * 86400000;
  historyPoints.value = historyPoints.value.filter(h => h.t > cutoff);
  localStorage.setItem("usageHistory", JSON.stringify(historyPoints.value));
}
async function onQuit() {
  await getCurrentWindow().close();
}


const alwaysOnTop = ref(localStorage.getItem("alwaysOnTop") !== "false");
const snapEnabled = ref(localStorage.getItem("snapEnabled") !== "false");
const displayMode = ref(localStorage.getItem("displayMode") ?? "remaining"); // remaining | used
const refreshMin = ref(Number(localStorage.getItem("refreshMin") ?? "5"));
const resetShowSec = ref(Number(localStorage.getItem("resetShowSec") ?? "5"));
const autostart = ref(false);

const THEMES: Record<string, { bg: string; left: string; right: string; num: string; name: string }> = {
  light: { bg: "linear-gradient(180deg, rgba(233,236,241,0.72), rgba(199,204,214,0.72))", left: "rgba(255,255,255,0.85)", right: "rgba(238,96,100,0.82)", num: "#2b2f3a", name: "浅灰" },
  dark: { bg: "linear-gradient(180deg, rgba(50,54,66,0.78), rgba(30,33,42,0.78))", left: "rgba(120,160,255,0.7)", right: "rgba(255,130,130,0.7)", num: "#e8eaf0", name: "深色" },
  forest: { bg: "linear-gradient(180deg, rgba(220,235,220,0.72), rgba(180,205,185,0.72))", left: "rgba(120,200,140,0.75)", right: "rgba(240,160,90,0.75)", num: "#2d3a30", name: "森林" },
  sunset: { bg: "linear-gradient(180deg, rgba(245,220,200,0.72), rgba(220,180,170,0.72))", left: "rgba(255,200,120,0.75)", right: "rgba(220,90,120,0.75)", num: "#3a2b2b", name: "晚霞" }
};

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
  if (showLeftReset.value) return fmtClock(w.reset_at);
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
  return w?.reset_at ? `5h 重置于 ${fmtClock(w.reset_at)}` : "";
});
const rightTitle = computed(() => {
  const w = usage.value?.weekly;
  return w?.reset_at ? `周重置于 ${fmtClock(w.reset_at)}` : "";
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
  try {
    const data = await invoke<UsageData>("fetch_usage");
    usage.value = data;
    lastError.value = data.error ?? null;
  } catch (e) {
    lastError.value = String(e);
    usage.value = null;
  } finally {
    loading.value = false;
  }
  lastRefresh.value = new Date().toLocaleTimeString("zh-CN", { hour12: false });
  recordHistory();
}

let interval: number | undefined;
function restartInterval() {
  if (interval) window.clearInterval(interval);
  interval = window.setInterval(refresh, refreshMin.value * 60 * 1000);
}
watch(refreshMin, restartInterval);

// —— 外观 ——
function applyTheme() {
  const t = THEMES[theme.value] || THEMES.neon;
  const root = document.documentElement.style;
  root.setProperty("--bg", t.bg);
  root.setProperty("--left-fill", t.left);
  root.setProperty("--right-fill", t.right);
  root.setProperty("--num-color", t.num);
}

function onThemeChange() {
  localStorage.setItem("theme", theme.value);
  applyTheme();
}

function applyWindowSettings() {
  applyTheme();
  document.body.style.opacity = String(opacity.value);
  document.documentElement.style.setProperty("--num-size", fontSize.value + "px");
}

function onOpacityChange() {
  localStorage.setItem("opacity", String(opacity.value));
  applyWindowSettings();
}
function onFontSizeChange() {
  localStorage.setItem("fontSize", String(fontSize.value));
  applyWindowSettings();
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

function onDisplayModeChange() {
  localStorage.setItem("displayMode", displayMode.value);
}

function onResetShowSecChange() {
  localStorage.setItem("resetShowSec", String(resetShowSec.value));
  // 若正在显示重置时间，立即用新时长重新计时
  if (showLeftReset.value) {
    clearTimer("left");
    leftTimer = window.setTimeout(() => { showLeftReset.value = false; }, resetShowSec.value * 1000);
  }
  if (showRightReset.value) {
    clearTimer("right");
    rightTimer = window.setTimeout(() => { showRightReset.value = false; }, resetShowSec.value * 1000);
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
const panelW = ref(Math.min(600, Math.max(PANEL_MIN_WIDTH, Number(localStorage.getItem("panelW") ?? PANEL_DEFAULT_WIDTH))));
const panelH = ref(Math.min(800, Math.max(PANEL_MIN_HEIGHT, Number(localStorage.getItem("panelH") ?? PANEL_DEFAULT_HEIGHT))));
let panelResize: { sx: number; sy: number; w: number; h: number } | null = null;

function onPanelResizeStart(e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  panelResize = { sx: e.clientX, sy: e.clientY, w: panelW.value, h: panelH.value };
  window.addEventListener("mousemove", onPanelResizeMove);
  window.addEventListener("mouseup", onPanelResizeEnd);
}
function onPanelResizeMove(e: MouseEvent) {
  if (!panelResize) return;
  const dx = e.clientX - panelResize.sx;
  const dy = e.clientY - panelResize.sy;
  panelW.value = Math.max(PANEL_MIN_WIDTH, Math.min(600, panelResize.w + dx));
  panelH.value = Math.max(PANEL_MIN_HEIGHT, Math.min(800, panelResize.h + dy));
  getCurrentWindow().setSize(new LogicalSize(panelW.value, 30 + panelH.value));
}
function onPanelResizeEnd() {
  localStorage.setItem("panelW", String(panelW.value));
  localStorage.setItem("panelH", String(panelH.value));
  window.removeEventListener("mousemove", onPanelResizeMove);
  window.removeEventListener("mouseup", onPanelResizeEnd);
  panelResize = null;
}

const panelAlignRight = ref(false);

async function togglePanel() {
  showPanel.value = !showPanel.value;
  const win = getCurrentWindow() as any;
  if (showPanel.value) {
    // 用 screenX 判断（Tauri webview 中 screenX = 窗口在屏幕上的 x）
    const winX = window.screenX;
    const winY = window.screenY;
    const screenW = window.screen.availWidth;
    console.log("DEBUG:", { winX, winY, screenW, panelW: panelW.value });
    panelAlignRight.value = (winX + panelW.value) > screenW;
    await win.setSize(new LogicalSize(panelW.value, 30 + panelH.value));
    if (panelAlignRight.value) {
      await win.setPosition(new LogicalPosition(winX - (panelW.value - 100), winY));
    }
  } else {
    // 关闭面板时恢复窗口宽度和位置
    const winX = window.screenX;
    const winY = window.screenY;
    await win.setSize(new LogicalSize(100, 30));
    if (panelAlignRight.value) {
      await win.setPosition(new LogicalPosition(winX + (panelW.value - 100), winY));
    }
    panelAlignRight.value = false;
  }
}

function onContextMenu(e: MouseEvent) {
  e.preventDefault();
  togglePanel();
}

// —— 恢复默认 ——
function resetDefaults() {
  opacity.value = 0.72;
  fontSize.value = 13;
  theme.value = "neon";
  alwaysOnTop.value = true;
  snapEnabled.value = true;
  displayMode.value = "remaining";
  refreshMin.value = 5;
  resetShowSec.value = 5;
  panelW.value = PANEL_DEFAULT_WIDTH;
  panelH.value = PANEL_DEFAULT_HEIGHT;
  ["opacity", "fontSize", "theme", "alwaysOnTop", "snapEnabled", "displayMode", "refreshMin", "resetShowSec", "panelW", "panelH"].forEach((k) =>
    localStorage.removeItem(k)
  );
  applyWindowSettings();
  applyAlwaysOnTop();
  applySnap();
  restartInterval();
  if (autostart.value) {
    autostart.value = false;
    onAutostartChange();
  }
  getCurrentWindow().setSize(new LogicalSize(PANEL_DEFAULT_WIDTH, 30 + PANEL_DEFAULT_HEIGHT));
}

onMounted(() => {
  applyWindowSettings();
  applyAlwaysOnTop();
  applySnap();
  initAutostart();
  refresh();
  restartInterval();
});

onBeforeUnmount(() => {
  if (interval) window.clearInterval(interval);
  clearTimer("left");
  clearTimer("right");
});
</script>

<template>
  <div class="capsule" :class="[{'error': lastError}, {'capsule-right': panelAlignRight}]" @mousedown="onCapsuleMouseDown" @contextmenu="onContextMenu">
    <div class="half left" @click="onLeftClick" :title="leftTitle">
      <div class="fill" :style="{ width: leftFillWidth }"></div>
      <span class="num" :class="{ dim: showLeftReset }">{{ leftDisplay }}</span>
    </div>
    <div class="half right" @click="onRightClick" :title="rightTitle">
      <div class="fill fill--red" :style="{ width: rightFillWidth }"></div>
      <span class="num" :class="{ dim: showRightReset }">{{ rightDisplay }}</span>
    </div>
  </div>

  <div v-if="showPanel" class="panel" :class='[theme, { "align-right": panelAlignRight }]' :style="{ width: panelW + 'px', height: panelH + 'px' }" @contextmenu.prevent>
    <div class="panel-head">
      <div class="panel-heading">
        <span class="panel-title">设置</span>
        <span class="panel-description">个性化用量胶囊</span>
      </div>
      <span class="panel-sub">{{ lastError ? "数据异常" : (usage?.plan || "未登录").toUpperCase() }}</span>
      <button class="panel-close" title="关闭" aria-label="关闭设置" @click="togglePanel()">×</button>
    </div>

    <div class="tabs" role="tablist" aria-label="设置分类">
      <button class="tab" :class="{ on: activeTab==='appearance' }" role="tab" :aria-selected="activeTab==='appearance'" @click="activeTab='appearance'"><svg aria-hidden="true" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 2a10 10 0 0 1 10 10h-10V2z"/><path d="M12 12l9.3-9.3"/><path d="M12 12l-6 6"/></svg><span>外观</span></button>
      <button class="tab" :class="{ on: activeTab==='behavior' }" role="tab" :aria-selected="activeTab==='behavior'" @click="activeTab='behavior'"><svg aria-hidden="true" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/></svg><span>行为</span></button>
      <button class="tab" :class="{ on: activeTab==='stats' }" role="tab" :aria-selected="activeTab==='stats'" @click="activeTab='stats'"><svg aria-hidden="true" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 3v18h18"/><path d="M18 17V9"/><path d="M13 17V5"/><path d="M8 17v-3"/></svg><span>统计</span></button>
      <button class="tab" :class="{ on: activeTab==='system' }" role="tab" :aria-selected="activeTab==='system'" @click="activeTab='system'"><svg aria-hidden="true" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg><span>系统</span></button>
    </div>

    <div class="panel-body">
      <template v-if="activeTab==='appearance'">
      <div class="row">
        <span class="row-name">主题</span>
        <div class="themes">
          <button v-for="(_, key) in THEMES" :key="key" class="theme-dot" :class="{ active: theme === key }"
                  :title="THEMES[key].name" @click="theme = key; onThemeChange()"></button>
        </div>
        <span class="row-val">{{ THEMES[theme]?.name || "" }}</span>
      </div>
      <div class="row">
        <span class="row-name">透明度</span>
        <input type="range" min="0.3" max="1" step="0.05" v-model.number="opacity" @input="onOpacityChange" />
        <span class="row-val">{{ Math.round(opacity * 100) }}%</span>
      </div>
      <div class="row">
        <span class="row-name">字号</span>
        <input type="range" min="9" max="20" step="1" v-model.number="fontSize" @input="onFontSizeChange" />
        <span class="row-val">{{ fontSize }}px</span>
      </div>

      </template>

      <template v-if="activeTab==='behavior'">
      <div class="group-label">行为</div>
      <div class="row">
        <span class="row-name">窗口置顶</span>
        <button class="switch" :class="{ on: alwaysOnTop }" @click="alwaysOnTop = !alwaysOnTop; onAlwaysOnTopChange()">
          <span class="knob"></span>
        </button>
      </div>
      <div class="row">
        <span class="row-name">贴边吸附</span>
        <button class="switch" :class="{ on: snapEnabled }" @click="snapEnabled = !snapEnabled; onSnapChange()">
          <span class="knob"></span>
        </button>
      </div>
      <div class="row">
        <span class="row-name">显示模式</span>
        <div class="seg">
          <button :class="{ on: displayMode === 'remaining' }" @click="displayMode = 'remaining'; onDisplayModeChange()">剩余</button>
          <button :class="{ on: displayMode === 'used' }" @click="displayMode = 'used'; onDisplayModeChange()">已用</button>
        </div>
      </div>
      <div class="row">
        <span class="row-name">重置显示时长</span>
        <select v-model.number="resetShowSec" @change="onResetShowSecChange()">
          <option :value="3">3 秒</option>
          <option :value="5">5 秒</option>
          <option :value="8">8 秒</option>
          <option :value="10">10 秒</option>
        </select>
      </div>

      <div class="group-label">数据</div>
      <div class="row">
        <span class="row-name">自动刷新</span>
        <select v-model.number="refreshMin" @change="restartInterval()">
          <option :value="1">1 分钟</option>
          <option :value="2">2 分钟</option>
          <option :value="5">5 分钟</option>
          <option :value="10">10 分钟</option>
          <option :value="30">30 分钟</option>
        </select>
      </div>
      <div class="row">
        <span class="row-name">上次刷新</span>
        <span class="row-val">{{ lastRefresh }}</span>
        <button class="btn-mini" @click="refresh()">立即刷新</button>
      </div>

      </template>

      <template v-if="activeTab==='stats'">
        <StatsPanel :history-points="historyPoints" @open-history="invoke('open_history')" />
      </template>

      <template v-if="activeTab==='system'">
      <div class="group-label">系统</div>
      <div class="row">
        <span class="row-name">开机自启</span>
        <button class="switch" :class="{ on: autostart }" @click="autostart = !autostart; onAutostartChange()">
          <span class="knob"></span>
        </button>
      </div>

      <div class="group-label">关于</div>
      <div class="row"><span class="row-name">版本</span><span class="row-val">v1.0.0</span></div>
      <div class="row privacy-row"><span class="row-name">隐私</span><p class="privacy-text">本地运行，不上传数据</p></div>
      <div class="row"><span class="row-name">许可证</span><span class="row-val">MIT</span></div>
      </template>

    </div>

    <div class="panel-foot">
      <button class="btn-mini btn-secondary" @click="resetDefaults()">恢复默认</button>
      <button class="btn-mini btn-danger" @click="onQuit()">退出应用</button>
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
  height: 30px;
  flex-shrink: 0;
  background: var(--bg);
  border: 1px solid rgba(255, 255, 255, 0.5);
  border-radius: 999px;
  box-shadow: inset 0 1px 0 rgba(255,255,255,0.6), 0 1px 4px rgba(0,0,0,0.18);
  overflow: hidden;
  cursor: default;
}

.half {
  position: relative;
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 0;
  overflow: hidden;
}

/* 填充层：按当前显示模式从中间分隔线向外填充 */
.fill {
  position: absolute;
  right: 0;
  top: 0;
  bottom: 0;
  width: 0%;
  background: var(--left-fill);
  transition: width 0.4s ease;
}

.fill--red {
  left: 0;
  right: auto;
  background: var(--right-fill);
}

.half:hover {
  background: rgba(255, 255, 255, 0.07);
}

.num {
  position: relative;
  z-index: 1;
  font-size: var(--num-size, 13px);
  font-weight: 700;
  color: var(--num-color);
  letter-spacing: 0;
  transition: opacity 0.15s ease;
}

.num.dim {
  font-size: calc(var(--num-size, 13px) * 0.78);
  font-weight: 600;
  color: #5a6172;
  opacity: 1;
  white-space: nowrap;
  padding: 0 3px;
}

/* —— 设置面板 —— */
.panel {
  position: fixed;
  top: 30px;
  left: 0;
  z-index: 10;
  display: flex;
  flex-direction: column;
  background: var(--panel-bg, rgba(24, 27, 35, 0.97));
  border: 1px solid var(--panel-border, rgba(255, 255, 255, 0.12));
  border-radius: 12px;
  margin-top: 6px;
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.42), inset 0 1px 0 rgba(255, 255, 255, 0.04);
  backdrop-filter: blur(20px);
  overflow: hidden;
}

.panel-head {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 56px;
  padding: 10px 12px 9px 14px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  flex-shrink: 0;
}

.panel-heading {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.panel-title {
  font-size: 14px;
  font-weight: 700;
  color: var(--panel-title, #f4f6fb);
  letter-spacing: 0.2px;
}

.panel-description {
  color: var(--panel-val, #7f8799);
  font-size: 9px;
  white-space: nowrap;
}

.panel-sub {
  flex: 1;
  font-size: 9px;
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

.tabs {
  display: flex;
  gap: 4px;
  padding: 8px 10px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.07);
  flex-shrink: 0;
}

.tab {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  min-width: 0;
  padding: 7px 5px;
  border: none;
  background: transparent;
  color: var(--panel-val, #7f8799);
  font-size: 10px;
  cursor: pointer;
  border-radius: 7px;
  transition: color 0.15s ease, background 0.15s ease;
}

.tab svg { flex-shrink: 0; }

.tab:hover { background: rgba(255, 255, 255, 0.05); }

.tab.on {
  background: var(--panel-bg-active, rgba(107, 138, 240, 0.15));
  color: var(--panel-text, #fff);
  font-weight: 600;
  box-shadow: inset 0 0 0 1px rgba(107, 138, 240, 0.2);
}

.panel-body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 10px 12px 12px;
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
  font-size: 9px;
  font-weight: 700;
  color: var(--panel-accent, #7d98f5);
  letter-spacing: 1.2px;
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
  min-height: 38px;
  padding: 7px 9px;
  font-size: 11px;
  color: #d8dce6;
  background: rgba(255, 255, 255, 0.035);
  border: 1px solid rgba(255, 255, 255, 0.055);
  border-radius: 8px;
}

.row-name {
  flex: 0 0 88px;
  color: var(--panel-text, #cfd4df);
  line-height: 1.35;
}

.row-val {
  margin-left: auto;
  font-size: 10px;
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

/* 主题色板 */
.themes {
  display: flex;
  align-items: center;
  gap: 5px;
  flex: 1;
}

.theme-dot {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: 1px solid rgba(255, 255, 255, 0.3);
  cursor: pointer;
  padding: 0;
  flex-shrink: 0;
}

.theme-dot.active {
  outline: 2px solid #6b8af0;
  outline-offset: 1px;
}

.theme-dot:nth-child(1) { background: #c8ccd4; }
.theme-dot:nth-child(2) { background: #2a2e3a; }
.theme-dot:nth-child(3) { background: #78c88c; }
.theme-dot:nth-child(4) { background: #f0a878; }

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
  background: #6b8af0;
  border-color: #6b8af0;
}

.switch.on .knob {
  left: 16px;
}

/* 分段选择 */
.seg {
  display: flex;
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 5px;
  overflow: hidden;
  flex-shrink: 0;
  margin-left: auto;
}

.seg button {
  background: transparent;
  border: none;
  color: #9aa1b5;
  font-size: 10px;
  padding: 4px 12px;
  cursor: pointer;
  line-height: 12px;
}

.seg button.on {
  background: #6b8af0;
  color: #fff;
}

/* 下拉选择 */
.panel select {
  background: rgba(255, 255, 255, 0.1);
  color: #d8dce6;
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 4px;
  min-width: 88px;
  margin-left: auto;
  font-size: 10px;
  padding: 5px 8px;
  outline: none;
  flex-shrink: 0;
}

.panel select option {
  background: #242936;
  color: #d8dce6;
}

/* 小按钮 */
.btn-mini {
  background: rgba(107, 138, 240, 0.16);
  color: #8fa7f5;
  border: 1px solid rgba(107, 138, 240, 0.35);
  border-radius: 6px;
  font-size: 10px;
  padding: 6px 10px;
  cursor: pointer;
  flex-shrink: 0;
}

.btn-mini:hover {
  background: rgba(107, 138, 240, 0.3);
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
  border-radius: 8px; padding: 9px 6px; text-align: center;
}
.kpi-val { font-size: 13px;
  font-weight: 700;
  color: #fff;
  font-variant-numeric: tabular-nums;
  line-height: 1.1;
}
.kpi-lbl { font-size: 7px;
  color: var(--panel-val, #888);
  margin-top: 2px;
}

.privacy-text { margin-left: auto; font-size: 10px; color: var(--panel-val, #888); line-height: 1.5; }

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

.btn-secondary { margin-right: auto; }

.btn-danger {
  color: #ef9a9a;
  border-color: rgba(239, 100, 100, 0.28);
  background: rgba(239, 100, 100, 0.08);
}

.btn-danger:hover { background: rgba(239, 100, 100, 0.18); }

.tab:focus-visible,
.panel button:focus-visible,
.panel select:focus-visible,
.panel input:focus-visible {
  outline: 2px solid var(--panel-accent, #6b8af0);
  outline-offset: 2px;
}

.panel.light {
  --panel-bg: rgba(245, 247, 250, 0.97);
  --panel-border: rgba(0, 0, 0, 0.1);
  --panel-title: #1a1d26;
  --panel-text: #2b2f3a;
  --panel-sub: #5a6172;
  --panel-val: #8a90a0;
  --panel-accent: #4a6ee0;
}

.capsule-right {
  position: absolute;
  right: 0;
}

.panel.align-right {
  left: auto;
  right: 0;
}

.resize-handle {
  position: absolute;
  right: 2px;
  bottom: 2px;
  width: 20px; height: 20px; cursor: se-resize;
  background: linear-gradient(135deg, transparent 50%, rgba(255,255,255,0.25) 50%);
}
</style>
