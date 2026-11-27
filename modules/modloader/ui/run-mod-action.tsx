import type { ModInfo, ModAction } from './mod-types'
import { postRequest } from '../../transport/ui/send-host-command'

export function ModActionButton({
  mod,
  action,
}: {
  mod: ModInfo
  action: ModAction
}) {
  return (
    <button
      className={`button ${action.style === 'danger' ? 'danger' : action.style === 'secondary' ? 'ghost' : ''}`}
      onClick={() => {
        if (!action.confirm || window.confirm(action.confirm))
          postRequest('run-mod-action', { modId: mod.instanceKey ?? mod.id, action: action.id })
      }}
    >
      {action.label || action.id}
    </button>
  )
}
