import { useI18n } from './i18n'
import type { TranslationKey } from './i18n'
import { Icon } from './show-icon'

const environmentKeys: Record<string, TranslationKey> = {
  client_only: 'mod.environment.clientOnly',
  server_only: 'mod.environment.serverOnly',
  dedicated_server_only: 'mod.environment.dedicatedServerOnly',
  client_and_server: 'mod.environment.clientAndServer',
  server_only_client_optional: 'mod.environment.serverOnlyClientOptional',
  client_only_server_optional: 'mod.environment.clientOnlyServerOptional',
  client_or_server_prefers_both: 'mod.environment.clientOrServerPrefersBoth',
  client_or_server: 'mod.environment.clientOrServer',
  singleplayer_only: 'mod.environment.singleplayerOnly',
}

export function EnvironmentBadge({
  environment,
  target,
  catalog = false,
}: {
  environment?: string | null
  target?: string
  catalog?: boolean
}) {
  const { t } = useI18n()
  const key = environment ? environmentKeys[environment] : undefined
  if (!key && !target) return null
  const fallback = target === 'client'
    ? 'mod.environment.client'
    : target === 'server'
      ? 'mod.environment.server'
      : 'mod.environment.clientAndServer'
  const server = environment === 'server_only' || environment === 'dedicated_server_only' || target === 'server'
  return (
    <span className={catalog ? 'discover-badge environment' : 'mod-info-badge target'}>
      <Icon name={server ? 'server' : 'client'} />
      {t(key || fallback)}
    </span>
  )
}
