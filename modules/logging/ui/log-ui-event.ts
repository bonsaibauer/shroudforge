import { post } from '../../transport/ui/send-host-command'

type UiLogEvent = {
  level: 'INFO' | 'WARN' | 'ERROR' | 'DEBUG' | 'TRACE'
  source: 'wiki' | 'compatibility' | 'modloader'
  operation: string
  subject: string
  message: string
  notify?: boolean
}

const logText = (value: string, limit: number) =>
  Array.from(value.replace(/[\u0000-\u001f\u007f-\u009f]/g, ' ').trim()).slice(0, limit).join('')

export function logUiEvent(event: UiLogEvent) {
  post('ui-log', {
    event: {
      ...event,
      operation: logText(event.operation, 120),
      subject: logText(event.subject, 120),
      message: logText(event.message, 2000),
      notify: event.notify === true && ['WARN', 'ERROR'].includes(event.level),
    },
  })
}
