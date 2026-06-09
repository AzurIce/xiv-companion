import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vite'
import solid from 'vite-plugin-solid'
import tailwindcss from '@tailwindcss/vite'

function versionInfo() {
  try {
    const dir = fileURLToPath(new URL('.', import.meta.url))
    return JSON.parse(readFileSync(resolve(dir, 'public', 'version.json'), 'utf-8'))
  } catch {
    return { commit: 'dev', date: new Date().toLocaleString('zh-CN') }
  }
}

const version = versionInfo()

export default defineConfig({
  base: process.env.BASE_URL || '/',
  plugins: [solid(), tailwindcss()],
  define: {
    __BUILD_COMMIT__: JSON.stringify(version.commit),
    __BUILD_DATE__: JSON.stringify(version.date),
  },
})

