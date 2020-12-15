mod decoder;

use decoder::Decoder;
use std::fs::File;
use std::env;
use std::io::{BufReader,BufWriter};

fn main() {
    let path = env::args().nth(1).unwrap();
    println!("path {}", path);
    let mut decoder = Decoder::new(BufReader::new(File::open(path).unwrap()));
    decoder.decode().unwrap();
    let mut w = BufWriter::new(File::create("output.ppm").unwrap());
    decoder.outputppm(&mut w).unwrap();
}
