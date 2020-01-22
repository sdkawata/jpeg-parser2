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

fn check_soi<T:Read>(r: &mut T) -> Result<(), Error> {
    let u0 = read_u8(r)?;
    let u1 = read_u8(r)?;
    if u0 != 0xff || u1 != 0xd8 {
        return Err(format_err!("no SOI found found {} {}", u0, u1));
    }
    Ok(())
}

fn main() {
    let path = env::args().nth(1).unwrap();
    println!("path {}", path);
    let mut file = BufReader::new(File::open(path).unwrap());
    check_soi(&mut file).unwrap()
}
