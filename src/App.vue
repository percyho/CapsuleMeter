<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";

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

const usage = ref<UsageData | null>(null);
const loading = ref(true);
const lastError = ref<string | null>(null);

// 当前左侧/右侧是否正在显示重置时间
const showLeftReset = ref(false);
const showRightReset = ref(false);
let leftTimer: number | undefined;
let rightTimer: number | undefined;


const leftValue = computed(() => {
  const w = usage.value?.five_hour;
  if (!w || w.remaining_percent === null || w.remaining_percent === undefined)
    return "--";
  return String(Math.round(w.remaining_percent));
});

const rightValue = computed(() => {
  const w = usage.value?.weekly;
  if (!w || w.remaining_percent === null || w.remaining_percent === undefined)
    return "--";
  return String(Math.round(w.remaining_percent));
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
  return `${d.getMonth() + 1}/${d.getDate()} ${hh}:${mm}`;
}

// 左侧点击：显示 5 小时重置倒计时，5 秒后恢复
function onLeftClick() {
  if (usage.value?.error) return;
  showLeftReset.value = !showLeftReset.value;
  clearTimer("left");
  if (showLeftReset.value) {
    leftTimer = window.setTimeout(() => {
      showLeftReset.value = false;
    }, 5000);
  }
}

// 右侧点击：显示周重置倒计时，5 秒后恢复
function onRightClick() {
  if (usage.value?.error) return;
  showRightReset.value = !showRightReset.value;
  clearTimer("right");
  if (showRightReset.value) {
    rightTimer = window.setTimeout(() => {
      showRightReset.value = false;
    }, 5000);
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

// 剩余量填充宽度：剩余多少百分比就填充多少（用量满=全填，用光=全透明）
const leftFillWidth = computed(() => {
  const w = usage.value?.five_hour;
  if (!w || w.remaining_percent === null || w.remaining_percent === undefined)
    return "0%";
  return `${Math.max(0, Math.min(100, w.remaining_percent))}%`;
});
const rightFillWidth = computed(() => {
  const w = usage.value?.weekly;
  if (!w || w.remaining_percent === null || w.remaining_percent === undefined)
    return "0%";
  return `${Math.max(0, Math.min(100, w.remaining_percent))}%`;
});

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
}

let interval: number | undefined;

// —— 右键设置面板 ——
const showPanel = ref(false);
const opacity = ref(Number(localStorage.getItem("opacity") ?? "0.72"));
const fontSize = ref(Number(localStorage.getItem("fontSize") ?? "13"));
const theme = ref(localStorage.getItem("theme") ?? "light");

const THEMES: Record<string, { bg: string; left: string; right: string; num: string; name: string }> = {
  light: { bg: "linear-gradient(180deg, rgba(233,236,241,0.72), rgba(199,204,214,0.72))", left: "rgba(255,255,255,0.85)", right: "rgba(238,96,100,0.82)", num: "#2b2f3a", name: "浅灰" },
  dark: { bg: "linear-gradient(180deg, rgba(50,54,66,0.78), rgba(30,33,42,0.78))", left: "rgba(120,160,255,0.7)", right: "rgba(255,130,130,0.7)", num: "#e8eaf0", name: "深色" },
  forest: { bg: "linear-gradient(180deg, rgba(220,235,220,0.72), rgba(180,205,185,0.72))", left: "rgba(120,200,140,0.75)", right: "rgba(240,160,90,0.75)", num: "#2d3a30", name: "森林" },
  sunset: { bg: "linear-gradient(180deg, rgba(245,220,200,0.72), rgba(220,180,170,0.72))", left: "rgba(255,200,120,0.75)", right: "rgba(220,90,120,0.75)", num: "#3a2b2b", name: "晚霞" }
};

function applyTheme() {
  const t = THEMES[theme.value] || THEMES.light;
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

const panelW = ref(Number(localStorage.getItem("panelW") ?? "300"));
const panelH = ref(Number(localStorage.getItem("panelH") ?? "225"));
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
  panelW.value = Math.max(120, Math.min(300, panelResize.w + dx));
  panelH.value = Math.max(80, Math.min(200, panelResize.h + dy));
  getCurrentWindow().setSize(new LogicalSize(panelW.value, 30 + panelH.value));
}
function onPanelResizeEnd() {
  localStorage.setItem("panelW", String(panelW.value));
  localStorage.setItem("panelH", String(panelH.value));
  window.removeEventListener("mousemove", onPanelResizeMove);
  window.removeEventListener("mouseup", onPanelResizeEnd);
  panelResize = null;
}

function applyWindowSettings() {
  try {
    applyTheme();
    document.body.style.opacity = String(opacity.value);
    document.documentElement.style.setProperty("--num-size", fontSize.value + "px");
  } catch {}
}

function togglePanel() {
  showPanel.value = !showPanel.value;
  const win = getCurrentWindow() as any;
  if (showPanel.value) {
    win.setSize(new LogicalSize(panelW.value, 30 + panelH.value));
  } else {
    win.setSize(new LogicalSize(100, 30));
  }
}

function onContextMenu(e: MouseEvent) {
  e.preventDefault();
  togglePanel();
}


function onOpacityChange() {
  localStorage.setItem("opacity", String(opacity.value));
  applyWindowSettings();
}
function onFontSizeChange() {
  localStorage.setItem("fontSize", String(fontSize.value));
  applyWindowSettings();
}

onMounted(() => {
  applyWindowSettings();
  refresh();
  // 每 5 分钟自动刷新一次
  interval = window.setInterval(refresh, 5 * 60 * 1000);
});

onBeforeUnmount(() => {
  if (interval) window.clearInterval(interval);
  clearTimer("left");
  clearTimer("right");
});
</script>

<template>
  <div class="capsule" :class="{ error: lastError }" @mousedown="onCapsuleMouseDown" @contextmenu="onContextMenu">
    <div class="half left" @click="onLeftClick" :title="leftTitle">
      <div class="fill" :style="{ width: leftFillWidth }"></div>
      <span class="num" :class="{ dim: showLeftReset }">{{ leftDisplay }}</span>
    </div>
    <div class="half right" @click="onRightClick" :title="rightTitle">
      <div class="fill fill--red" :style="{ width: rightFillWidth }"></div>
      <span class="num" :class="{ dim: showRightReset }">{{ rightDisplay }}</span>
    </div>
  </div>
  <div v-if="showPanel" class="panel" :style="{ width: panelW + 'px', height: panelH + 'px' }" @contextmenu.prevent>
    <div class="row themes">
      <button v-for="(_, key) in THEMES" :key="key" class="theme-dot" :class="{ active: theme === key }"
              :title="THEMES[key].name" @click="theme = key; onThemeChange()"></button>
    </div>
    <div class="resize-handle" @mousedown="onPanelResizeStart"></div>

    <label class="row">
      <span>透明度</span>
      <input type="range" min="0.3" max="1" step="0.05" v-model.number="opacity" @input="onOpacityChange" />
    </label>
    <label class="row">
      <span>字号</span>
      <input type="range" min="9" max="20" step="1" v-model.number="fontSize" @input="onFontSizeChange" />
    </label>
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

/* 剩余量填充层：剩余多少就填多少，已用部分透明露出胶囊底色 */
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
  font-size: 6.5px;
  font-weight: 600;
  color: #5a6172;
  opacity: 1;
  white-space: nowrap;
  padding: 0 3px;
}





/* 设置面板 */
.panel {
  position: absolute;
  position: fixed;
  top: 30px;
  left: 0;
  z-index: 10;
  background: rgba(30, 33, 42, 0.97);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  padding: 6px 8px;
  margin-top: 4px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.panel .row {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 9px;
  color: #d8dce6;
}

.themes {
  justify-content: center;
  gap: 6px;
}

.theme-dot {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  border: 1px solid rgba(255,255,255,0.3);
  cursor: pointer;
  padding: 0;
}

.theme-dot.active {
  outline: 2px solid #6b8af0;
  outline-offset: 1px;
}

.theme-dot:nth-child(1) { background: #c8ccd4; }
.theme-dot:nth-child(2) { background: #2a2e3a; }
.theme-dot:nth-child(3) { background: #78c88c; }
.theme-dot:nth-child(4) { background: #f0a878; }

.panel input[type="range"] {
  flex: 1;
  height: 3px;
  accent-color: #6b8af0;
}

.resize-handle {
  position: absolute;
  right: 2px;
  bottom: 2px;
  width: 12px;
  height: 12px;
  cursor: nwse-resize;
  background: linear-gradient(135deg, transparent 50%, rgba(255,255,255,0.25) 50%);
}

.panel input[type="checkbox"] {
  accent-color: #6b8af0;
  width: 10px;
  height: 10px;
  margin: 0;
}
</style>
