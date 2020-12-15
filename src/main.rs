mod decoder;

use log::{Log, Metadata, Record, info, LevelFilter};
use decoder::Decoder;
use std::fs::File;
use std::env;
use std::io::{BufReader,BufWriter};
use std::sync::Mutex;
use std::sync::Arc;
use std::cell::RefCell;

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


fn main() {
    let s = Arc::new(Mutex::new(RefCell::new("".to_string())));
    log::set_boxed_logger(Box::new(StrLogger{s: s.clone()})).unwrap();
    log::set_max_level(LevelFilter::Info);
    let path = env::args().nth(1).unwrap();
    info!("path {}", path);
    let mut decoder = Decoder::new(BufReader::new(File::open(path).unwrap()));
    decoder.decode().unwrap();
    let mut w = BufWriter::new(File::create("output.ppm").unwrap());
    decoder.outputppm(&mut w).unwrap();
    println!("{}", s.lock().unwrap().borrow())
}
