export type WikiPageSummary = {
  id: string
  pageTitle: string
  locale: string
  title: string
  summary: string
  categories?: string[]
  illustration?: string
  revisionId?: number
  sortIndex?: number
}

export type WikiSearchPage = {
  pages: WikiPageSummary[]
  continuation?: Record<string, string>
}

export type WikiSearchFilters = {
  query: string
  category: string
  language: string
  locale: string
}

export type CachedWikiCategories = {
  savedAt: number
  categories: string[]
}

export type MediaWikiArticle = {
  id: string
  pageTitle: string
  revisionId?: number
  locale: string
  title: string
  summary: string
  categories?: string[]
  bodyHtml: string
  illustration?: string
  fallback: boolean
}

export type CachedWikiSearch = {
  savedAt: number
  pages: WikiPageSummary[]
  continuation?: Record<string, string>
}
