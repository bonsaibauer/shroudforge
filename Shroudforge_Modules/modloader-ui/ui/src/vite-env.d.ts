/// <reference types="vite/client" />

interface Window {
  ipc?: { postMessage(message: string): void }
  __shroudforgeUpdate(snapshot: unknown): void
}
