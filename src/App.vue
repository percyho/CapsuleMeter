<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

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

const planLabel = computed(() => {
  const p = usage.value?.plan;
  if (!p) return "···";
  return p;
});

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

onMounted(() => {
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
  <div class="capsule" :class="{ error: lastError }" @mousedown="onCapsuleMouseDown">
    <div class="badge" :title="lastError || 'Codex 套餐'">{{ planLabel }}</div>
    <div class="half left" @click="onLeftClick" :title="leftTitle">
      <div class="fill" :style="{ width: leftFillWidth }"></div>
      <span class="num" :class="{ dim: showLeftReset }">{{ leftDisplay }}</span>
    </div>
    <div class="half right" @click="onRightClick" :title="rightTitle">
      <div class="fill fill--red" :style="{ width: rightFillWidth }"></div>
      <span class="num" :class="{ dim: showRightReset }">{{ rightDisplay }}</span>
    </div>
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
  width: 100vw;
  height: 100vh;
  background: rgba(205, 210, 220, 0.5); /* 浅灰半透明底色 */
  border: 1px solid rgba(255, 255, 255, 0.35);
  border-radius: 999px; /* 两端全圆，胶囊形状 */
  box-shadow: inset 0 0 6px rgba(255, 255, 255, 0.25);
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
  background: rgba(255, 255, 255, 0.92); /* 左侧：从中间向左边填充 */
  transition: width 0.4s ease;
}

.fill--red {
  left: 0;
  right: auto;
  background: rgba(229, 72, 77, 0.92);
}

.half:hover {
  background: rgba(255, 255, 255, 0.07);
}

.num {
  position: relative;
  z-index: 1;
  font-size: 13px;
  font-weight: 700;
  color: #e8eaf0;
  letter-spacing: 0;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.6);
  background: rgba(16, 18, 24, 0.55);
  border-radius: 999px;
  padding: 0 4px;
  transition: opacity 0.15s ease;
}

.num.dim {
  font-size: 6.5px;
  font-weight: 600;
  color: #9fb3ff;
  opacity: 1;
  white-space: nowrap;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.8);
  background: rgba(16, 18, 24, 0.65);
  padding: 0 3px;
}

.badge {
  position: absolute;
  top: 1px;
  right: 4px;
  z-index: 2;
  font-size: 4.5px;
  font-weight: 700;
  color: rgba(255, 255, 255, 0.75);
  text-transform: uppercase;
  letter-spacing: 0.2px;
  pointer-events: none;
  line-height: 1;
}

.capsule.error .badge {
  color: #ff6b6b;
}
</style>
