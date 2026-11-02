import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { basename, dirname, extname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const token = process.env.CROWDIN_PERSONAL_TOKEN?.trim()
const projectId = process.env.CROWDIN_PROJECT_ID?.trim() || '938941'
const apiUrl = 'https://api.crowdin.com/api/v2'
const scriptDirectory = dirname(fileURLToPath(import.meta.url))
const repositoryRoot = resolve(scriptDirectory, '../..')
const localeDirectory = resolve(repositoryRoot, 'modules/ui-resources/locales')
const outputPath = resolve(repositoryRoot, 'build/ui/modloader-inputs/crowdin-progress.json')
const sourceLocale = 'en-GB'
const localeFiles = (await readdir(localeDirectory))
  .filter((file) => extname(file).toLowerCase() === '.json')
  .map((file) => basename(file, extname(file)))
  .sort((left, right) => left.localeCompare(right))

if (!localeFiles.includes(sourceLocale)) {
  throw new Error('Missing source locale: modules/ui-resources/locales/en-GB.json')
}

const targetLocales = localeFiles.filter((locale) => locale !== sourceLocale)
const targetLocaleByCode = new Map(
  targetLocales.map((locale) => [normalizeCode(locale), locale]),
)
const requestedBranch =
  process.env.CROWDIN_BRANCH?.trim() ||
  (await readFile(resolve(repositoryRoot, 'VERSION'), 'utf8')).trim()

function normalizeCode(value) {
  return String(value ?? '').trim().replaceAll('_', '-').toLowerCase()
}

function versionTuple(value) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(value)
  return match ? match.slice(1).map(Number) : null
}

function compareVersions(left, right) {
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return left[index] - right[index]
  }
  return 0
}

function isRecord(value) {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value)
}

function emptyLanguage(locale) {
  return {
    locale,
    translationProgress: null,
    approvalProgress: null,
    translatedWords: 0,
    approvedWords: 0,
    totalWords: 0,
  }
}

function createSnapshot(branch, languages, updatedAt = null) {
  return {
    schemaVersion: 1,
    projectId: Number(projectId),
    projectUrl: 'https://crowdin.com/project/shroudforge',
    branch,
    updatedAt,
    languages,
  }
}

async function writeSnapshot(snapshot) {
  await mkdir(dirname(outputPath), { recursive: true })
  await writeFile(outputPath, JSON.stringify(snapshot, null, 2) + '\n', 'utf8')
}

if (!token) {
  const snapshot = createSnapshot(requestedBranch, targetLocales.map(emptyLanguage))
  await writeSnapshot(snapshot)
  console.warn(
    'Crowdin progress was not refreshed because CROWDIN_PERSONAL_TOKEN is not set. Wrote an unavailable entry for each local target locale.',
  )
} else {
  if (!requestedBranch) {
    throw new Error(
      'Set CROWDIN_BRANCH or provide VERSION with the branch being built, for example 1.7.1.',
    )
  }

  async function request(path) {
    const response = await fetch(apiUrl + path, {
      headers: {
        Accept: 'application/json',
        Authorization: 'Bearer ' + token,
      },
      signal: AbortSignal.timeout(30000),
    })
    if (!response.ok) {
      const detail = (await response.text()).slice(0, 500)
      throw new Error(
        'Crowdin API request failed (' +
          response.status +
          '): ' +
          detail,
      )
    }
    return response.json()
  }

  async function getAllPages(path) {
    const result = []
    let offset = 0
    while (true) {
      const separator = path.includes('?') ? '&' : '?'
      const page = await request(
        path + separator + 'limit=500&offset=' + offset,
      )
      const items = Array.isArray(page.data) ? page.data : []
      const normalizedItems = items.map((item) =>
        isRecord(item) && isRecord(item.data) ? item.data : item,
      )
      result.push(...normalizedItems)
      if (items.length < 500) break
      offset += items.length
    }
    return result
  }

  function localeForLanguageProgress(item) {
    if (!isRecord(item)) return null
    const value = isRecord(item.data) ? item.data : item
    const language = isRecord(value.language) ? value.language : {}
    // Match Crowdin language identifiers and filename codes, not the locale
    // fallback. Crowdin's generic `en` language reports locale `en-US`, which
    // would collide with the distinct `en-US` target language.
    const codes = [
      language.id,
      value.languageId,
      language.twoLettersCode,
      language.editorCode,
    ]

    for (const code of codes) {
      const locale = targetLocaleByCode.get(normalizeCode(code))
      if (locale) return locale
    }
    return null
  }

  function normalizeLanguage(item, locale) {
    const value = isRecord(item.data) ? item.data : item
    const words = isRecord(value.words) ? value.words : {}
    const totalWords = Math.max(0, Number(words.total) || 0)
    const translatedWords = Math.max(0, Number(words.translated) || 0)
    const approvedWords = Math.max(0, Number(words.approved) || 0)
    const progress = (candidate) =>
      typeof candidate === 'number' && Number.isFinite(candidate)
        ? Math.min(100, Math.max(0, Math.round(candidate)))
        : null
    const percentageFromWords = (count) =>
      totalWords > 0
        ? Math.min(100, Math.max(0, Math.round((count / totalWords) * 100)))
        : null

    return {
      locale,
      translationProgress:
        totalWords > 0
          ? progress(value.translationProgress) ??
            percentageFromWords(translatedWords)
          : null,
      approvalProgress:
        totalWords > 0
          ? progress(value.approvalProgress) ??
            percentageFromWords(approvedWords)
          : null,
      translatedWords,
      approvedWords,
      totalWords,
    }
  }

  const branches = await getAllPages('/projects/' + projectId + '/branches')
  const exactBranch = branches.find(
    (branch) => isRecord(branch) && branch.name === requestedBranch,
  )
  const requestedVersion = versionTuple(requestedBranch)
  const fallbackBranch =
    !exactBranch && requestedVersion
      ? branches
          .filter((branch) => {
            if (!isRecord(branch) || typeof branch.name !== 'string') {
              return false
            }
            const candidateVersion = versionTuple(branch.name)
            return (
              candidateVersion &&
              compareVersions(candidateVersion, requestedVersion) < 0
            )
          })
          .sort((left, right) =>
            compareVersions(versionTuple(left.name), versionTuple(right.name)),
          )
          .at(-1)
      : null
  const currentBranch = exactBranch ?? fallbackBranch

  if (!currentBranch || typeof currentBranch.id !== 'number') {
    const snapshot = createSnapshot(
      requestedBranch,
      targetLocales.map(emptyLanguage),
    )
    await writeSnapshot(snapshot)
    console.warn(
      'Crowdin progress was not refreshed because the project does not have the requested version branch ' +
        requestedBranch +
        '. Wrote an unavailable entry for each local target locale.',
    )
  } else {
    if (currentBranch.name !== requestedBranch) {
      console.warn(
        'Crowdin does not have version branch ' +
          requestedBranch +
          '. Using the latest earlier branch ' +
          currentBranch.name +
          ' for the progress snapshot.',
      )
    }

    const branchRows = await getAllPages(
      '/projects/' + projectId + '/branches/' + currentBranch.id + '/languages/progress',
    )
    const rowsByLocale = new Map()
    for (const item of branchRows) {
      const locale = localeForLanguageProgress(item)
      if (!locale) continue
      if (rowsByLocale.has(locale)) {
        throw new Error(
          'More than one Crowdin language maps to the local locale file ' +
            locale +
            '.json. Use distinct locale filenames for these languages.',
        )
      }
      rowsByLocale.set(locale, normalizeLanguage(item, locale))
    }

    const languages = targetLocales.map(
      (locale) => rowsByLocale.get(locale) ?? emptyLanguage(locale),
    )
    const snapshot = createSnapshot(
      currentBranch.name,
      languages,
      new Date().toISOString(),
    )
    await writeSnapshot(snapshot)

    console.log(
      'Crowdin progress captured for build branch ' +
        currentBranch.name +
        ': ' +
        snapshot.languages
          .map(
            (language) =>
              language.locale +
              ' ' +
              (language.translationProgress === null
                ? 'unavailable'
                : language.translationProgress + '%'),
          )
          .join(', '),
    )
  }
}
