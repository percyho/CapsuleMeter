<script setup lang="ts">
import { capsuleThemeOptions, type CapsuleTheme } from "../../types/capsule-theme";

defineProps<{
  modelValue: CapsuleTheme;
  translate: (key: string) => string;
}>();

const emit = defineEmits<{
  select: [theme: CapsuleTheme];
}>();
</script>

<template>
  <section class="theme-panel" aria-labelledby="capsule-theme-heading">
    <header class="theme-heading">
      <div>
        <h2 id="capsule-theme-heading">{{ translate("胶囊主题") }}</h2>
        <p>{{ translate("选择胶囊的材质与读数风格") }}</p>
      </div>
      <span class="theme-count">6</span>
    </header>

    <div class="theme-grid">
      <button
        v-for="option in capsuleThemeOptions"
        :key="option.id"
        type="button"
        class="theme-card"
        :class="{ selected: modelValue === option.id }"
        :aria-pressed="modelValue === option.id"
        @click="emit('select', option.id)"
      >
        <span class="preview-stage" aria-hidden="true">
          <span class="preview-capsule" :class="`preview-${option.id}`">
            <span class="preview-half preview-left"><span class="preview-fill"></span><span class="preview-num">72</span></span>
            <span class="preview-half preview-right"><span class="preview-fill"></span><span class="preview-num">41</span></span>
          </span>
        </span>
        <span class="theme-copy">
          <strong>{{ translate(option.name) }}</strong>
          <small>{{ translate(option.description) }}</small>
        </span>
        <span class="selection-mark" aria-hidden="true">✓</span>
      </button>
    </div>
  </section>
</template>

<style scoped>
.theme-panel { padding: 2px 0 12px; }
.theme-heading { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; margin: 0 2px 12px; }
.theme-heading h2 { margin: 0; color: var(--panel-title, #f4f6fb); font-size: 14px; font-weight: 700; }
.theme-heading p { margin: 3px 0 0; color: var(--panel-val, #7f8799); font-size: 11px; line-height: 1.4; }
.theme-count { display: grid; place-items: center; min-width: 24px; height: 20px; border: 1px solid var(--panel-border, rgba(255,255,255,.1)); border-radius: 10px; color: var(--panel-val, #7f8799); font: 600 10px/1 "Segoe UI", sans-serif; }
.theme-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
.theme-card { position: relative; display: grid; grid-template-columns: 112px minmax(0, 1fr); align-items: center; gap: 10px; min-height: 78px; padding: 10px; border: 1px solid var(--panel-border, rgba(255,255,255,.1)); border-radius: 8px; background: var(--panel-control-bg, rgba(255,255,255,.04)); color: inherit; text-align: left; cursor: pointer; transition: transform .14s ease, border-color .16s ease, background-color .16s ease; }
.theme-card:hover { border-color: rgba(125, 152, 245, .45); background: var(--panel-control-active, rgba(255,255,255,.08)); }
.theme-card:active { transform: scale(.98); }
.theme-card:focus-visible { outline: 2px solid var(--panel-accent, #7d98f5); outline-offset: 2px; }
.theme-card.selected { border-color: var(--panel-accent, #7d98f5); background: rgba(91, 112, 205, .12); box-shadow: inset 0 0 0 1px rgba(125, 152, 245, .12); }
.preview-stage { display: grid; place-items: center; height: 50px; border-radius: 6px; background: transparent; overflow: hidden; }
.preview-capsule { position: relative; display: flex; width: 100px; height: 30px; overflow: hidden; border: 1px solid rgba(255,255,255,.2); border-radius: 999px; background: transparent; box-shadow: inset 0 1px rgba(255,255,255,.2), 0 3px 8px rgba(0,0,0,.3); }
.preview-half { position: relative; flex: 1; display: grid; place-items: center; overflow: hidden; }
.preview-half + .preview-half { border-left: 1px solid rgba(255,255,255,.16); }
.preview-fill { position: absolute; inset: 0; width: 100%; background: linear-gradient(90deg,#4f46e5,#9b87f5); }
.preview-right .preview-fill { inset: 0; width: 100%; background: linear-gradient(90deg,#ff956f,#e11d48); }
.preview-num { position: relative; z-index: 1; color: #f8fafc; font-size: 12px; font-weight: 750; font-variant-numeric: tabular-nums; text-shadow: 0 1px 2px #0008; }
.preview-realistic .preview-num { font-family:"Segoe UI Variable Display","Segoe UI",sans-serif; font-weight:700; letter-spacing:-.2px; }
.preview-pixel { height:34px; overflow:hidden; border:0; border-radius:0; padding:4px 4px 5px; background:#090b0e; clip-path:polygon(8% 0,92% 0,92% 12%,96% 12%,96% 24%,100% 24%,100% 76%,96% 76%,96% 88%,92% 88%,92% 100%,8% 100%,8% 88%,4% 88%,4% 76%,0 76%,0 24%,4% 24%,4% 12%,8% 12%); image-rendering:pixelated; filter:drop-shadow(0 3px 0 rgba(0,0,0,.52)); box-shadow:none; }
.preview-pixel::before { content:""; position:absolute; inset:5px auto auto 10px; z-index:4; width:28px; height:8px; pointer-events:none; background:linear-gradient(135deg,rgba(255,255,255,.92),rgba(207,237,255,.48) 46%,transparent 48%); clip-path:polygon(12% 0,100% 0,76% 100%,0 100%,0 45%); }
.preview-pixel .preview-half { background:#1e2430; }
.preview-pixel .preview-left { flex:52; clip-path:polygon(6% 0,100% 0,100% 100%,6% 100%,6% 88%,0 88%,0 12%,6% 12%); }
.preview-pixel .preview-right { flex:48; background:linear-gradient(180deg,#737b89 0,#4b525f 22%,#353c49 58%,#252a34 100%); clip-path:polygon(0 0,94% 0,94% 12%,100% 12%,100% 88%,94% 88%,94% 100%,0 100%); }
.preview-pixel .preview-fill { background-color:#287cf5; background-image:linear-gradient(180deg,#83c4ff 0,#4b97f8 22%,#2f7df4 62%,#255ed8 100%); box-shadow:inset 0 2px #b9e4ff,inset 0 -3px rgba(11,48,139,.52); }
.preview-pixel .preview-right .preview-fill { inset:0 auto 0 0; width:41%; background-color:#11151c; background-image:repeating-conic-gradient(#11151c 0 25%,#222833 0 50%); background-size:4px 4px; box-shadow:inset 0 1px rgba(255,255,255,.1),inset 0 -2px rgba(0,0,0,.42); }
.preview-pixel .preview-num { font-family:Arial,"Microsoft YaHei",sans-serif; font-size:14px; font-weight:800; letter-spacing:-.35px; text-shadow:1px 1px 0 rgba(14,20,33,.72); }
.preview-flat { border:0; background:transparent; box-shadow:none; }
.preview-flat .preview-fill { background:#3787f7; }
.preview-flat .preview-right .preview-fill { background:#111318; }
.preview-flat .preview-num { font-family:Arial,"Microsoft YaHei",sans-serif; font-weight:700; letter-spacing:0; text-shadow:none; }
.preview-skeuomorphic { border-color:#95a4bc; background:transparent; box-shadow:inset 0 2px 3px #fff8,inset 0 -3px 5px #0008,0 5px 9px #0006; }
.preview-skeuomorphic .preview-fill { background:linear-gradient(180deg,#b9efff,#2964f1 62%,#3420c8); }
.preview-skeuomorphic .preview-right .preview-fill { background:linear-gradient(180deg,#565e72,#171b28); }
.preview-skeuomorphic .preview-num { font-family:"Trebuchet MS","Microsoft YaHei",sans-serif; font-weight:700; letter-spacing:-.3px; text-shadow:0 2px 2px #0009; }
.preview-jelly { border-color:#91eaff; background:transparent; box-shadow:inset 0 3px 3px #fff9,inset 0 -3px 5px #1634a099,0 0 8px #4acfff77; }
.preview-jelly .preview-fill { background:linear-gradient(90deg,#3687ff,#736dff); }
.preview-jelly .preview-right .preview-fill { background:linear-gradient(90deg,#8b62ff,#f244a9); }
.preview-jelly .preview-num { font-family:"Arial Rounded MT Bold","Segoe UI",sans-serif; font-weight:700; letter-spacing:-.35px; text-shadow:0 1px 2px #3a2180aa; }
.preview-neon { border:2px solid transparent; background:linear-gradient(transparent,transparent) padding-box,linear-gradient(100deg,#42dfff 0%,#985cff 50%,#ff62c7 100%) border-box; box-shadow:-2px 0 7px #42dfff99,2px 0 7px #ff62c788,0 0 12px #7a35ff, inset 0 0 8px #131c55; }
.preview-neon::after { content:""; position:absolute; inset:0 49% 0 auto; z-index:3; width:1px; background:#f06bff; box-shadow:0 0 5px #e053ff; }
.preview-neon .preview-fill { background:linear-gradient(90deg,#073d72,#302070); box-shadow:inset 0 0 8px #35dcff55; }
.preview-neon .preview-right .preview-fill { background:linear-gradient(90deg,#38196d,#711452); box-shadow:inset 0 0 8px #ff4fcb55; }
.preview-neon .preview-num { color:#fff; font-family:Bahnschrift,"Arial Narrow","Segoe UI",sans-serif; font-weight:600; letter-spacing:.35px; text-shadow:0 0 4px #86eaff,0 0 8px #945cff; }
.theme-copy { display: grid; min-width: 0; gap: 3px; }
.theme-copy strong { color: var(--panel-title, #f4f6fb); font-size: 12px; font-weight: 650; }
.theme-copy small { color: var(--panel-val, #7f8799); font-size: 10px; line-height: 1.35; text-wrap: pretty; }
.selection-mark { position: absolute; top: 7px; right: 7px; display: grid; place-items: center; width: 16px; height: 16px; border-radius: 50%; background: var(--panel-accent, #7d98f5); color: white; font-size: 10px; opacity: 0; transform: scale(.8); transition: opacity .14s ease, transform .14s ease; }
.selected .selection-mark { opacity: 1; transform: scale(1); }
@media (max-width: 520px) { .theme-grid { grid-template-columns: 1fr; } }
@media (prefers-reduced-motion: reduce) { .theme-card, .selection-mark { transition-duration: .01ms; } }
</style>
