import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Tauri 期望前端在固定端口，且使用相对 base 以兼容移动端
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [vue()],

  // Tauri 支持 Windows 上的 webview2，也支持 macOS/Linux 上的 webkit
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
      // 不监听 Rust 后端变化，避免不必要的重载
      ignored: ["**/src-tauri/**"],
    },
  },
}));
