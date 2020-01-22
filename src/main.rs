use std::env;
use std::io::{BufReader, Read, Cursor, Seek};
use std::fs::File;
use std::iter::Iterator;
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

struct QuantizationTable {
    id: u8,
    table: [u8;64]
}

struct ScanComponent {
    id: u8,
    hi: u8,
    vi: u8,
    qt_id: u8,
}

struct Component<'a> {
    qTable: & 'a QuantizationTable,
    acHaff: & 'a HaffTable,
    dcHaff: & 'a HaffTable,
    hi: u8,
    vi: u8,
}

struct HaffTable {
    tc: u8,
    id: u8,
    bits: [u8;16],
    values: Vec<u8>
}

struct Decoder<T:Read> {
    reader: T,
    qts: Vec<QuantizationTable>,
    hafftables: Vec<HaffTable>,
    scanComponents: Vec<ScanComponent>,
    height: u16,
    width: u16,
}

impl<T:Read> Decoder<T> {
    pub fn new(reader:T) -> Decoder<T> {
        Decoder {
            reader: reader,
            qts: Vec::new(),
            hafftables: Vec::new(),
            height: 0,
            width: 0,
            scanComponents: Vec::new()
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
    fn parse_dqt(&mut self) -> Result<(), Error> {
        let content = self.read_marker_content()?;
        let len = content.len() as u64;
        println!("DQT size={}", len);
        let mut cursor = Cursor::new(content);
        while len > cursor.position() {
            let flag = read_u8(&mut cursor)?;
            let pq = flag >> 4;
            let tq = flag & 0xf;
            println!("pq(presision)={} tq(destination identifier)={}", pq, tq);
            let mut buf = [0;64];
            cursor.read_exact(&mut buf)?;
            self.qts.push(QuantizationTable{
                id: tq,
                table: buf
            })
        }
        Ok(())
    }
    fn parse_sof0(&mut self) -> Result<(), Error> {
        let content = self.read_marker_content()?;
        println!("SOF0 size={}", content.len());
        let mut r = Cursor::new(content);
        let p = read_u8(&mut r)?;
        let y = read_u16(&mut r)?;
        let x = read_u16(&mut r)?;
        let nf = read_u8(&mut r)?;
        println!("p(presision)={} y(lines)={} x(samples per line)={} nf(number of components)={}", p,y,x,nf);
        self.height = y;
        self.width = x;
        for i in 0..nf {
            let ci = read_u8(&mut r)?;
            let hvi = read_u8(&mut r)?;
            let tqi = read_u8(&mut r)?;
            let hi = hvi >> 4;
            let vi = hvi & 0xf;
            println!("ci(id)={} hi,vi(sampling factor)={},{} tqi(dqt selector)={}", ci, hi, vi, tqi);
            self.scanComponents.push(ScanComponent{
                id: ci,
                hi: hi,
                vi: vi,
                qt_id: tqi
            })
        }
        Ok(())
    }
    fn parse_dht(&mut self) -> Result<(), Error> {
        let content = self.read_marker_content()?;
        let len = content.len() as u64;
        println!("DHT size={}", len);
        let mut cursor = Cursor::new(content);
        while len > cursor.position() {
            let flag = read_u8(&mut cursor)?;
            let tc = flag >> 4;
            let tn = flag & 0xf;
            println!("tc={}({}) th(destination identifier)={}", tc, if tc == 0 { "DC" } else {"AC"}, tn);
            let mut bits = [0;16];
            cursor.read_exact(&mut bits)?;
            let valuenum = bits.iter().fold(0, |acc, a| acc + a);
            let mut values = vec![0;valuenum as usize];
            cursor.read_exact(&mut values)?;
            self.hafftables.push(HaffTable{
                tc: tc,
                id: tn,
                bits: bits,
                values:values
            })
        }
        Ok(())
    }
    fn parse_sos(&mut self) -> Result<(), Error> {
        let content = self.read_marker_content()?;
        println!("SOS size={}", content.len());
        let mut cursor = Cursor::new(content);
        let ns = read_u8(&mut cursor)?;
        println!("ns(number of component)={}", ns);
        let mut components: Vec<Component> = Vec::new();
        for i in 0..ns {
            let csj = read_u8(&mut cursor)?;
            let tj = read_u8(&mut cursor)?;
            let tdj = tj >> 4;
            let taj = tj & 0xf;
            println!("csj(scan component selector)={} tdj(dc entropy coding selector)={} taj(ac entropy coding selector)={}", csj, tdj, taj);
            let scanC = self.scanComponents.iter().find(|&sc| sc.id == csj).unwrap();
            let acHaff = self.hafftables.iter().find(|&ht| scanC.qt_id == ht.id && ht.tc != 0).unwrap();
            let dcHaff = self.hafftables.iter().find(|&ht| scanC.qt_id == ht.id && ht.tc == 0).unwrap();
            let qTable = self.qts.iter().find(|&qt| scanC.qt_id == qt.id).unwrap();
            components.push(Component{
                hi: scanC.hi,
                vi: scanC.vi,
                acHaff: acHaff,
                dcHaff: dcHaff,
                qTable: qTable,
            })
        }
        let ss = read_u8(&mut cursor)?;
        let se = read_u8(&mut cursor)?;
        let a = read_u8(&mut cursor)?;
        let ah = a>> 4;
        let al = a & 0xf;
        println!("ss(Start of spectral or predictor selection)={} se(End of spectral selection)={}", ss, se);
        println!("ah(Successive approximation bit position high)={} al(Successive approximation bit position low or point transform)={}", ah, al);
        Ok(())
    }
    pub fn decode(&mut self) -> Result<(), Error>{
        check_soi(&mut self.reader)?;
        println!("SOI found");
        loop {
            match self.next_marker()? {
                m @ 0xe0..=0xef => self.parse_app(m-0xe0)?,
                0xdb => self.parse_dqt()?,
                0xc0 => self.parse_sof0()?,
                0xc4 => self.parse_dht()?,
                0xda => self.parse_sos()?,
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
