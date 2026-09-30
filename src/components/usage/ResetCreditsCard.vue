<script setup lang="ts">
import { computed, shallowRef } from "vue";
import type { ResetCreditsSummary } from "../../types/usage";
import { useLocale } from "../../composables/useLocale";
import ResetCreditItem from "./ResetCreditItem.vue";

const { locale, t } = useLocale();

const props = defineProps<{
  summary: ResetCreditsSummary | null;
  loading: boolean;
  error: string | null;
}>();

const emit = defineEmits<{
  refresh: [];
  consume: [creditId: string | null];
}>();

const expanded = shallowRef(true);
const confirmingFallback = shallowRef(false);

const credits = computed(() =>
  [...(props.summary?.credits ?? [])].sort((a, b) => b.grantedAt - a.grantedAt),
);
const canUseFallback = computed(
  () => credits.value.length === 0 && (props.summary?.availableCount ?? 0) > 0,
);
const hasResetContent = computed(() => {
  if (props.error || !props.summary) return true;
  return credits.value.length > 0 || canUseFallback.value;
});
const countLabel = computed(() =>
  locale.value === "en-US"
    ? `${props.summary?.availableCount ?? 0} available`
    : `可用 ${props.summary?.availableCount ?? 0} 次`,
);
const errorMessage = computed(() => {
  let message = props.error ?? "";
  const prefix = locale.value === "en-US"
    ? ["读取重置机会失败：", "Failed to load reset credits: "] as const
    : ["Failed to load reset credits: ", "读取重置机会失败："] as const;
  if (message.startsWith(prefix[0])) {
    message = `${t(prefix[0])}${message.slice(prefix[0].length)}`;
  }
  const suffix = locale.value === "en-US"
    ? ["；再次确认将安全重试本次操作", "; Confirm again to safely retry this operation"] as const
    : ["; Confirm again to safely retry this operation", "；再次确认将安全重试本次操作"] as const;
  if (message.endsWith(suffix[0])) {
    message = `${message.slice(0, -suffix[0].length)}${t(suffix[0])}`;
  }
  return message;
});

function requestFallbackConsume() {
  if (!confirmingFallback.value) {
    confirmingFallback.value = true;
    return;
  }
  emit("consume", null);
  confirmingFallback.value = false;
}
</script>

<template>
  <section class="reset-card">
    <div class="reset-header">
      <button
        class="reset-toggle"
        type="button"
        :aria-expanded="expanded"
        @click="expanded = !expanded"
      >
        <span class="reset-title">{{ t("使用限额重置") }}</span>
        <span class="reset-badge" :class="{ empty: !summary?.availableCount }">{{ countLabel }}</span>
        <span class="reset-chevron" :class="{ expanded }" aria-hidden="true">⌃</span>
      </button>
      <button
        class="reset-refresh"
        type="button"
        :class="{ spinning: loading }"
        :disabled="loading"
        :title="loading ? t('刷新中…') : t('刷新')"
        :aria-label="loading ? t('刷新中…') : t('刷新')"
        @click="emit('refresh')"
      >
        <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M20 11a8.1 8.1 0 0 0-15.5-2M4 4v5h5" />
          <path d="M4 13a8.1 8.1 0 0 0 15.5 2M20 20v-5h-5" />
        </svg>
      </button>
    </div>

    <div v-if="expanded && hasResetContent" class="reset-content">
      <p v-if="errorMessage" class="reset-message error" role="alert">{{ errorMessage }}</p>
      <p v-if="loading && !summary" class="reset-message">{{ t("加载中…") }}</p>

      <ResetCreditItem
        v-for="credit in credits"
        :key="credit.id"
        :credit="credit"
        :loading="loading"
        @consume="emit('consume', $event)"
      />

      <section v-if="canUseFallback" class="reset-fallback">
        <div class="reset-fallback-copy">
          <strong>{{ t("完全重置（每周 + 5 小时）") }}</strong>
          <span>{{ t("重置卡详情暂不可用") }}</span>
        </div>
        <button
          class="reset-use"
          :class="{ confirm: confirmingFallback }"
          type="button"
          :disabled="loading"
          @click="requestFallbackConsume"
        >
          {{ loading ? t("处理中…") : confirmingFallback ? t("确认使用") : t("使用重置") }}
        </button>
        <p v-if="confirmingFallback" class="reset-warning">
          {{ t("重置机会使用后不可撤销，再次点击确认。") }}
        </p>
      </section>

      <p v-if="!summary && !loading && !error" class="reset-message">
        {{ t("重置机会列表尚未加载") }}
      </p>
    </div>
  </section>
</template>

<style scoped>
.reset-card {
  border: 1px solid var(--panel-border, rgba(255, 255, 255, 0.12));
  border-radius: 12px;
  overflow: hidden;
}

.reset-header {
  display: flex;
  align-items: center;
  width: 100%;
  min-height: 42px;
  padding-right: 5px;
}

.reset-toggle {
  display: grid;
  grid-template-columns: 1fr auto auto;
  align-items: center;
  gap: 8px;
  min-width: 0;
  min-height: 42px;
  padding: 9px 6px 9px 11px;
  flex: 1;
  border: 0;
  background: transparent;
  color: var(--panel-text, #d8dce6);
  text-align: left;
  cursor: pointer;
}

.reset-title { font-size: 13px; font-weight: 650; }
.reset-badge { padding: 3px 8px; border-radius: 999px; background: rgba(73, 201, 126, 0.18); color: #1a7a40; font-size: 10px; font-weight: 700; }
.reset-badge.empty { background: rgba(255, 255, 255, 0.08); color: var(--panel-val, #8f97a8); }
.reset-chevron { color: var(--panel-val, #8f97a8); transform: rotate(90deg); transition: transform 0.16s ease; }
.reset-chevron.expanded { transform: rotate(0deg); }
.reset-content { display: grid; gap: 8px; padding: 8px; }
.reset-message { margin: 0; padding: 10px; border-radius: 8px; color: var(--panel-val, #8f97a8); font-size: 11px; line-height: 1.45; }
.reset-message.error { color: #ff9f8f; background: rgba(194, 65, 75, 0.12); overflow-wrap: anywhere; }
.reset-fallback { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; padding: 10px; border: 1px solid var(--panel-border, rgba(255, 255, 255, 0.12)); border-radius: 10px; }
.reset-fallback-copy { display: grid; gap: 3px; min-width: 0; flex: 1; }
.reset-fallback-copy strong { color: var(--panel-text, #d8dce6); font-size: 12px; font-weight: 600; }
.reset-fallback-copy span { color: var(--panel-val, #8f97a8); font-size: 10px; line-height: 1.35; }
.reset-use { flex: 0 0 auto; border: 0; border-radius: 8px; padding: 7px 10px; background: #202127; color: #fff; font-size: 10px; font-weight: 600; cursor: pointer; }
.reset-use.confirm { background: #c2414b; }
.reset-use:disabled { opacity: 0.55; cursor: wait; }
.reset-warning { flex-basis: 100%; margin: 0; color: #e59a72; font-size: 10px; }
.reset-refresh { display: grid; place-items: center; flex: 0 0 34px; width: 34px; height: 34px; padding: 0; border: 0; border-radius: 7px; background: transparent; color: var(--panel-val, #8f97a8); cursor: pointer; }
.reset-refresh:hover:not(:disabled) { background: rgba(255, 255, 255, 0.07); color: var(--panel-text, #d8dce6); }
.reset-refresh:active:not(:disabled) { transform: scale(0.97); }
.reset-refresh:focus-visible,.reset-toggle:focus-visible,.reset-use:focus-visible { outline: 2px solid var(--panel-accent, #6b8af0); outline-offset: -2px; }
.reset-refresh:disabled { opacity: 0.55; cursor: wait; }
.reset-refresh svg { width: 15px; height: 15px; }
.reset-refresh.spinning svg { animation: reset-spin 0.8s linear infinite; }

@keyframes reset-spin { to { transform: rotate(360deg); } }

@media (prefers-reduced-motion: reduce) {
  .reset-refresh.spinning svg { animation: none; }
}
</style>
