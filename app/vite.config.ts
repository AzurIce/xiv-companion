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

function appBaseUrl() {
  const value = process.env.XIV_COMPANION_BASE_URL || process.env.BASE_URL || '/'
  if (value.startsWith('http://') || value.startsWith('https://')) {
    return value.endsWith('/') ? value : `${value}/`
  }
  const withLeadingSlash = value.startsWith('/') ? value : `/${value}`
  return withLeadingSlash.endsWith('/') ? withLeadingSlash : `${withLeadingSlash}/`
}

export default defineConfig({
  base: appBaseUrl(),
  plugins: [solid(), tailwindcss()],
  server: {
    proxy: {
      '/api/universalis': {
        target: 'https://universalis.app',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api\/universalis/, ''),
      },
    },
  },
  define: {
    __BUILD_COMMIT__: JSON.stringify(version.commit),
    __BUILD_DATE__: JSON.stringify(version.date),
  },
})
