import { fileURLToPath, URL } from 'node:url'

import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import vueDevTools from 'vite-plugin-vue-devtools'

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    vue(),
    vueDevTools(),
  ],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url))
    },
  },
  server: {
    proxy: {
      '/api': {
        target: `http://localhost:${process.env.ADMIN_UI_DEV_PORT}`,
        changeOrigin: true,
        secure: false,
      },
      '/files': {
        target: `http://localhost:${process.env.PUBLIC_UI_DEV_PORT}`,
        changeOrigin: true,
        secure: false,
      },
      '/thumbnails': {
        target: `http://localhost:${process.env.PUBLIC_UI_DEV_PORT}`,
        changeOrigin: true,
        secure: false,
      },
    }
  }
})
