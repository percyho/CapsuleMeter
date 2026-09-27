<script setup lang="ts">
import { BarChart, LineChart } from "echarts/charts";
import { GraphicComponent, GridComponent, TooltipComponent } from "echarts/components";
import { init, use, type ECharts } from "echarts/core";
import { CanvasRenderer } from "echarts/renderers";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { computed, nextTick, onBeforeUnmount, onMounted, shallowRef, useTemplateRef, watch } from "vue";
import WindowControls from "../components/common/WindowControls.vue";
import { useUiTheme } from "../composables/useUiTheme";
import { useUsageHistory } from "../composables/useUsageHistory";
import { useWindowControls } from "../composables/useWindowControls";
import type { UsageData } from "../types/usage";
import { compactNumber } from "../utils/formatters";

use([BarChart, LineChart, GraphicComponent, GridComponent, TooltipComponent, CanvasRenderer]);

interface DailyUsage { date?: string; tokens?: number }
interface AnalyticsData { daily: DailyUsage[]; total_tokens: number | null; peak_tokens: number | null; error: string | null }
type Tab = "quota" | "tokens";
type QuotaWindow = "fiveHour" | "weekly";
type QuotaRange = "current" | 7 | 14 | 30;
type TokenRange = 7 | 30 | 90 | 365 | 3650;

const tab = shallowRef<Tab>("quota");
const quotaWindow = shallowRef<QuotaWindow>("weekly");
const quotaRange = shallowRef<QuotaRange>("current");
const tokenRange = shallowRef<TokenRange>(30);
const loading = shallowRef(false);
const refreshing = shallowRef(false);
const error = shallowRef("");
const quotaError = shallowRef("");
const actionMessage = shallowRef("");
const daily = shallowRef<DailyUsage[]>([]);
const { historyPoints, reloadHistory, appendUsage, clearHistory: clearStoredHistory } = useUsageHistory({
  syncAcrossWindows: true,
  onChange: () => scheduleRender(0),
});
const chartElement = useTemplateRef<HTMLDivElement>("chart");
let chart: ECharts | null = null;
let resizeObserver: ResizeObserver | null = null;
let renderTimer: ReturnType<typeof setTimeout> | null = null;
let requestSequence = 0;
let unlistenHistoryShown: UnlistenFn | null = null;
const { toggleMaximizeWindow } = useWindowControls();
useUiTheme({ syncAcrossWindows: true, onChange: () => scheduleRender(0) });

const quotaRangeDays = computed(() => quotaRange.value === "current"
  ? (quotaWindow.value === "fiveHour" ? 1 : 7)
  : quotaRange.value);
const filteredQuotaPoints = computed(() => {
  const cutoff = Date.now() - quotaRangeDays.value * 86_400_000;
  return historyPoints.value
    .filter(point => point.t >= cutoff)
    .map(point => ({ time: point.t, value: quotaWindow.value === "weekly" ? point.weekly : (point.fiveHour ?? point.p) }))
    .filter((point): point is { time: number; value: number } => point.value != null);
});
const quotaStats = computed(() => {
  const points = filteredQuotaPoints.value;
  if (!points.length) return null;
  const values = points.map(point => point.value);
  return {
    current: values[values.length - 1], lowest: Math.min(...values), samples: values.length,
    spanHours: points.length > 1 ? (points[points.length - 1].time - points[0].time) / 3_600_000 : 0,
  };
});
const tokenValues = computed(() => daily.value.filter(item => item.tokens != null));
const tokenStats = computed(() => {
  const values = tokenValues.value.map(item => item.tokens ?? 0);
  if (!values.length) return null;
  const total = values.reduce((sum, value) => sum + value, 0);
  return { total, peak: Math.max(...values), average: total / values.length, activeDays: values.filter(value => value > 0).length };
});
function scheduleRender(delay = 100) {
  if (renderTimer) clearTimeout(renderTimer);
  renderTimer = setTimeout(() => { renderTimer = null; renderChart(); }, delay);
}
async function loadTokens() {
  const sequence = ++requestSequence;
  loading.value = true; error.value = ""; scheduleRender(0);
  try {
    const result = await invoke<AnalyticsData>("fetch_analytics", { days: tokenRange.value });
    if (sequence !== requestSequence) return;
    daily.value = Array.isArray(result.daily) ? result.daily : [];
    error.value = result.error ?? "";
  } catch (reason) {
    if (sequence !== requestSequence) return;
    daily.value = []; error.value = String(reason);
  } finally { if (sequence === requestSequence) loading.value = false; }
  scheduleRender(0);
}
function emptyGraphic(text: string, color: string) {
  return { type: "text", left: "center", top: "middle", style: { text, fill: color, fontSize: 13 } };
}
function renderChart() {
  if (!chart) return;
  const styles = chartElement.value ? getComputedStyle(chartElement.value) : null;
  const text = styles?.getPropertyValue("--text-secondary").trim() || "#8c8c92";
  const grid = styles?.getPropertyValue("--chart-grid").trim() || "rgba(128,128,128,.14)";
  const tooltip = { trigger: "axis", backgroundColor: "rgba(28,32,42,.96)", borderColor: grid, textStyle: { color: "#f5f7fb", fontSize: 12 } };
  if (tab.value === "tokens") {
    const values = tokenValues.value;
    chart.setOption({ animationDuration: 220, grid: { left: 58, right: 20, top: 24, bottom: 38 },
      tooltip: { ...tooltip, valueFormatter: (value: unknown) => compactNumber(Number(value)) },
      xAxis: { type: "category", data: values.map(item => item.date?.slice(5) ?? ""), axisLine: { lineStyle: { color: grid } }, axisTick: { show: false }, axisLabel: { color: text, fontSize: 11, hideOverlap: true } },
      yAxis: { type: "value", axisLabel: { color: text, fontSize: 11, formatter: (value: number) => compactNumber(value) }, splitLine: { lineStyle: { color: grid } } },
      series: values.length ? [{ name: "Token", type: "bar", data: values.map(item => item.tokens ?? 0), barMaxWidth: 24, itemStyle: { color: "#0a84ff", borderRadius: [4,4,0,0] } }] : [],
      graphic: values.length ? [] : [emptyGraphic(loading.value ? "正在加载 Token 活动…" : error.value || "还没有记录 Token 活动", text)],
    }, { notMerge: true });
    return;
  }
  const points = filteredQuotaPoints.value;
  chart.setOption({ animationDuration: 220, grid: { left: 52, right: 20, top: 24, bottom: 38 },
    tooltip: { ...tooltip, formatter: (params: unknown) => { const item = (params as Array<{ value: [number, number] }>)[0]; return item ? `${new Date(item.value[0]).toLocaleString("zh-CN", { hour12: false })}<br/><strong>剩余 ${Math.round(item.value[1])}%</strong>` : ""; } },
    xAxis: { type: "time", axisLine: { lineStyle: { color: grid } }, axisTick: { show: false }, axisLabel: { color: text, fontSize: 11, hideOverlap: true }, splitLine: { show: false } },
    yAxis: { type: "value", min: 0, max: 100, axisLabel: { color: text, fontSize: 11, formatter: "{value}%" }, splitLine: { lineStyle: { color: grid } } },
    series: points.length ? [{ name: "剩余额度", type: "line", data: points.map(point => [point.time, point.value]), showSymbol: points.length === 1, symbolSize: 7, smooth: .22, lineStyle: { color: "#0a84ff", width: 2 }, areaStyle: { color: "rgba(10,132,255,.14)" } }] : [],
    graphic: points.length ? [] : [emptyGraphic(refreshing.value ? "正在加载额度历史…" : quotaError.value ? "额度数据不可用" : "当前范围还没有额度记录", text)],
  }, { notMerge: true });
}
async function refreshQuotaHistory(): Promise<boolean> {
  if (refreshing.value) return false;
  refreshing.value = true;
  quotaError.value = "";
  scheduleRender(0);
  try {
    const latest = await invoke<UsageData>("fetch_usage");
    if (latest.error) throw new Error(latest.error);
    appendUsage(latest);
    scheduleRender(0);
    return true;
  } catch (reason) {
    reloadHistory();
    quotaError.value = String(reason);
    return false;
  } finally {
    refreshing.value = false;
    await nextTick();
    chart?.resize();
    scheduleRender(0);
  }
}
async function onHistoryWindowShown() {
  await nextTick();
  chart?.resize();
  await Promise.all([refreshQuotaHistory(), loadTokens()]);
}
async function refreshHistory() {
  if (refreshing.value) return;
  if (loading.value) return;
  actionMessage.value = "";
  const refreshed = await refreshQuotaHistory();
  await loadTokens();
  if (refreshed) {
    actionMessage.value = `已刷新：${new Date().toLocaleTimeString("zh-CN", { hour12: false })}`;
  } else {
    actionMessage.value = `刷新失败：${quotaError.value}`;
  }
}
async function exportCsv() {
  const rows = [["date", "five_hour_remaining_percent", "weekly_remaining_percent"]];
  for (const point of historyPoints.value) rows.push([new Date(point.t).toISOString(), point.fiveHour == null && point.p == null ? "" : String(point.fiveHour ?? point.p), point.weekly == null ? "" : String(point.weekly)]);
  actionMessage.value = "";
  try {
    const path = await save({
      defaultPath: "codex-usage-history.csv",
      filters: [{ name: "CSV 文件", extensions: ["csv"] }],
    });
    if (!path) return;
    await invoke("write_history_csv", {
      path,
      contents: `\uFEFF${rows.map(row => row.join(",")).join("\n")}`,
    });
    actionMessage.value = `已导出到 ${path}`;
  } catch (reason) {
    actionMessage.value = `导出失败：${String(reason)}`;
  }
}
function clearHistory() {
  if (!window.confirm("确定清除所有本地额度历史记录？此操作无法撤销。")) return;
  clearStoredHistory(); scheduleRender(0);
}
watch(tab, async () => { await nextTick(); scheduleRender(0); });
watch([quotaWindow, quotaRange], () => scheduleRender());
watch(tokenRange, () => void loadTokens());
onMounted(async () => {
  if (chartElement.value) { chart = init(chartElement.value); resizeObserver = new ResizeObserver(() => chart?.resize()); resizeObserver.observe(chartElement.value); }
  renderChart();
  unlistenHistoryShown = await listen("history-window-shown", () => void onHistoryWindowShown());
  if (await getCurrentWindow().isVisible()) void onHistoryWindowShown();
  await loadTokens();
});
onBeforeUnmount(() => { requestSequence += 1; unlistenHistoryShown?.(); if (renderTimer) clearTimeout(renderTimer); resizeObserver?.disconnect(); chart?.dispose(); chart = null; });
</script>

<template>
  <main class="history-view">
    <header class="topbar" data-tauri-drag-region @dblclick.self="toggleMaximizeWindow">
      <div class="title-block" data-tauri-drag-region @dblclick="toggleMaximizeWindow"><h1 class="page-title" data-tauri-drag-region>用量历史</h1><p class="page-subtitle" data-tauri-drag-region>查看额度变化与 Token 活动</p></div>
      <WindowControls />
    </header>
    <section class="workspace">
      <div v-if="actionMessage" class="action-message" role="status">{{ actionMessage }}</div>
      <div class="toolbar">
        <nav class="tabs" aria-label="历史类型">
          <button :class="{ active: tab === 'quota' }" @click="tab = 'quota'">额度历史</button>
          <button :class="{ active: tab === 'tokens' }" @click="tab = 'tokens'">Token 活动</button>
        </nav>
        <template v-if="tab === 'quota'">
          <div class="quota-controls">
            <label class="field"><span>额度窗口</span><select v-model="quotaWindow"><option value="fiveHour">5 小时</option><option value="weekly">每周</option></select></label>
            <div class="segmented" aria-label="额度历史范围">
              <button :class="{ active: quotaRange === 'current' }" @click="quotaRange = 'current'">当前周期</button>
              <button v-for="days in ([7, 14, 30] as const)" :key="days" :class="{ active: quotaRange === days }" @click="quotaRange = days">{{ days === 30 ? '最近一个月' : `最近 ${days} 天` }}</button>
            </div>
          </div>
        </template>
        <div v-else class="segmented token-range" aria-label="Token 活动范围"><button v-for="days in ([7,30,90,365,3650] as const)" :key="days" :class="{ active: tokenRange === days }" @click="tokenRange = days">{{ days === 3650 ? '全部' : days === 365 ? '1 年' : `${days} 天` }}</button></div>
        <span v-if="refreshing && tab === 'quota'" class="state-badge">正在加载额度历史…</span>
        <span v-else-if="quotaError && tab === 'quota'" class="state-badge warning" :title="quotaError">额度数据不可用</span>
        <span v-if="loading && tab === 'tokens'" class="state-badge">正在加载 Token 活动…</span>
        <span v-else-if="error && tab === 'tokens'" class="state-badge warning" :title="error">Token 活动不可用</span>
        <div class="topbar-actions">
          <button class="icon-button" :class="{ spinning: refreshing || loading }" :disabled="refreshing || loading" :title="loading ? '正在加载用量' : '获取最新用量'" :aria-label="loading ? '正在加载用量' : '获取最新用量'" @click="refreshHistory">↻</button>
          <button class="ghost" @click="exportCsv">导出 CSV</button><button class="ghost danger" @click="clearHistory">清除历史</button>
        </div>
      </div>
      <div v-if="tab === 'quota'" class="metric-grid">
        <article class="metric-card"><span>当前剩余</span><strong>{{ quotaStats ? `${Math.round(quotaStats.current)}%` : '—' }}</strong></article>
        <article class="metric-card"><span>最低剩余</span><strong>{{ quotaStats ? `${Math.round(quotaStats.lowest)}%` : '—' }}</strong></article>
        <article class="metric-card"><span>采样点数</span><strong>{{ quotaStats?.samples ?? 0 }}</strong></article>
        <article class="metric-card"><span>记录跨度</span><strong>{{ quotaStats ? `${quotaStats.spanHours.toFixed(1)}h` : '—' }}</strong></article>
      </div>
      <div v-else class="metric-grid">
        <article class="metric-card"><span>本范围总量</span><strong>{{ compactNumber(tokenStats?.total) }}</strong></article>
        <article class="metric-card"><span>本范围峰值</span><strong>{{ compactNumber(tokenStats?.peak) }}</strong></article>
        <article class="metric-card"><span>日均</span><strong>{{ compactNumber(tokenStats?.average) }}</strong></article>
        <article class="metric-card"><span>活跃天数</span><strong>{{ tokenStats?.activeDays ?? 0 }}</strong></article>
      </div>
      <article class="chart-card">
        <div class="chart-heading"><div><h2>{{ tab === 'quota' ? `${quotaWindow === 'weekly' ? '每周' : '5 小时'}剩余额度` : 'Token 使用趋势' }}</h2><p>{{ tab === 'quota' ? '百分比越低，表示当前周期剩余额度越少' : '按日期汇总的 Token 使用量' }}</p></div><span class="legend"><i></i>{{ tab === 'quota' ? '剩余额度' : 'Token' }}</span></div>
        <div ref="chart" class="chart-main" role="img" :aria-label="tab === 'quota' ? '剩余额度历史折线图' : 'Token 活动柱状图'"></div>
      </article>
    </section>
    <footer class="statusbar"><span>本地记录，不上传数据</span><span>{{ historyPoints.length }} 个额度采样点</span></footer>
  </main>
</template>

<style>
:root { color-scheme: light dark; --accent:#0a84ff; --bg:#f4f5f8; --surface:rgba(255,255,255,.9); --surface-solid:#fff; --surface-hover:rgba(25,35,55,.06); --border:rgba(20,28,42,.1); --text-primary:#1d2433; --text-secondary:#667085; --text-tertiary:#98a2b3; --chart-grid:rgba(20,28,42,.1); }
@media (prefers-color-scheme: dark) { :root { --bg:#17191f; --surface:rgba(38,41,51,.9); --surface-solid:#292c35; --surface-hover:rgba(255,255,255,.07); --border:rgba(255,255,255,.1); --text-primary:#f4f6fb; --text-secondary:#a9b0bf; --text-tertiary:#737b8b; --chart-grid:rgba(255,255,255,.1); } }
:root[data-ui-theme="light"] { color-scheme:light; --bg:#f4f5f8; --surface:rgba(255,255,255,.9); --surface-solid:#fff; --surface-hover:rgba(25,35,55,.06); --border:rgba(20,28,42,.1); --text-primary:#1d2433; --text-secondary:#667085; --text-tertiary:#98a2b3; --chart-grid:rgba(20,28,42,.1); }
:root[data-ui-theme="dark"] { color-scheme:dark; --bg:#17191f; --surface:rgba(38,41,51,.9); --surface-solid:#292c35; --surface-hover:rgba(255,255,255,.07); --border:rgba(255,255,255,.1); --text-primary:#f4f6fb; --text-secondary:#a9b0bf; --text-tertiary:#737b8b; --chart-grid:rgba(255,255,255,.1); }
* { box-sizing:border-box; } html,body,#app { width:100%; height:100%; margin:0; overflow:hidden; } body { background:var(--bg); } button,select { font:inherit; }
</style>
<style scoped>
.action-message {
  padding: 8px 10px;
  border: 1px solid rgba(10, 132, 255, 0.2);
  border-radius: 9px;
  background: rgba(10, 132, 255, 0.08);
  color: var(--text-secondary);
  font-size: 11px;
  word-break: break-all;
}

.icon-button.spinning { animation: refresh-spin 0.8s linear infinite; }
.icon-button:disabled { cursor: wait; opacity: 0.65; }
@keyframes refresh-spin { to { transform: rotate(360deg); } }
</style>
<style scoped>
.history-view{display:flex;flex-direction:column;height:100%;color:var(--text-primary);background:radial-gradient(circle at 16% 0%,rgba(10,132,255,.08),transparent 34%),var(--bg);font-family:Inter,system-ui,-apple-system,"Segoe UI","Microsoft YaHei",sans-serif}.topbar{display:grid;grid-template-columns:minmax(150px,1fr) auto minmax(230px,1fr) auto;align-items:center;gap:16px;padding:10px 10px 10px 20px;border-bottom:1px solid var(--border);background:color-mix(in srgb,var(--surface) 82%,transparent);backdrop-filter:blur(18px)}.title-block{min-width:0}.page-title{margin:0;font-size:18px;line-height:1.2}.page-subtitle{margin:3px 0 0;color:var(--text-secondary);font-size:11px}.tabs,.segmented{display:inline-flex;gap:2px;padding:3px;border-radius:10px;background:var(--surface-hover)}.tabs button,.segmented button{border:0;border-radius:7px;background:transparent;color:var(--text-secondary);cursor:pointer;font-size:12px;padding:6px 12px;white-space:nowrap;transition:.15s}.tabs button.active,.segmented button.active{color:var(--text-primary);background:var(--surface-solid);box-shadow:0 1px 4px rgba(16,24,40,.1);font-weight:600}.topbar-actions{display:flex;justify-content:flex-end;gap:8px}.window-controls{display:flex;align-self:stretch;margin:-10px -10px -10px 0}.window-controls button{width:44px;border:0;background:transparent;color:var(--text-secondary);cursor:pointer;font-size:15px}.window-controls button:hover{background:var(--surface-hover);color:var(--text-primary)}.window-controls .window-close:hover{background:#e81123;color:#fff}.ghost,.icon-button{border:1px solid var(--border);border-radius:8px;background:var(--surface);color:var(--text-primary);cursor:pointer;font-size:12px;padding:6px 11px}.ghost:hover,.icon-button:hover{background:var(--surface-hover)}.icon-button{width:31px;padding:0;font-size:16px}.danger{color:#ff453a}.workspace{display:flex;flex:1;flex-direction:column;min-height:0;gap:12px;padding:16px 20px 12px}.toolbar{display:flex;align-items:center;gap:12px;min-height:34px}.field{display:flex;align-items:center;gap:8px;color:var(--text-secondary);font-size:12px}.field select{border:1px solid var(--border);border-radius:8px;outline:0;background:var(--surface-solid);color:var(--text-primary);padding:6px 28px 6px 9px}.range-label{margin-left:auto;color:var(--text-tertiary);font-size:11px}.state-badge{color:var(--text-secondary);font-size:11px}.state-badge.warning{color:#ff9500}.metric-grid{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:10px}.metric-card{display:flex;flex-direction:column-reverse;gap:4px;min-width:0;padding:13px 15px;border:1px solid var(--border);border-radius:12px;background:var(--surface);box-shadow:0 4px 14px rgba(16,24,40,.035)}.metric-card span{color:var(--text-secondary);font-size:11px}.metric-card strong{overflow:hidden;color:var(--text-primary);font-size:21px;line-height:1.1;text-overflow:ellipsis;white-space:nowrap;font-variant-numeric:tabular-nums}.chart-card{display:flex;flex:1;flex-direction:column;min-height:0;padding:14px 16px 10px;border:1px solid var(--border);border-radius:14px;background:var(--surface);box-shadow:0 8px 28px rgba(16,24,40,.05)}.chart-heading{display:flex;align-items:flex-start;justify-content:space-between;gap:16px}.chart-heading h2{margin:0;font-size:14px}.chart-heading p{margin:4px 0 0;color:var(--text-secondary);font-size:11px}.legend{display:flex;align-items:center;gap:6px;color:var(--text-secondary);font-size:11px;white-space:nowrap}.legend i{width:8px;height:8px;border-radius:3px;background:var(--accent)}.chart-main{flex:1;width:100%;min-height:260px}.statusbar{display:flex;align-items:center;justify-content:space-between;padding:9px 20px;border-top:1px solid var(--border);color:var(--text-tertiary);background:color-mix(in srgb,var(--surface) 76%,transparent);font-size:11px}button:focus-visible,select:focus-visible{outline:2px solid var(--accent);outline-offset:2px}@media(max-width:760px){.topbar{grid-template-columns:1fr auto auto}.tabs{grid-column:1/-1;grid-row:2;justify-self:stretch}.tabs button{flex:1}.topbar-actions{grid-column:2;grid-row:1}.window-controls{grid-column:3;grid-row:1}.toolbar{align-items:flex-start;flex-wrap:wrap}.range-label{width:100%;margin-left:0}}@media(max-width:560px){.topbar,.workspace{padding-left:12px}.page-subtitle{display:none}.topbar-actions .ghost{padding-inline:8px}.metric-grid{grid-template-columns:repeat(2,minmax(0,1fr))}.segmented{max-width:100%;overflow-x:auto}.statusbar{padding-inline:12px}}
</style>
<style scoped>
.title-block {
  min-width: 0;
  display: flex;
  align-items: flex-end;
  gap: 10px;
}

.topbar {
  grid-template-columns: minmax(0, 1fr) auto;
}

.toolbar {
  gap: 6px;
  flex-wrap: nowrap;
  min-width: 0;
}

.toolbar .tabs,
.toolbar .field,
.toolbar .quota-controls,
.toolbar .topbar-actions {
  flex: 0 0 auto;
}

.toolbar .quota-controls {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  margin-left: auto;
}

.toolbar .tabs button,
.toolbar .segmented button {
  padding-inline: 7px;
}

.toolbar .segmented {
  min-width: 0;
}

.toolbar .token-range {
  margin-left: auto;
}

.toolbar .quota-controls .segmented {
  min-width: 0;
}

.toolbar .range-label {
  flex: 0 0 auto;
}

@media (max-width: 760px) {
  .topbar .window-controls {
    grid-column: 2;
    grid-row: 1;
  }

  .toolbar {
    align-items: flex-start;
    flex-wrap: wrap;
  }

  .toolbar .range-label {
    width: 100%;
    margin-left: 0;
  }

  .toolbar .quota-controls {
    flex-wrap: wrap;
    margin-left: 0;
  }

  .toolbar .topbar-actions {
    margin-left: auto;
  }
}
</style>
