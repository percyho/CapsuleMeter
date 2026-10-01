<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, shallowRef } from "vue";
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
const now = ref(Date.now());
const dayMs = 86_400_000;
let countdownTimer: number | undefined;

const expired = computed(
  () => (props.credit.expiresAt !== null && props.credit.expiresAt * 1000 <= now.value)
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
const title = computed(() => {
  if (props.credit.resetType.toLowerCase() === "codexratelimits") {
    return t("完全重置（每周 + 5 小时）");
  }
  return props.credit.title?.trim()
    ? t(props.credit.title.trim())
    : t("完全重置（每周 + 5 小时）");
});
const description = computed(() => {
  if (!props.credit.description?.trim()) return "";
  if (props.credit.resetType.toLowerCase() === "codexratelimits") {
    return t("重置每周与 5 小时限额");
  }
  return t(props.credit.description.trim());
});
const expirationRemainingMs = computed(() =>
  props.credit.expiresAt === null ? null : props.credit.expiresAt * 1000 - now.value,
);
const expirationCountdown = computed(() => {
  const remainingMs = expirationRemainingMs.value;
  if (remainingMs === null) return "";
  if (expired.value) return t("有效期已过");
  if (remainingMs < dayMs) return locale.value === "en-US"
    ? t("离有效期不到1天")
    : "离有效期仅剩不到1天";
  const days = Math.ceil(remainingMs / dayMs);
  return locale.value === "en-US"
    ? `Expires in ${days} ${days === 1 ? "day" : "days"}`
    : `离有效期${remainingMs <= 3 * dayMs ? "仅剩" : "剩余"}${days}天`;
});
const expirationCountdownClass = computed(() => {
  const remainingMs = expirationRemainingMs.value;
  if (remainingMs === null) return "";
  if (expired.value) return "critical";
  if (remainingMs <= 3 * dayMs) return "critical";
  if (remainingMs <= 7 * dayMs) return "warning";
  return "safe";
});

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

function requestConsume() {
  if (!available.value || props.loading) return;
  if (!confirming.value) {
    confirming.value = true;
    return;
  }
  emit("consume", props.credit.id);
  confirming.value = false;
}

onMounted(() => {
  countdownTimer = window.setInterval(() => {
    now.value = Date.now();
  }, 60_000);
});

onBeforeUnmount(() => {
  if (countdownTimer !== undefined) window.clearInterval(countdownTimer);
});
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
    <p v-if="description" class="reset-description">{{ description }}</p>
    <div class="reset-item-meta">
      <span>{{ t("获得时间") }} · {{ formatDate(credit.grantedAt) }}</span>
      <span v-if="credit.expiresAt !== null" class="reset-expiry">
        <span>{{ t("到期时间") }} · {{ formatDate(credit.expiresAt) }}</span>
        <span class="reset-countdown" :class="expirationCountdownClass">{{ expirationCountdown }}</span>
      </span>
      <span v-else>{{ t("无过期时间") }}</span>
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
.reset-expiry { display: inline-flex; flex-wrap: wrap; align-items: baseline; gap: 4px 8px; }
.reset-countdown { font-weight: 700; white-space: nowrap; }
.reset-countdown.safe { color: #1a7a40; }
.reset-countdown.warning { color: #956000; }
.reset-countdown.critical { color: #b42318; }
.reset-use { flex: 0 0 auto; border: 0; border-radius: 8px; padding: 7px 10px; background: #202127; color: #fff; font-size: 10px; font-weight: 600; cursor: pointer; }
.reset-use.confirm { background: #c2414b; }
.reset-use:disabled { opacity: 0.55; cursor: wait; }
.reset-use:focus-visible { outline: 2px solid var(--panel-accent, #6b8af0); outline-offset: 2px; }
.reset-warning { margin: 0; color: #e59a72; font-size: 10px; line-height: 1.4; }

@media (max-width: 380px) {
  .reset-item-heading { align-items: flex-start; flex-direction: column; }
}
</style>
