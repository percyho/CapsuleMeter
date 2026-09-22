<script setup lang="ts">
import { useWindowControls } from "../../composables/useWindowControls";

const { minimizeWindow, toggleMaximizeWindow, closeWindow } = useWindowControls();
</script>

<template>
  <div class="window-controls" aria-label="窗口控制">
    <button title="最小化" aria-label="最小化" @click="minimizeWindow">—</button>
    <button title="最大化或还原" aria-label="最大化或还原" @click="toggleMaximizeWindow">□</button>
    <button class="window-close" title="关闭" aria-label="关闭" @click="closeWindow">×</button>
  </div>
</template>

<style scoped>
.window-controls { display: flex; align-self: stretch; margin: -10px -10px -10px 0; }
.window-controls button { width: 44px; border: 0; background: transparent; color: var(--text-secondary); cursor: pointer; font-size: 15px; }
.window-controls button:hover { background: var(--surface-hover); color: var(--text-primary); }
.window-controls .window-close:hover { background: #e81123; color: #fff; }
.window-controls button:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
</style>
