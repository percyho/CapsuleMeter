import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// @tauri-apps/cli 开发模式下 Vite 默认端口
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [vue()],
  build: {
    rollupOptions: {
      input: {
        main: "index.html",
        history: "history.html",
      },
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
