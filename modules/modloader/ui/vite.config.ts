import { defineConfig, type Plugin } from 'vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const inlineBuiltAssets: Plugin = {
  name: 'shroudforge:inline-built-assets',
  apply: 'build',
  enforce: 'post',
  generateBundle(_options, bundle) {
    const assets = Object.values(bundle)
    const htmlFiles = assets.filter(
      (asset) => asset.type === 'asset' && /\.html?$/i.test(asset.fileName),
    )
    const scripts = assets.filter(
      (asset) => asset.type === 'chunk' && /\.[mc]?js$/i.test(asset.fileName),
    )
    const styles = assets.filter(
      (asset) => asset.type === 'asset' && /\.css$/i.test(asset.fileName),
    )
    const remove = new Set<string>()

    const escapeRegExp = (value: string) =>
      value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    const asText = (source: string | Uint8Array) =>
      typeof source === 'string' ? source : new TextDecoder().decode(source)

    for (const asset of htmlFiles) {
      if (asset.type !== 'asset') continue
      let html = asText(asset.source)

      for (const script of scripts) {
        if (script.type !== 'chunk') continue
        const name = escapeRegExp(script.fileName)
        const pattern = new RegExp(
          `<script([^>]*?) src="(?:[^\"]*?/)?${name}"([^>]*)></script>`,
        )
        if (!pattern.test(html)) continue
        const code = script.code
          .replace(/"?__VITE_PRELOAD__"?/g, 'void 0')
          .replace(/<(\/script>|!--)/g, '\\x3C$1')
        html = html.replace(
          pattern,
          (_match, beforeSrc: string, afterSrc: string) =>
            `<script${beforeSrc}${afterSrc}>${code.trim()}</script>`,
        )
        remove.add(script.fileName)
      }

      for (const style of styles) {
        if (style.type !== 'asset') continue
        const name = escapeRegExp(style.fileName)
        const pattern = new RegExp(
          `<link([^>]*?) href="(?:[^\"]*?/)?${name}"([^>]*)>`,
        )
        if (!pattern.test(html)) continue
        const css = asText(style.source).replace('@charset "UTF-8";', '')
        html = html.replace(
          pattern,
          (_match, beforeHref: string, afterHref: string) =>
            `<style${beforeHref}${afterHref}>${css.trim()}</style>`,
        )
        remove.add(style.fileName)
      }

      asset.source = html
    }

    for (const fileName of remove) delete bundle[fileName]
  },
}

export default defineConfig(({ mode }) => {
  const standaloneUpdater = mode === 'updater'
  const root = standaloneUpdater
    ? resolve(process.cwd(), '../../updater/ui')
    : process.cwd()
  return {
    plugins: [react(), inlineBuiltAssets],
    resolve: {
      alias: {
        react: resolve(process.cwd(), 'node_modules/react'),
        'react-dom': resolve(process.cwd(), 'node_modules/react-dom'),
        dompurify: resolve(process.cwd(), 'node_modules/dompurify'),
        '@generated-flags': fileURLToPath(
          new URL(
            '../../../build/ui/modloader-inputs/generated-flags.ts',
            import.meta.url,
          ),
        ),
      },
      dedupe: ['react', 'react-dom'],
    },
    root,
    server: { fs: { allow: [resolve(process.cwd(), '../../../')] } },
    base: './',
    build: {
      outDir: fileURLToPath(
        new URL(
          standaloneUpdater ? '../../../build/ui/updater/' : '../../../build/ui/modloader/',
          import.meta.url,
        ),
      ),
      emptyOutDir: true,
      target: 'es2020',
      assetsInlineLimit: 100000000,
      chunkSizeWarningLimit: 100000000,
      assetsDir: '',
      cssCodeSplit: false,
      rollupOptions: {
        input: standaloneUpdater
          ? resolve(root, 'updater.html')
          : resolve(root, 'index.html'),
        output: { codeSplitting: false },
      },
    },
  }
})
