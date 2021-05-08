import Worker from 'worker-loader?filename=dist/worker-[fullhash].js!./worker'
import { BrowserMessage, WorkerMessage } from './message'

const worker = new Worker()

function readImage(file: File): Promise<Uint8Array> {
  return new Promise((resolve) => {
    let reader = new FileReader()
    reader.addEventListener('load', () => {
      resolve(new Uint8Array(reader.result as ArrayBuffer))
    })
    reader.readAsArrayBuffer(file)
  })
}

document.body.addEventListener('dragover', (e) => {
  e.preventDefault();
  e.dataTransfer.dropEffect = 'copy';
});
document.body.addEventListener('dragenter', (e) => {
  e.preventDefault();
});
document.body.addEventListener('drop', async (e) => {
  e.preventDefault();
  let files = e.dataTransfer.files;
  let file = files[0];
  const img = await readImage(file);
  let currentLog = ""
  const appendLog = (s: string) => {
    currentLog += s + "\n"
    document.getElementById('output').textContent = currentLog
  }
  worker.onmessage = (e) => {
    const msg = e.data as WorkerMessage
    if (msg.type === 'log') {
      appendLog(msg.message)
    } else if (msg.type === 'done') {
      const canvas = document.getElementById('canvas') as HTMLCanvasElement
      canvas.width = msg.width
      canvas.height = msg.height
      if (msg.width == 0 || msg.height == 0) {
        return
      }
      const ctx = canvas.getContext('2d')
      const idata = new ImageData(msg.width, msg.height)
      idata.data.set(msg.result)
      ctx.putImageData(idata, 0, 0)
      worker.onmessage = () => {}
    }
  }
  appendLog('decode start...')
  console.log(img)
  worker.postMessage({
    type: 'start',
    img,
  } as BrowserMessage, [img.buffer])
});