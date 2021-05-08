export type WorkerMessage = {
  type: 'log',
  message: string,
} | {
  type: 'done',
  result: Uint8Array,
  width: number,
  height: number
}

export type BrowserMessage = {
  type: 'start',
  img: Uint8Array,
}
