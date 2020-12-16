mod decoder;

use log::{Log, Metadata, Record, info, LevelFilter};
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
    fn enabled(&self, metadata: &Metadata) -> bool {
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
    log: String
}

#[wasm_bindgen]
pub struct Decoder {
    results: HashMap<usize, Result>,
    ptr: usize,
}

#[wasm_bindgen]
impl Decoder {
    pub fn new() -> Decoder {
        Decoder{
            results: HashMap::new(),
            ptr: 0,
        }
    }
    pub fn parse(&mut self, data: &[u8]) -> usize {
        let s = Arc::new(Mutex::new(RefCell::new("".to_string())));
        log::set_boxed_logger(Box::new(StrLogger{s: s.clone()})).unwrap();
        log::set_max_level(LevelFilter::Info);
        let mut decoder = decoder::Decoder::new(BufReader::new(data));
        decoder.decode().unwrap();
        let result = Result{
            width: decoder.get_width() as usize,
            height: decoder.get_height() as usize,
            log: s.lock().unwrap().borrow().clone(),
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
}

pub fn main(){}