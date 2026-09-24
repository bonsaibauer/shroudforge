/// <reference types="vite/client" />

interface Window {
  ipc?: { postMessage(message: string): void }
  __shroudforgeUpdate(snapshot: unknown): void
  __shroudforgeWindowState(state: unknown): void
  __shroudforgeCommandResult(result: {requestId:string;success:boolean;message:string;batchKey?:string;restoreSettings?:boolean}): void
}
