use std::env;
use std::io::{BufReader, Read, Cursor, Seek, Write};
use std::fs::File;
use std::iter::Iterator;
use failure::format_err;
use failure::Error;

static zigzags: [[i32;8];8] = [
    [  0,  1,  5,  6, 14, 15, 27, 28 ],
    [  2,  4,  7, 13, 16, 26, 29, 42 ],
    [  3,  8, 12, 17, 25, 30, 41, 43 ],
    [  9, 11, 18, 24, 31, 40, 44, 53 ],
    [ 10, 19, 23, 32, 39, 45, 52, 54 ],
    [ 20, 22, 33, 38, 46, 51, 55, 60 ],
    [ 21, 34, 37, 47, 50, 56, 59, 61 ],
    [ 35, 36, 48, 49, 57, 58, 62, 63 ],
];

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
fn ceildiv(d0:u64, d1:u64) -> u64 {
    (d0 + (d1 - 1)) / d1
}


fn check_soi<T:Read>(r: &mut T) -> Result<(), Error> {
    let u0 = read_u8(r)?;
    let u1 = read_u8(r)?;
    if u0 != 0xff || u1 != 0xd8 {
        return Err(format_err!("no SOI found found {} {}", u0, u1));
    }
    Ok(())
}

// Componentに回してぶん回したいのでCopyにする。何とかしたい
#[derive(Clone, Copy)]
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

struct Component {
    qTable: QuantizationTable,
    acHaff: HaffTable,
    dcHaff: HaffTable,
    hi: u8,
    vi: u8,
    prevDC: i32
}

#[derive(Clone, Copy)]
struct HaffTable {
    tc: u8,
    id: u8,
    bits: [u8;16],
    values: [u8;256], //Vecだとコピーできない…
    mincodes: [i32;16],
    maxcodes: [i32;16],
    indices: [i32;16],
}
impl HaffTable {
    pub fn new(tc:u8, id:u8, bits: [u8;16], values: [u8;256]) -> HaffTable {
        let mut mincodes = [-1;16];
        let mut maxcodes = [-1;16];
        let mut indices = [-1;16];
        let mut code = 0;
        let mut cumm = 0;
        for i in 0..16 {
            code = code << 1;
            if bits[i] > 0 {
                indices[i] = cumm as i32;
                cumm += bits[i];
                mincodes[i] = (code & ((1 << (i + 1)) - 1)) as i32;
                code += bits[i] as i32;
                maxcodes[i] = ((code - 1) & ((1 << (i + 1)) - 1)) as i32;
            }
        }
        HaffTable {
            tc:tc,
            id:id,
            bits:bits,
            values:values,
            mincodes:mincodes,
            maxcodes:maxcodes,
            indices:indices,
        }
    }
}

struct HaffDecoder {
    ptr: u8,
    buf: u8,
}

impl HaffDecoder {
    pub fn new() -> HaffDecoder {
        HaffDecoder{
            ptr: 0,
            buf: 0,
        }
    }
    pub fn parseCoeffs<T:Read>(&mut self, rd:&mut T, dcHaff: &HaffTable, acHaff: &HaffTable) -> Result<[i32;64], Error> {
        let mut buf = [0;64];
        let ssss = self.parseHaff(rd, dcHaff)?;
        buf[0]=self.readSSSSBits(ssss, rd)?;
        let mut ptr = 1;
        while ptr < 64 {
            let r = self.parseHaff(rd, acHaff)?;
            let rrrr = r >> 4;
            let ssss = r & 0xf;
            if ssss == 0 && rrrr == 0 {
                // EOB
                while ptr < 64 {
                    buf[ptr] = 0;
                    ptr+=1
                }
                break;
            } else if ssss == 0 && rrrr == 0xf {
                // ZRL
                for _ in 0..16 {
                    buf[ptr] = 0;
                    ptr+=1;
                }
            } else {
                for _ in 0..rrrr {
                    buf[ptr] = 0;
                    ptr+=1;
                }
                buf[ptr] = self.readSSSSBits(ssss, rd)?;
                ptr+=1;
            }
        }
        Ok(buf)
    }
    fn readSSSSBits<T:Read>(&mut self, ssss: u8, rd:&mut T) -> Result<i32, Error> {
        if ssss == 0 {
            return Ok(0)
        }
        let mut r = 0;
        for _ in 0..ssss {
            r = (r << 1) + self.readBit(rd)? as i32
        }
        if r < (1 << (ssss - 1)) {
            return Ok(r - (1 << ssss) + 1);
        }
        Ok(r)
    }
    fn parseHaff<T:Read>(&mut self, r:&mut T, haff: &HaffTable) -> Result<u8, Error> {
        let mut curBit = 0 as i32;
        for i in 0..16 {
            curBit = (curBit << 1) + self.readBit(r)? as i32;
            if haff.indices[i] == -1 {
                continue;
            }
            if haff.mincodes[i] <=curBit && curBit <= haff.maxcodes[i] {
                //println!("haff:{}",haff.values[(haff.indices[i] + curBit - haff.mincodes[i]) as usize]);
                return Ok(haff.values[(haff.indices[i] + curBit - haff.mincodes[i]) as usize])
            }
        }
        Err(format_err!("haff parse error"))
    }
    fn readBit<T:Read>(&mut self, r:&mut T) -> Result<u8, Error> {
        if self.ptr == 0 {
            let mut buf = [0];
            r.read_exact(&mut buf);
            self.buf = buf[0];
            self.ptr = 8;
        }
        self.ptr-=1;
        Ok((self.buf >> self.ptr) & 0x1)
    }
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
            let mut tmp_values = vec![0;valuenum as usize];
            cursor.read_exact(&mut tmp_values)?;
            let mut values = [0;256];
            for i in 0..valuenum {
                values[i as usize] = tmp_values[i as usize];
            }
            self.hafftables.push(HaffTable::new(tc, tn, bits, values))
        }
        Ok(())
    }
    fn idct(&mut self, coeffs: &[i32;64]) -> [[u8;8];8] {
        let mut zigzaged = [[0 as f64;8];8];
        for iy in 0..8 {
            for ix in 0..8 {
                zigzaged[iy][ix] = coeffs[zigzags[iy][ix] as usize] as f64;
            }
        }
        let mut sumx = [[0 as f64;8];8];
        let s2 = f64::sqrt(2.);
        for jy in 0..8 {
            for ix in 0..8 {
                let mut s:f64 = 0.;
                for jx in 0..8 {
                    let cy:f64 = if jy == 0 {1.} else {s2};
                    let cx:f64 = if jx == 0 {1.} else {s2};
                    s+=  cy*cx*(std::f64::consts::PI * ((2*ix+1)*jx) as f64 / ((2*8) as f64)).cos() * zigzaged[jy][jx];
                }
                sumx[jy][ix] = s
            }
        }
        let mut res = [[0 as u8;8];8];
        for iy in 0..8 {
            for ix in 0..8 {
                let mut s:f64=0.;
                for jy in 0..8 {
                    s+=(std::f64::consts::PI * ((2*iy+1)*jy) as f64 / ((2*8) as f64)).cos() * sumx[jy][ix];
                }
                let mut r = (s.round()) as i32;
                r = i32::max(r,0);
                r = i32::min(r,255);
                res[iy][ix] = r as u8
            }
        } 
        res
    }
    fn parseBlock(&mut self, decoder: &mut HaffDecoder, dcHaff: &HaffTable, acHaff: &HaffTable, qTable: &QuantizationTable, prevDC: i32) -> Result<(i32, [i32;64]), Error> {
        let mut coeffs = decoder.parseCoeffs(&mut self.reader, dcHaff, acHaff)?;
        let curDC = coeffs[0];
        for i in 0..64 {
            coeffs[i] = coeffs[i] * (qTable.table[i] as i32)
        }
        let idcted = self.idct(&coeffs);
        coeffs[0]+=prevDC;
        Ok((curDC, coeffs))
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
            let scanC = self.scanComponents.iter().find(|&sc| sc.id == csj).ok_or(format_err!("cannot found from csj"))?;
            let acHaff = self.hafftables.iter().find(|&ht| scanC.qt_id == ht.id && ht.tc != 0).ok_or(format_err!("cannot found achafftable"))?;
            let dcHaff = self.hafftables.iter().find(|&ht| scanC.qt_id == ht.id && ht.tc == 0).ok_or(format_err!("cannot found dchafftable"))?;
            let qTable = self.qts.iter().find(|&qt| scanC.qt_id == qt.id).ok_or(format_err!("cannot found qtable"))?;
            components.push(Component{
                hi: scanC.hi,
                vi: scanC.vi,
                acHaff: *acHaff,
                dcHaff: *dcHaff,
                qTable: *qTable,
                prevDC: 0,
            })
        }
        let ss = read_u8(&mut cursor)?;
        let se = read_u8(&mut cursor)?;
        let a = read_u8(&mut cursor)?;
        let ah = a>> 4;
        let al = a & 0xf;
        println!("ss(Start of spectral or predictor selection)={} se(End of spectral selection)={}", ss, se);
        println!("ah(Successive approximation bit position high)={} al(Successive approximation bit position low or point transform)={}", ah, al);
        let maxHi = components.iter().fold(0, |acc, v| u8::max(acc,v.hi));
        let maxVi = components.iter().fold(0, |acc, v| u8::max(acc,v.vi));
        let mcuX = ceildiv(self.width as u64, (maxHi as u64)*8);
        let mcuY = ceildiv(self.height as u64, (maxVi as u64)*8);
        //println!("width={} height={} mcuX={} mcuY={}", self.width, self.height, mcuX, mcuY);
        let mut decoder = HaffDecoder::new();
        for iy in 0..mcuY {
            for ix in 0..mcuX {
                //parseMCU
                for i in 0..components.len() {
                    let c = &mut components[i];
                    for iv in 0..c.vi {
                        for ih in 0..c.hi {
                            //println!("MCU ix={} iy={} ih={} iv={}", ix, iy, ih, iv);
                            let (dc, parsed) = self.parseBlock(&mut decoder, &c.dcHaff, &c.acHaff, &c.qTable, c.prevDC)?;
                            //c.prevDC = dc;
                        }
                    }
                }
            }
        }
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
                0xd9 => {println!("reached EOI");return Ok(())}
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
