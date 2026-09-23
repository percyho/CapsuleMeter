<script setup lang="ts">
import { computed, shallowRef } from "vue";
import type { ResetCredit, ResetCreditsSummary } from "../../types/usage";
import { useLocale } from "../../composables/useLocale";

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
const confirming = shallowRef(false);

const availableCredits = computed(() =>
  (props.summary?.credits ?? []).filter(
    (credit) => credit.status === "available" && (!credit.expiresAt || credit.expiresAt * 1000 > Date.now()),
  ),
);

const nextCredit = computed<ResetCredit | null>(() => availableCredits.value[0] ?? null);
const countLabel = computed(() => locale.value === "en-US"
  ? `${props.summary?.availableCount ?? 0} available`
  : `可用 ${props.summary?.availableCount ?? 0} 次`);

function formatExpiry(epoch: number | null | undefined): string {
  if (!epoch) return t("无过期时间");
  const date = new Date(epoch * 1000).toLocaleString(locale.value, {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
  return locale.value === "en-US" ? `Expires ${date}` : `将于 ${date} 到期`;
}

function requestConsume() {
  if (!confirming.value) {
    confirming.value = true;
    return;
  }
  emit("consume", nextCredit.value?.id ?? null);
  confirming.value = false;
}
</script>

<template>
  <section class="reset-card">
    <button
      class="reset-header"
      type="button"
      :aria-expanded="expanded"
      @click="expanded = !expanded"
    >
      <span class="reset-title">{{ t("使用限额重置") }}</span>
      <span class="reset-badge" :class="{ empty: !summary?.availableCount }">{{ countLabel }}</span>
      <span class="reset-chevron" :class="{ expanded }" aria-hidden="true">⌃</span>
    </button>

    <div v-if="expanded" class="reset-content">
      <div class="reset-copy">
        <strong>{{ t("完全重置（每周 + 5 小时）") }}</strong>
        <span v-if="summary?.availableCount">{{ formatExpiry(nextCredit?.expiresAt) }}</span>
        <span v-else-if="error">{{ error }}</span>
        <span v-else>{{ t("当前没有可用的限额重置机会") }}</span>
      </div>
      <button
        v-if="summary?.availableCount"
        class="reset-use"
        :class="{ confirm: confirming }"
        type="button"
        :disabled="loading"
        @click="requestConsume"
      >
        {{ loading ? t("处理中…") : confirming ? t("确认使用") : t("使用重置") }}
      </button>
      <button v-else class="reset-refresh" type="button" :disabled="loading" @click="emit('refresh')">
        {{ loading ? t("刷新中…") : t("刷新") }}
      </button>
    </div>
    <p v-if="confirming" class="reset-warning">{{ t("重置机会使用后不可撤销，再次点击确认。") }}</p>
  </section>
</template>

<style scoped>
.reset-card {
  border: 1px solid var(--panel-border, rgba(255, 255, 255, 0.12));
  border-radius: 12px;
  overflow: hidden;
}

.reset-header {
  display: grid;
  grid-template-columns: 1fr auto auto;
  align-items: center;
  gap: 8px;
  width: 100%;
  min-height: 42px;
  padding: 9px 11px;
  border: 0;
  background: transparent;
  color: var(--panel-text, #d8dce6);
  text-align: left;
  cursor: pointer;
}

.reset-title { font-size: 11px; font-weight: 650; }
.reset-badge { padding: 3px 8px; border-radius: 999px; background: rgba(73, 201, 126, 0.18); color: #70d79b; font-size: 9px; }
.reset-badge.empty { background: rgba(255, 255, 255, 0.08); color: var(--panel-val, #8f97a8); }
.reset-chevron { color: var(--panel-val, #8f97a8); transform: rotate(90deg); transition: transform 0.16s ease; }
.reset-chevron.expanded { transform: rotate(0deg); }
.reset-content { display: flex; align-items: center; gap: 10px; padding: 5px 11px 11px; }
.reset-copy { display: grid; gap: 3px; min-width: 0; flex: 1; }
.reset-copy strong { color: var(--panel-text, #d8dce6); font-size: 10px; font-weight: 600; }
.reset-copy span { color: var(--panel-val, #8f97a8); font-size: 9px; line-height: 1.35; }
.reset-use, .reset-refresh { flex: 0 0 auto; border: 0; border-radius: 8px; padding: 7px 10px; background: #202127; color: #fff; font-size: 9px; font-weight: 600; cursor: pointer; }
.reset-use.confirm { background: #c2414b; }
.reset-use:disabled, .reset-refresh:disabled { opacity: 0.55; cursor: wait; }
.reset-warning { margin: -4px 11px 10px; color: #e59a72; font-size: 8px; }
</style>
