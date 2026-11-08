import { useState } from 'react'
import type { CommandResult } from '../../notifications/ui/notification-types'
import { logUiEvent } from '../../logging/ui/log-ui-event'
import { useI18n } from '../../ui-resources/i18n'
import { Icon } from '../../ui-resources/show-icon'
import { postRequest } from '../../transport/ui/send-host-command'
import { clearWikiCache, WIKI_API_SETTINGS } from '../mediawiki-api'

export function WikiActions({
  onRefresh,
  notifyCommandResult,
  refreshDisabled = false,
}: {
  onRefresh(): Promise<void>
  notifyCommandResult(result: CommandResult): void
  refreshDisabled?: boolean
}) {
  const { t } = useI18n()
  const [refreshing, setRefreshing] = useState(false)

  const notifySuccess = (action: string, message: string) => notifyCommandResult({
    requestId: `wiki-${action}-${Date.now()}`,
    success: true,
    message,
    level: 'info',
  })

  const refresh = async () => {
    if (refreshing || refreshDisabled) return
    setRefreshing(true)
    try {
      await onRefresh()
      logUiEvent({ level: 'INFO', source: 'wiki', operation: 'Refresh Wiki',
        subject: 'manual', message: 'The user refreshed Wiki data.' })
      notifySuccess('refresh', 'wiki.refreshSucceeded')
    } catch (error) {
      logUiEvent({ level: 'ERROR', source: 'wiki', operation: 'Refresh Wiki',
        subject: 'manual', message: String(error), notify: true, notifyMessage: t('wiki.loadFailed') })
    } finally {
      setRefreshing(false)
    }
  }

  const clearCache = () => {
    if (clearWikiCache()) {
      logUiEvent({ level: 'INFO', source: 'wiki', operation: 'Clear Wiki cache',
        subject: 'local-cache', message: 'The local Wiki cache was deleted.' })
      notifySuccess('clear-cache', 'wiki.cacheCleared')
      return
    }
    logUiEvent({ level: 'ERROR', source: 'wiki', operation: 'Clear Wiki cache',
      subject: 'local-cache', message: 'The local Wiki cache could not be deleted.',
      notify: true, notifyMessage: t('wiki.cacheClearFailed') })
  }

  return (
    <div className="wiki-toolbar-actions">
      <button type="button" className="button ghost" onClick={() => postRequest('open-url', {
        url: WIKI_API_SETTINGS.siteOrigin,
      })}>
        <Icon name="wiki" />{t('wiki.openOnline')}
      </button>
      <button type="button" className="button ghost" onClick={() => void refresh()}
        disabled={refreshing || refreshDisabled}>
        <Icon name="refresh" />{t('wiki.refresh')}
      </button>
      <button type="button" className="button danger" onClick={clearCache}>
        <Icon name="trash" />{t('wiki.clearCache')}
      </button>
    </div>
  )
}
