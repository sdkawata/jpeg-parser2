import {Decoder} from 'wasm-jpeg-parser2';

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
  console.log('parse start')
  const handle = decoder.parse(u8array);
  console.log('parse success')
  document.getElementById('output').textContent = decoder.get_width(handle) + "x" + decoder.get_height(handle) + "\n" + decoder.get_log(handle)
  decoder.free_handle(handle);
});