use std::env;
use std::io::{BufReader, Read};
use std::fs::File;
use failure::format_err;
use failure::Error;

fn read_u8<T:Read>(r: &mut T) -> Result<u8, Error> {
    let mut buf = [0;1];
    r.read_exact(&mut buf)?;
    Ok(buf[0])
}
fn read_u16<T:Read>(r: &mut T) -> Result<u16, Error> {
    let mut buf = [0;2];
    r.read_exact(&mut buf)?;
    Ok((buf[0] as u16) * 0x100 + (buf[1] as u16))
}


fn check_soi<T:Read>(r: &mut T) -> Result<(), Error> {
    let u0 = read_u8(r)?;
    let u1 = read_u8(r)?;
    if u0 != 0xff || u1 != 0xd8 {
        return Err(format_err!("no SOI found found {} {}", u0, u1));
    }
    Ok(())
}

struct Decoder<T:Read> {
    reader: T
}

impl<T:Read> Decoder<T> {
    pub fn new(reader:T) -> Decoder<T> {
        Decoder {
            reader: reader
        }
    }
    fn next_marker(&mut self) -> Result<u8, Error>  {
        let u0 = read_u8(&mut self.reader)?;
        if u0 != 0xff {
            return Err(format_err!("expected 0xff get:{:x}", u0))
        }
        read_u8(&mut self.reader)
    }
    fn read_marker_content(&mut self) -> Result<Vec<u8>, Error> {
        let size = read_u16(&mut self.reader)?;
        let mut buf = vec![0;size as usize - 2];
        self.reader.read_exact(&mut buf)?;
        Ok(buf)
    }
    fn parse_app(&mut self, index: u8) -> Result<(), Error> {
        let content = self.read_marker_content()?;
        println!("APP{} size={}", index, content.len());
        Ok(())
    }
    pub fn decode(&mut self) -> Result<(), Error>{
        check_soi(&mut self.reader)?;
        println!("SOI found");
        loop {
            match self.next_marker()? {
                m @ 0xe0..=0xef => self.parse_app(m-0xe0)?,
                m => return Err(format_err!("unknown marker {:x}", m))
            }
        }
    }
}


fn main() {
    let path = env::args().nth(1).unwrap();
    println!("path {}", path);
    let mut decoder = Decoder::new(BufReader::new(File::open(path).unwrap()));
    decoder.decode().unwrap()
}
