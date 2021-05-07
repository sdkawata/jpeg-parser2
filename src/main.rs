mod decoder;

use env_logger;
use log::{Log, Metadata, Record, info, warn, LevelFilter};
use std::fs::File;
use std::env;
use std::io::{BufReader,BufWriter};
use std::sync::Mutex;
use std::sync::Arc;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use std::collections::HashMap;

struct StrLogger {
    s: Arc<Mutex<RefCell<String>>>,
}

impl Log for StrLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        println!("{}", record.args());
        self.s.lock().unwrap().borrow_mut().push_str(&format!("{}\n", record.args()))
    }
    fn flush(&self){}
}



struct Result {
    width: usize,
    height: usize,
    log: String,
    pix: Vec<u8>,
}

#[wasm_bindgen]
pub struct Decoder {
    results: HashMap<usize, Result>,
    ptr: usize,
    log_string: Arc<Mutex<RefCell<String>>>,
}

#[wasm_bindgen]
impl Decoder {
    pub fn new() -> Decoder {
        // do not call this more function then once or panic
        let s = Arc::new(Mutex::new(RefCell::new("".to_string())));
        log::set_boxed_logger(Box::new(StrLogger{s: s.clone()})).unwrap();
        log::set_max_level(LevelFilter::Info);
        Decoder{
            results: HashMap::new(),
            ptr: 0,
            log_string: s.clone(),
        }
    }
    pub fn parse(&mut self, data: &[u8]) -> usize {
        *(self.log_string.lock().unwrap().borrow_mut()) = "".to_string();
        let mut decoder = decoder::Decoder::new(BufReader::new(data));
        let decode_res = decoder.decode();
        match decode_res  {
            Err(e) => warn!("error occured while decoding {}", e),
            _ => (),
        }
        let result = Result{
            width: decoder.get_width() as usize,
            height: decoder.get_height() as usize,
            log: self.log_string.lock().unwrap().borrow().clone(),
            pix: decoder.get_rgb_vec(true),
        };
        self.ptr += 1;
        self.results.insert(self.ptr, result);
        self.ptr
    }
    pub fn get_height(&self, handle:usize) -> usize {
        self.results.get(&handle).unwrap().height
    }
    pub fn get_width(&self, handle:usize) -> usize {
        self.results.get(&handle).unwrap().width
    }
    pub fn get_log(&self, handle:usize) -> String {
        self.results.get(&handle).unwrap().log.clone()
    }
    pub fn get_pix_ptr(&self, handle:usize) -> *const u8 {
        self.results.get(&handle).unwrap().pix.as_ptr()
    }
    pub fn free_handle(&mut self, handle:usize) {
        self.results.remove(&handle);
    }
}

pub fn main(){
    if cfg!(target_arch="wasm32") {
        return
    }
    env_logger::init();
    let path = env::args().nth(1).unwrap();
    info!("path {}", path);
    let mut decoder = decoder::Decoder::new(BufReader::new(File::open(path).unwrap()));
    let decode_res = decoder.decode();
    match decode_res  {
        Err(e) => warn!("error occured while decoding {}", e),
        _ => (),
    }
    let mut w = BufWriter::new(File::create("output.ppm").unwrap());
    decoder.outputppm(&mut w).unwrap();
}
