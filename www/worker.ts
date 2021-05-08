
global['log'] = (s) => {
  console.log('uncaught message', s)
}
import {BrowserMessage, WorkerMessage} from './message'

import {loadWasm} from './wasm_loader'

loadWasm().then(([{Decoder}, {memory}]) => {
  const ctx: Worker = self as any
  function postMessage(message: WorkerMessage, transfer:any = []) {
    ctx.postMessage(message, transfer)
  }
  
  const decoder = Decoder.new()
  
  console.log('worker start')
  
  ctx.onmessage = (e) => {
    const msg = e.data as BrowserMessage
    if (msg.type === "start") {
      const appendLog = (s:string) => {
        postMessage({
          type: 'log',
          message: s
        })
      }
      global['log'] = appendLog
      console.log('decode start at worker')
      const start = performance.now()
      const handle = decoder.parse(msg.img);
      const end = performance.now()
      console.log('decode end at worker')
      appendLog("decode end elapsed: " + (end - start) + " sec")
      const width = decoder.get_width(handle)
      const height = decoder.get_height(handle)
      if (width == 0 || height == 0) {
        const zero = new Uint8Array(0)
        ctx.postMessage({
          type: 'done',
          result: zero,
          width: 0,
          height: 0,
        }, [zero.buffer])
        return
      }
      // cf. https://stackoverflow.com/questions/59705741/why-memory-could-not-be-cloned
      const pix = new Uint8Array(memory.buffer, decoder.get_pix_ptr(handle), width*height*4)
      const clonedPix = new Uint8Array(width*height*4)
      clonedPix.set(pix)
      ctx.postMessage({
        type: 'done',
        result: clonedPix,
        width,
        height,
      }, [clonedPix.buffer])
      decoder.free_handle(handle);
    }
  }
})

