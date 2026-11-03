export type WikiArticle = {
  id: string
  locale: string
  title: string
  summary: string
  body: string
  category?: string
  illustration?: string
  fallback: boolean
}

const documents = import.meta.glob('../articles/*/*.md', {
  eager: true, query: '?raw', import: 'default',
}) as Record<string, string>
const images = import.meta.glob('../articles/*/*.{svg,png,jpg,webp}', {
  eager: true, query: '?url', import: 'default',
}) as Record<string, string>
const translations = new Map<string, Map<string, string>>()

for (const [path, contents] of Object.entries(documents)) {
  const match = path.match(/\/([^/]+)\/([^/]+)\.md$/)
  if (!match) continue
  const [, id, locale] = match
  if (!translations.has(id)) translations.set(id, new Map())
  translations.get(id)!.set(locale, contents)
}

export function readWikiArticle(id: string, requestedLocale: string): WikiArticle | undefined {
  const available = translations.get(id)
  if (!available) return undefined
  const locale = [requestedLocale, requestedLocale.split('-')[0], 'en', 'de']
    .find((candidate) => available.has(candidate)) || available.keys().next().value!
  const markdown = available.get(locale)!.replace(/\r/g, '').trim()
  const heading = markdown.match(/^#\s+(.+)\n+/)
  const introduction = heading ? markdown.slice(heading[0].length) : markdown
  const [summary, ...sections] = introduction.split(/\n\s*\n/)
  let body = sections.join('\n\n')
  // An optional first body comment stores article category metadata without rendering it.
  const category = body.match(/^<!--\s*wiki-category:\s*([a-z0-9-]+)\s*-->\n*/i)
  if (category) body = body.slice(category[0].length).trim()
  return {
    id, locale, title: heading?.[1] || id,
    summary: summary.replace(/\n/g, ' '), body, category: category?.[1].toLowerCase(),
    illustration: images[`../articles/${id}/illustration.svg`],
    fallback: locale !== requestedLocale,
  }
}

export function readWikiArticles(locale: string): WikiArticle[] {
  return [...translations.keys()].map((id) => readWikiArticle(id, locale)!)
    .sort((left, right) => left.title.localeCompare(right.title))
}

export function wikiImage(articleId: string, source: string): string {
  const relative = source.replace(/^\.\//, '')
  if (relative.split('/').includes('..')) return ''
  return images[`../articles/${articleId}/${relative}`] || ''
}
