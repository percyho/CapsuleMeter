<script setup lang="ts">
import { computed, shallowRef } from "vue";
import type { ResetCredit } from "../../types/usage";
import { useLocale } from "../../composables/useLocale";

const props = defineProps<{
  credit: ResetCredit;
  loading: boolean;
}>();

const emit = defineEmits<{
  consume: [creditId: string];
}>();

const { locale, t } = useLocale();
const confirming = shallowRef(false);

const expired = computed(
  () => (props.credit.expiresAt !== null && props.credit.expiresAt * 1000 <= Date.now())
    || props.credit.status.toLowerCase() === "expired",
);
const available = computed(
  () => props.credit.status.toLowerCase() === "available" && !expired.value,
);
const statusLabel = computed(() => {
  if (expired.value) return t("已过期");
  if (available.value) return t("可用");
  if (["redeemed", "used", "consumed"].includes(props.credit.status.toLowerCase())) {
    return t("已使用");
  }
  return t("不可用");
});
const statusClass = computed(() => {
  if (expired.value) return "expired";
  if (available.value) return "available";
  if (["redeemed", "used", "consumed"].includes(props.credit.status.toLowerCase())) return "used";
  return "unavailable";
});
const title = computed(() =>
  props.credit.title?.trim() || t("完全重置（每周 + 5 小时）"),
);

function formatDate(epoch: number): string {
  return new Date(epoch * 1000).toLocaleString(locale.value, {
    year: "numeric",
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hour12: locale.value === "en-US",
  });
}

function formatExpiry(epoch: number | null): string {
  return epoch !== null ? `${t("到期时间")} · ${formatDate(epoch)}` : t("无过期时间");
}

function requestConsume() {
  if (!available.value || props.loading) return;
  if (!confirming.value) {
    confirming.value = true;
    return;
  }
  emit("consume", props.credit.id);
  confirming.value = false;
}
</script>

<template>
  <article class="reset-item" :class="{ disabled: !available }">
    <div class="reset-item-heading">
      <div class="reset-item-title">
        <strong>{{ title }}</strong>
        <span class="reset-status" :class="statusClass">{{ statusLabel }}</span>
      </div>
      <button
        v-if="available"
        class="reset-use"
        :class="{ confirm: confirming }"
        type="button"
        :disabled="loading"
        @click="requestConsume"
      >
        {{ loading ? t("处理中…") : confirming ? t("确认使用") : t("使用重置") }}
      </button>
    </div>
    <p v-if="credit.description" class="reset-description">{{ credit.description }}</p>
    <div class="reset-item-meta">
      <span>{{ t("获得时间") }} · {{ formatDate(credit.grantedAt) }}</span>
      <span>{{ formatExpiry(credit.expiresAt) }}</span>
    </div>
    <p v-if="confirming" class="reset-warning" role="status">
      {{ t("重置机会使用后不可撤销，再次点击确认。") }}
    </p>
  </article>
</template>

<style scoped>
.reset-item { display: grid; gap: 7px; padding: 10px; border: 1px solid var(--panel-border, rgba(255, 255, 255, 0.12)); border-radius: 10px; background: rgba(255, 255, 255, 0.025); }
.reset-item.disabled { opacity: 0.78; }
.reset-item-heading { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
.reset-item-title { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; min-width: 0; }
.reset-item-title strong { color: var(--panel-text, #d8dce6); font-size: 12px; font-weight: 600; overflow-wrap: anywhere; }
.reset-status { padding: 3px 7px; border-radius: 999px; font-size: 10px; font-weight: 700; white-space: nowrap; }
.reset-status.available { color: #1a7a40; background: rgba(73, 201, 126, 0.18); }
.reset-status.used,.reset-status.expired,.reset-status.unavailable { color: var(--panel-val, #8f97a8); background: rgba(255, 255, 255, 0.08); }
.reset-description { margin: 0; color: var(--panel-val, #8f97a8); font-size: 10px; line-height: 1.45; overflow-wrap: anywhere; }
.reset-item-meta { display: flex; flex-wrap: wrap; gap: 4px 14px; color: var(--panel-val, #8f97a8); font-size: 10px; line-height: 1.4; }
.reset-use { flex: 0 0 auto; border: 0; border-radius: 8px; padding: 7px 10px; background: #202127; color: #fff; font-size: 10px; font-weight: 600; cursor: pointer; }
.reset-use.confirm { background: #c2414b; }
.reset-use:disabled { opacity: 0.55; cursor: wait; }
.reset-use:focus-visible { outline: 2px solid var(--panel-accent, #6b8af0); outline-offset: 2px; }
.reset-warning { margin: 0; color: #e59a72; font-size: 10px; line-height: 1.4; }

@media (max-width: 380px) {
  .reset-item-heading { align-items: flex-start; flex-direction: column; }
}
</style>
