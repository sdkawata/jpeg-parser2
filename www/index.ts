import {Decoder} from 'wasm-jpeg-parser2';
import {memory} from 'wasm-jpeg-parser2/jpeg-parser2_bg.wasm';

window['log'] = (s) => {
  console.log(s)
}

const decoder = Decoder.new()

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
  const u8array = await readImage(file);
  let currentLog = "decoding...\n"
  document.getElementById('output').textContent = currentLog
  window['log'] = (s) => {
    currentLog = currentLog + s + "\n"
    document.getElementById('output').textContent = currentLog
  }
  console.log('parse start')
  const handle = decoder.parse(u8array);
  console.log('parse success')
  const width = decoder.get_width(handle)
  const height = decoder.get_height(handle)
  const canvas = document.getElementById('canvas') as HTMLCanvasElement
  canvas.width = width
  canvas.height = height
  if (width == 0 || height == 0) {
    return
  }
  const ctx = canvas.getContext('2d')
  const idata = new ImageData(width, height)
  const pix = new Uint8Array(memory.buffer, decoder.get_pix_ptr(handle), width*height*4)
  idata.data.set(pix)
  ctx.putImageData(idata, 0, 0)
  decoder.free_handle(handle);
});