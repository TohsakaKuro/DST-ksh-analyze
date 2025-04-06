import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [vue()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
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
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },

  // Monaco Editor 配置
  optimizeDeps: {
    include: ['monaco-editor/esm/vs/editor/editor.worker'],
  },
  build: {
    // 启用最小化
    minify: 'terser',
    terserOptions: {
      compress: {
        // 移除console和debugger
        drop_console: true,
        drop_debugger: true
      }
    },
    // 启用 CSS 代码分割
    cssCodeSplit: true,
    // 配置 rollup 选项
    rollupOptions: {
      output: {
        // 控制代码分割
        manualChunks: {
          // 只保留核心编辑器功能
          'monaco-editor': ['monaco-editor/esm/vs/editor/editor.api'],
          // 将 worker 分开打包
          'editor-worker': ['monaco-editor/esm/vs/editor/editor.worker'],
          // Tauri API 单独打包
          'tauri': ['@tauri-apps/api'],
          // Tauri 插件单独打包
          'tauri-plugins': [
            '@tauri-apps/plugin-dialog',
            '@tauri-apps/plugin-fs',
            '@tauri-apps/plugin-opener'
          ],
          // Vue 相关
          'vue': ['vue'],
          // 其他依赖
          'vendor': ['prismjs']
        },
        // 优化静态资源
        assetFileNames: (assetInfo) => {
          if (assetInfo.name.endsWith('.css')) {
            return 'assets/css/[name]-[hash][extname]';
          }
          return 'assets/[name]-[hash][extname]';
        },
        // 优化 chunk 文件名
        chunkFileNames: 'assets/js/[name]-[hash].js',
        // 优化入口文件名
        entryFileNames: 'assets/js/[name]-[hash].js',
      }
    },
    // 配置资源文件大小警告限制
    chunkSizeWarningLimit: 1000,
  }
}));
