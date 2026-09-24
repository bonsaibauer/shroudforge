import { readdir, readFile } from 'node:fs/promises'
import { dirname, extname, basename, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const localeDirectory = resolve(root, 'src/locales')
const files = (await readdir(localeDirectory)).filter(file => extname(file) === '.json').sort()
const catalogs = Object.fromEntries(await Promise.all(files.map(async file => [
  basename(file, '.json'),
  JSON.parse(await readFile(resolve(localeDirectory, file), 'utf8')),
])))
const source = catalogs.en
if (!source) throw new Error('Missing source locale: src/locales/en.json')

const sourceKeys = Object.keys(source).sort()
const metadataKeys = ['locale.name', 'locale.flag']
const placeholders = value => [...value.matchAll(/\{([A-Za-z0-9_]+)\}/g)].map(match => match[1]).sort()
const failures = []

for (const [locale, messages] of Object.entries(catalogs)) {
  const keys = Object.keys(messages).sort()
  const missing = sourceKeys.filter(key => !(key in messages))
  const extra = keys.filter(key => !(key in source))
  if (missing.length) failures.push(`${locale}: missing keys: ${missing.join(', ')}`)
  if (extra.length) failures.push(`${locale}: extra keys: ${extra.join(', ')}`)
  for (const key of sourceKeys) {
    if (typeof messages[key] !== 'string' || !messages[key].trim()) failures.push(`${locale}: empty or invalid value: ${key}`)
    if (JSON.stringify(placeholders(source[key])) !== JSON.stringify(placeholders(messages[key] ?? ''))) failures.push(`${locale}: placeholder mismatch: ${key}`)
  }
  for (const key of metadataKeys) {
    if (!messages[key]?.trim()) failures.push(`${locale}: missing Crowdin locale metadata: ${key}`)
  }
  if (!/^[a-z]{2}$/i.test(messages['locale.flag'] ?? '')) failures.push(`${locale}: locale.flag must be a two-letter SVG country code`)
}

if (failures.length) {
  console.error(failures.join('\n'))
  process.exit(1)
}
console.log(`Locale validation passed: ${sourceKeys.length} keys, ${Object.keys(catalogs).length} locales.`)
