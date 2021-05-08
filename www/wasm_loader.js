// tsにするとうまく別のchunkになってくれないのでこれだけtsにする
// cf. https://github.com/rustwasm/wasm-bindgen/issues/700
export const loadWasm = Promise.all([
  import(/* webpackChunkName: "wasm" */'wasm-jpeg-parser2'),
  import(/* webpackChunkName: "wasm" */'wasm-jpeg-parser2/jpeg-parser2_bg.wasm')
])