<script setup lang="ts">
import { BarChart, LineChart } from "echarts/charts";
import { GraphicComponent, GridComponent, TooltipComponent } from "echarts/components";
import { init, use, type ECharts } from "echarts/core";
import { CanvasRenderer } from "echarts/renderers";
import { invoke } from "@tauri-apps/api/core";
import { computed, nextTick, onBeforeUnmount, onMounted, shallowRef, useTemplateRef, watch } from "vue";
import type { HistoryPoint } from "../../types/usage";
import { compactNumber } from "../../utils/formatters";

use([BarChart, LineChart, GraphicComponent, GridComponent, TooltipComponent, CanvasRenderer]);

interface DailyUsage {
  date?: string;
  tokens?: number;
}

interface AnalyticsData {
  daily: DailyUsage[];
  total_tokens: number | null;
  peak_tokens: number | null;
  error: string | null;
}

type ViewTab = "quota" | "tokens";
type QuotaWindow = "fiveHour" | "weekly";
type QuotaRange = 1 | 7 | 14 | 30;
type TokenRange = 7 | 30 | 90 | 365 | 3650;

const props = defineProps<{ historyPoints: HistoryPoint[]; uiTheme: "dark" | "light" | "system" }>();
const emit = defineEmits<{ openHistory: [] }>();

const activeView = shallowRef<ViewTab>("quota");
const quotaWindow = shallowRef<QuotaWindow>("weekly");
const quotaRange = shallowRef<QuotaRange>(7);
const tokenRange = shallowRef<TokenRange>(30);
const analytics = shallowRef<AnalyticsData | null>(null);
const loading = shallowRef(false);
const error = shallowRef("");
const chartContainer = useTemplateRef<HTMLDivElement>("chart");

let chart: ECharts | null = null;
let resizeObserver: ResizeObserver | null = null;
let renderTimer: ReturnType<typeof setTimeout> | null = null;
let requestSequence = 0;

const quotaPoints = computed(() => {
  const cutoff = Date.now() - quotaRange.value * 86_400_000;
  return props.historyPoints
    .filter((point) => point.t >= cutoff)
    .map((point) => ({
      time: point.t,
      value: quotaWindow.value === "weekly" ? point.weekly : (point.fiveHour ?? point.p),
    }))
    .filter((point): point is { time: number; value: number } => point.value != null);
});

const quotaStats = computed(() => {
  const values = quotaPoints.value.map((point) => point.value);
  if (!values.length) return null;
  return {
    current: values[values.length - 1] ?? 0,
    lowest: Math.min(...values),
    samples: values.length,
    span: quotaPoints.value.length > 1
      ? (quotaPoints.value[quotaPoints.value.length - 1].time - quotaPoints.value[0].time) / 3_600_000
      : 0,
  };
});

const tokenBuckets = computed(() => analytics.value?.daily.filter((item) => item.tokens != null) ?? []);
const tokenStats = computed(() => {
  const values = tokenBuckets.value.map((item) => item.tokens ?? 0);
  if (!values.length) return null;
  const total = values.reduce((sum, value) => sum + value, 0);
  return {
    total,
    peak: Math.max(...values),
    average: total / values.length,
    activeDays: values.filter((value) => value > 0).length,
  };
});

function scheduleRender(delay = 120) {
  if (renderTimer) clearTimeout(renderTimer);
  renderTimer = setTimeout(() => {
    renderTimer = null;
    renderChart();
  }, delay);
}

async function loadAnalytics() {
  const sequence = ++requestSequence;
  loading.value = true;
  error.value = "";
  try {
    const result = await invoke<AnalyticsData>("fetch_analytics", { days: tokenRange.value });
    if (sequence !== requestSequence) return;
    analytics.value = result;
    error.value = result.error ?? "";
  } catch (reason) {
    if (sequence !== requestSequence) return;
    analytics.value = null;
    error.value = String(reason);
  } finally {
    if (sequence === requestSequence) loading.value = false;
  }
  scheduleRender(0);
}

function renderChart() {
  if (!chart) return;
  const styles = chartContainer.value ? getComputedStyle(chartContainer.value) : null;
  const textColor = styles?.getPropertyValue("--panel-val").trim() || "#8991a3";
  const gridColor = "rgba(135, 145, 165, 0.14)";
  const tooltip = {
    trigger: "axis",
    backgroundColor: "rgba(26, 29, 38, 0.96)",
    borderColor: "rgba(255,255,255,0.12)",
    textStyle: { color: "#eef1f7", fontSize: 11 },
  };

  if (activeView.value === "quota") {
    const points = quotaPoints.value;
    chart.setOption({
      animation: false,
      grid: { left: 38, right: 10, top: 18, bottom: 28 },
      tooltip: { ...tooltip, valueFormatter: (value: unknown) => `${Math.round(Number(value))}%` },
      xAxis: {
        type: "time",
        axisLine: { lineStyle: { color: gridColor } },
        axisLabel: { color: textColor, fontSize: 9 },
        splitLine: { show: false },
      },
      yAxis: {
        type: "value",
        min: 0,
        max: 100,
        axisLabel: { color: textColor, fontSize: 9, formatter: "{value}%" },
        splitLine: { lineStyle: { color: gridColor } },
      },
      series: points.length ? [{
        name: "剩余额度",
        type: "line",
        data: points.map((point) => [point.time, point.value]),
        showSymbol: false,
        smooth: 0.22,
        lineStyle: { color: "#0a84ff", width: 2 },
        areaStyle: { color: "rgba(10,132,255,0.16)" },
      }] : [],
      graphic: points.length ? [] : [{
        type: "text",
        left: "center",
        top: "middle",
        style: { text: "还没有记录额度历史", fill: textColor, fontSize: 11 },
      }],
    }, { notMerge: true });
    return;
  }

  const buckets = tokenBuckets.value;
  chart.setOption({
    animation: false,
    grid: { left: 44, right: 10, top: 18, bottom: 28 },
    tooltip: { ...tooltip, valueFormatter: (value: unknown) => compactNumber(Number(value)) },
    xAxis: {
      type: "category",
      data: buckets.map((item) => item.date?.slice(5) ?? ""),
      axisLine: { lineStyle: { color: gridColor } },
      axisLabel: { color: textColor, fontSize: 9, hideOverlap: true },
      splitLine: { show: false },
    },
    yAxis: {
      type: "value",
      axisLabel: { color: textColor, fontSize: 9, formatter: (value: number) => compactNumber(value) },
      splitLine: { lineStyle: { color: gridColor } },
    },
    series: buckets.length ? [{
      name: "Token",
      type: "bar",
      data: buckets.map((item) => item.tokens ?? 0),
      barMaxWidth: 18,
      itemStyle: { color: "#0a84ff", borderRadius: [3, 3, 0, 0] },
    }] : [],
    graphic: buckets.length || loading.value ? [] : [{
      type: "text",
      left: "center",
      top: "middle",
      style: { text: error.value || "还没有记录 Token 活动", fill: textColor, fontSize: 11 },
    }],
  }, { notMerge: true });
}

watch([activeView, quotaWindow, quotaRange, () => props.historyPoints, () => props.uiTheme], async () => {
  await nextTick();
  scheduleRender();
});

watch(tokenRange, () => void loadAnalytics());

onMounted(async () => {
  if (chartContainer.value) {
    chart = init(chartContainer.value);
    resizeObserver = new ResizeObserver(() => chart?.resize());
    resizeObserver.observe(chartContainer.value);
  }
  renderChart();
  await loadAnalytics();
});

onBeforeUnmount(() => {
  requestSequence += 1;
  if (renderTimer) clearTimeout(renderTimer);
  resizeObserver?.disconnect();
  chart?.dispose();
  chart = null;
});
</script>

<template>
  <section class="stats-panel">
    <div class="stats-header">
      <div class="view-tabs" role="tablist" aria-label="统计类型">
        <button :class="{ active: activeView === 'quota' }" role="tab" :aria-selected="activeView === 'quota'" @click="activeView = 'quota'">额度历史</button>
        <button :class="{ active: activeView === 'tokens' }" role="tab" :aria-selected="activeView === 'tokens'" @click="activeView = 'tokens'">Token 活动</button>
      </div>
      <button class="detail-button" @click="emit('openHistory')">详细历史 ↗</button>
    </div>

    <template v-if="activeView === 'quota'">
      <div class="control-row">
        <div class="segmented" aria-label="额度窗口">
          <button :class="{ active: quotaWindow === 'fiveHour' }" @click="quotaWindow = 'fiveHour'">5 小时</button>
          <button :class="{ active: quotaWindow === 'weekly' }" @click="quotaWindow = 'weekly'">每周</button>
        </div>
        <div class="segmented range-segmented" aria-label="额度范围">
          <button v-for="days in ([1, 7, 14, 30] as QuotaRange[])" :key="days" :class="{ active: quotaRange === days }" @click="quotaRange = days">{{ days === 1 ? '当前' : `${days}天` }}</button>
        </div>
      </div>
      <div class="metric-grid">
        <div class="metric"><strong>{{ quotaStats ? `${Math.round(quotaStats.current)}%` : '—' }}</strong><span>当前剩余</span></div>
        <div class="metric"><strong>{{ quotaStats ? `${Math.round(quotaStats.lowest)}%` : '—' }}</strong><span>最低剩余</span></div>
        <div class="metric"><strong>{{ quotaStats?.samples ?? 0 }}</strong><span>采样点数</span></div>
        <div class="metric"><strong>{{ quotaStats ? `${quotaStats.span.toFixed(1)}h` : '—' }}</strong><span>记录跨度</span></div>
      </div>
    </template>

    <template v-else>
      <div class="control-row">
        <div class="segmented token-ranges" aria-label="Token 时间范围">
          <button v-for="option in ([7, 30, 90, 365, 3650] as TokenRange[])" :key="option" :class="{ active: tokenRange === option }" @click="tokenRange = option">{{ option === 3650 ? '全部' : option === 365 ? '1年' : `${option}天` }}</button>
        </div>
        <span v-if="loading" class="status-text">加载中…</span>
        <span v-else-if="error" class="status-text error" :title="error">Token 活动不可用</span>
      </div>
      <div class="metric-grid">
        <div class="metric"><strong>{{ tokenStats ? compactNumber(tokenStats.total) : '—' }}</strong><span>本范围总量</span></div>
        <div class="metric"><strong>{{ tokenStats ? compactNumber(tokenStats.peak) : '—' }}</strong><span>本范围峰值</span></div>
        <div class="metric"><strong>{{ tokenStats ? compactNumber(tokenStats.average) : '—' }}</strong><span>日均</span></div>
        <div class="metric"><strong>{{ tokenStats?.activeDays ?? 0 }}</strong><span>活跃天数</span></div>
      </div>
    </template>

    <div ref="chart" class="stats-chart" role="img" :aria-label="activeView === 'quota' ? '额度历史趋势图' : 'Token 活动柱状图'"></div>
    <p class="privacy-note">数据保存在本机，统计图不会上传历史记录。</p>
  </section>
</template>

<style scoped>
.stats-panel { display: flex; flex-direction: column; gap: 10px; min-height: 100%; }
.stats-header, .control-row { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.view-tabs, .segmented { display: inline-flex; gap: 2px; padding: 2px; border-radius: 9px; background: rgba(255,255,255,0.06); }
.view-tabs button, .segmented button { border: 0; border-radius: 7px; background: transparent; color: var(--panel-val, #8f97a8); cursor: pointer; font-size: 10px; padding: 5px 8px; white-space: nowrap; }
.view-tabs button.active, .segmented button.active { color: var(--panel-text, #f4f6fb); background: rgba(255,255,255,0.1); box-shadow: 0 1px 3px rgba(0,0,0,0.18); font-weight: 600; }
.detail-button { border: 0; background: transparent; color: var(--panel-accent, #7d98f5); cursor: pointer; font-size: 10px; padding: 4px; }
.range-segmented button { padding-inline: 6px; }
.token-ranges { max-width: 100%; overflow-x: auto; }
.metric-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 6px; }
.metric { min-width: 0; padding: 8px 5px; border: 1px solid rgba(255,255,255,0.07); border-radius: 9px; background: rgba(255,255,255,0.04); text-align: center; }
.metric strong { display: block; overflow: hidden; color: var(--panel-title, #f4f6fb); font-size: 14px; line-height: 1.15; text-overflow: ellipsis; white-space: nowrap; }
.metric span { display: block; margin-top: 3px; color: var(--panel-val, #8f97a8); font-size: 8px; white-space: nowrap; }
.stats-chart { width: 100%; min-height: 150px; flex: 1; }
.status-text { margin-left: auto; color: var(--panel-val, #8f97a8); font-size: 9px; }
.status-text.error { color: #ff9f43; }
.privacy-note { margin: 0; padding-top: 7px; border-top: 1px solid rgba(255,255,255,0.07); color: var(--panel-val, #8f97a8); font-size: 9px; text-align: center; }
button:focus-visible { outline: 2px solid var(--panel-accent, #6b8af0); outline-offset: 2px; }
@media (max-width: 340px) { .metric-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); } .control-row { align-items: flex-start; flex-direction: column; } }
</style>
