mod haff;

use failure::format_err;
use failure::Error;
use haff::HaffDecoder;
use haff::HaffTable;
use log::info;
use std::io::{Cursor, Read, Write};
use std::iter::Iterator;

type Result<T> = std::result::Result<T, Error>;

static ZIGZAGS: [[i32; 8]; 8] = [
    [0, 1, 5, 6, 14, 15, 27, 28],
    [2, 4, 7, 13, 16, 26, 29, 42],
    [3, 8, 12, 17, 25, 30, 41, 43],
    [9, 11, 18, 24, 31, 40, 44, 53],
    [10, 19, 23, 32, 39, 45, 52, 54],
    [20, 22, 33, 38, 46, 51, 55, 60],
    [21, 34, 37, 47, 50, 56, 59, 61],
    [35, 36, 48, 49, 57, 58, 62, 63],
];

fn read_u8<T: Read>(r: &mut T) -> Result<u8> {
    let mut buf = [0; 1];
    r.read_exact(&mut buf)?;
    Ok(buf[0])
}
fn read_u16<T: Read>(r: &mut T) -> Result<u16> {
    let mut buf = [0; 2];
    r.read_exact(&mut buf)?;
    Ok((buf[0] as u16) * 0x100 + (buf[1] as u16))
}

fn tr255(i: f64) -> i32 {
    i32::min(i32::max(i as i32, 0), 255)
}
fn ceildiv(d0: u64, d1: u64) -> u64 {
    (d0 + (d1 - 1)) / d1
}

fn check_soi<T: Read>(r: &mut T) -> Result<()> {
    let u0 = read_u8(r)?;
    let u1 = read_u8(r)?;
    if u0 != 0xff || u1 != 0xd8 {
        return Err(format_err!("no SOI found found {} {}", u0, u1));
    }
    Ok(())
}

struct QuantizationTable {
    id: u8,
    table: [u8; 64],
}

struct ScanComponent {
    id: u8,
    hi: u8,
    vi: u8,
    qt_id: u8,
}

struct Component {
    qt_id: u8,
    tdj: u8,
    taj: u8,
    hi: u8,
    vi: u8,
    prev_dc: i32,
    plane: Vec<u8>,
    stride: i32,
}

pub struct Decoder<T: Read> {
    reader: T,
    qts: Vec<QuantizationTable>,
    hafftables: Vec<HaffTable>,
    scan_components: Vec<ScanComponent>,
    height: u16,
    width: u16,
    components: Vec<Component>,
    restart_interval: u16,
}

impl<T: Read> Decoder<T> {
    pub fn new(reader: T) -> Decoder<T> {
        Decoder {
            reader: reader,
            qts: Vec::new(),
            hafftables: Vec::new(),
            height: 0,
            width: 0,
            scan_components: Vec::new(),
            components: Vec::new(),
            restart_interval: 0,
        }
    }
    fn next_marker(&mut self) -> Result<u8> {
        let mut ignored = 0;
        loop {
            let u0 = read_u8(&mut self.reader)?;
            if u0 == 0xff {
                let u1 = read_u8(&mut self.reader)?;
                if u1 != 0x00 {
                    if ignored != 0 {
                        info!("extra {} byte before marker {:x}", ignored, u1);
                    }
                    return Ok(u1);
                }
                ignored += 1;
            }
            ignored += 1;
        }
    }
    fn read_marker_content(&mut self) -> Result<Vec<u8>> {
        let size = read_u16(&mut self.reader)?;
        let mut buf = vec![0; size as usize - 2];
        self.reader.read_exact(&mut buf)?;
        Ok(buf)
    }
    fn parse_app0(&mut self) -> Result<()> {
        let content = self.read_marker_content()?;
        info!("APP0 size={}", content.len());
        let mut cursor = Cursor::new(content);
        let mut prefix = [0; 5];
        cursor.read_exact(&mut prefix)?;
        match std::str::from_utf8(&prefix)? {
            "JFIF\0" => {
                info!("JFIF APP0");
                let version = read_u16(&mut cursor)?;
                let unit = read_u8(&mut cursor)?;
                let xdensity = read_u16(&mut cursor)?;
                let ydensity = read_u16(&mut cursor)?;
                let xthumbnail = read_u8(&mut cursor)?;
                let ythumbnail = read_u8(&mut cursor)?;
                info!("version={}", version);
                info!(
                    "unit={}({}) xdensity={} ydensity={}",
                    unit,
                    match unit {
                        0 => "no unit",
                        1 => "dpi",
                        2 => "dpc",
                        _ => "?",
                    },
                    xdensity,
                    ydensity
                );
                info!("xhtumnail={} ythumbnail={}", xthumbnail, ythumbnail);
            }
            "JFXX\0" => info!("JFXX APP0"),
            _ => (),
        }
        Ok(())
    }
    fn parse_app(&mut self, index: u8) -> Result<()> {
        let content = self.read_marker_content()?;
        info!("APP{} size={}", index, content.len());
        Ok(())
    }
    fn parse_dqt(&mut self) -> Result<()> {
        let content = self.read_marker_content()?;
        let len = content.len() as u64;
        info!("DQT size={}", len);
        let mut cursor = Cursor::new(content);
        while len > cursor.position() {
            let flag = read_u8(&mut cursor)?;
            let pq = flag >> 4;
            let tq = flag & 0xf;
            info!("pq(presision)={} tq(destination identifier)={}", pq, tq);
            let mut buf = [0; 64];
            cursor.read_exact(&mut buf)?;
            self.qts.push(QuantizationTable { id: tq, table: buf })
        }
        Ok(())
    }
    fn parse_sof0(&mut self) -> Result<()> {
        let content = self.read_marker_content()?;
        info!("SOF0 size={}", content.len());
        let mut r = Cursor::new(content);
        let p = read_u8(&mut r)?;
        let y = read_u16(&mut r)?;
        let x = read_u16(&mut r)?;
        let nf = read_u8(&mut r)?;
        info!(
            "p(presision)={} y(lines)={} x(samples per line)={} nf(number of components)={}",
            p, y, x, nf
        );
        self.height = y;
        self.width = x;
        for _i in 0..nf {
            let ci = read_u8(&mut r)?;
            let hvi = read_u8(&mut r)?;
            let tqi = read_u8(&mut r)?;
            let hi = hvi >> 4;
            let vi = hvi & 0xf;
            info!(
                "ci(id)={} hi,vi(sampling factor)={},{} tqi(dqt selector)={}",
                ci, hi, vi, tqi
            );
            self.scan_components.push(ScanComponent {
                id: ci,
                hi: hi,
                vi: vi,
                qt_id: tqi,
            })
        }
        Ok(())
    }
    fn parse_dht(&mut self) -> Result<()> {
        let content = self.read_marker_content()?;
        let len = content.len() as u64;
        info!("DHT size={}", len);
        let mut cursor = Cursor::new(content);
        while len > cursor.position() {
            let flag = read_u8(&mut cursor)?;
            let tc = flag >> 4;
            let tn = flag & 0xf;
            info!(
                "tc={}({}) th(destination identifier)={}",
                tc,
                if tc == 0 { "DC" } else { "AC" },
                tn
            );
            let mut bits = [0; 16];
            cursor.read_exact(&mut bits)?;
            let valuenum = bits.iter().fold(0, |acc, a| acc + a);
            let mut tmp_values = vec![0; valuenum as usize];
            cursor.read_exact(&mut tmp_values)?;
            let mut values = [0; 256];
            for i in 0..valuenum {
                values[i as usize] = tmp_values[i as usize];
            }
            self.hafftables.push(HaffTable::new(tc, tn, bits, values))
        }
        Ok(())
    }
    fn parse_dri(&mut self) -> Result<()> {
        let content = self.read_marker_content()?;
        let len = content.len() as u64;
        let mut cursor = Cursor::new(content);
        let ri = read_u16(&mut cursor)?;
        self.restart_interval = ri;
        info!("DRI size={} ri={}", len, ri);
        Ok(())
    }
    fn idct(&mut self, coeffs: &[i32; 64]) -> [[u8; 8]; 8] {
        let mut zigzaged = [[0 as f64; 8]; 8];
        for iy in 0..8 {
            for ix in 0..8 {
                zigzaged[iy][ix] = coeffs[ZIGZAGS[iy][ix] as usize] as f64;
            }
        }
        let mut sumx = [[0 as f64; 8]; 8];
        let s2 = f64::sqrt(2.);
        for jy in 0..8 {
            for ix in 0..8 {
                let mut s: f64 = 0.;
                for jx in 0..8 {
                    let cy: f64 = if jy == 0 { 1. } else { s2 };
                    let cx: f64 = if jx == 0 { 1. } else { s2 };
                    s += cy
                        * cx
                        * (std::f64::consts::PI * ((2 * ix + 1) * jx) as f64 / ((2 * 8) as f64))
                            .cos()
                        * zigzaged[jy][jx];
                }
                sumx[jy][ix] = s
            }
        }
        let mut res = [[0 as u8; 8]; 8];
        for iy in 0..8 {
            for ix in 0..8 {
                let mut s: f64 = 0.;
                for jy in 0..8 {
                    s += (std::f64::consts::PI * ((2 * iy + 1) * jy) as f64 / ((2 * 8) as f64))
                        .cos()
                        * sumx[jy][ix];
                }
                let mut r = ((s / 8.).round()) as i32 + 128;
                r = i32::max(r, 0);
                r = i32::min(r, 255);
                res[iy][ix] = r as u8
            }
        }
        res
    }
    fn parse_block(
        &mut self,
        decoder: &mut HaffDecoder,
        qt_id: u8,
        tdj: u8,
        taj: u8,
        prev_dc: i32,
    ) -> Result<(i32, [[u8; 8]; 8])> {
        let ac_haff = self
            .hafftables
            .iter()
            .find(|&ht| taj == ht.id && ht.tc != 0)
            .ok_or(format_err!("cannot found ac_hafftable"))?;
        let dc_haff = self
            .hafftables
            .iter()
            .find(|&ht| tdj == ht.id && ht.tc == 0)
            .ok_or(format_err!("cannot found dc_hafftable"))?;
        let q_table = self
            .qts
            .iter()
            .find(|&qt| qt_id == qt.id)
            .ok_or(format_err!("cannot found q_table"))?;
        let mut coeffs = decoder.parse_coeffs(&mut self.reader, dc_haff, ac_haff)?;
        coeffs[0] += prev_dc;
        let cur_dc = coeffs[0];
        for i in 0..64 {
            coeffs[i] = coeffs[i] * (q_table.table[i] as i32)
        }
        //for i in 0..64 {print!("{},", coeffs[i]);};info!("");
        let idcted = self.idct(&coeffs);
        //info!("{:?}", idcted);
        Ok((cur_dc, idcted))
    }
    fn parse_sos(&mut self) -> Result<()> {
        let content = self.read_marker_content()?;
        info!("SOS size={}", content.len());
        let mut cursor = Cursor::new(content);
        let ns = read_u8(&mut cursor)?;
        info!("ns(number of component)={}", ns);
        let mut components: Vec<Component> = Vec::new();
        for _i in 0..ns {
            let csj = read_u8(&mut cursor)?;
            let tj = read_u8(&mut cursor)?;
            let tdj = tj >> 4;
            let taj = tj & 0xf;
            info!("csj(scan component selector)={} tdj(dc entropy coding selector)={} taj(ac entropy coding selector)={}", csj, tdj, taj);
            let scan_c = self
                .scan_components
                .iter()
                .find(|&sc| sc.id == csj)
                .ok_or(format_err!("cannot found from csj"))?;
            components.push(Component {
                hi: scan_c.hi,
                vi: scan_c.vi,
                qt_id: scan_c.qt_id,
                tdj: tdj,
                taj: taj,
                prev_dc: 0,
                plane: Vec::new(),
                stride: 0,
            })
        }
        self.components = components;
        let ss = read_u8(&mut cursor)?;
        let se = read_u8(&mut cursor)?;
        let a = read_u8(&mut cursor)?;
        let ah = a >> 4;
        let al = a & 0xf;
        info!(
            "ss(Start of spectral or predictor selection)={} se(End of spectral selection)={}",
            ss, se
        );
        info!("ah(Successive approximation bit position high)={} al(Successive approximation bit position low or point transform)={}", ah, al);
        let max_hi = self.components.iter().fold(0, |acc, v| u8::max(acc, v.hi));
        let max_vi = self.components.iter().fold(0, |acc, v| u8::max(acc, v.vi));
        let mcu_x = ceildiv(self.width as u64, (max_hi as u64) * 8);
        let mcu_y = ceildiv(self.height as u64, (max_vi as u64) * 8);
        //info!("width={} height={} mcu_x={} mcu_y={}", self.width, self.height, mcu_x, mcu_y);
        for i in 0..self.components.len() {
            self.components[i].stride = mcu_x as i32 * 8 * (self.components[i].hi as i32);
            let height = mcu_y as i32 * 8 * (self.components[i].vi as i32);
            //info!("i={} stride={} height={}", i, self.components[i].stride, height);
            self.components[i].plane = vec![0; (height * self.components[i].stride) as usize];
        }
        let mut decoder = HaffDecoder::new();
        let mut mcu_ptr = 0;
        for iy in 0..mcu_y {
            for ix in 0..mcu_x {
                //parseMCU
                //check RST
                if mcu_ptr > 0 && self.restart_interval != 0 && mcu_ptr % self.restart_interval == 0 {
                    let next_marker = self.next_marker()?;
                    let expected = ((mcu_ptr / self.restart_interval + 7) % 8) as u8;
                    if next_marker == expected + 0xd0 {
                        //info!("RST {:x}", expected);
                        decoder.reset();
                        for i in 0..self.components.len() {
                            self.components[i].prev_dc = 0;
                        }
                    } else if next_marker >= 0xd0 && next_marker <= 0xd7 {
                        return Err(format_err!(
                            "expect RST {:x} found RST {:x}",
                            expected,
                            next_marker - 0xd0
                        ));
                    } else {
                        return Err(format_err!(
                            "seek RST marker found marker {:x}",
                            next_marker
                        ));
                    }
                }
                mcu_ptr += 1;
                for i in 0..self.components.len() {
                    for iv in 0..self.components[i].vi {
                        for ih in 0..self.components[i].hi {
                            //info!("MCU ix={} iy={} ih={} iv={}", ix, iy, ih, iv);
                            let (dc, parsed) = self.parse_block(
                                &mut decoder,
                                self.components[i].qt_id,
                                self.components[i].taj,
                                self.components[i].tdj,
                                self.components[i].prev_dc,
                            )?;
                            let c = &mut self.components[i];
                            c.prev_dc = dc;
                            let offset_x = ix as i32 * 8 * (c.hi as i32) + (ih as i32) * 8;
                            let offset_y = iy as i32 * 8 * (c.vi as i32) + (iv as i32) * 8;
                            for iy in 0..8 {
                                for ix in 0..8 {
                                    let offset =
                                        (offset_x + ix + (offset_y + iy) * c.stride) as usize;
                                    c.plane[offset] = parsed[iy as usize][ix as usize];
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
    pub fn get_rgb_vec(&self, alpha: bool) -> Vec<u8> {
        let max_hi = self.components.iter().fold(0, |acc, v| u8::max(acc, v.hi));
        let max_vi = self.components.iter().fold(0, |acc, v| u8::max(acc, v.vi));
        let mut vec = Vec::with_capacity(self.height as usize * self.width as usize * 3);
        for iy in 0..self.height {
            for ix in 0..self.width {
                let mut v = [0.; 3];
                for k in 0..3 {
                    let c = &self.components[k];
                    let offset_x = ix as i32 * c.hi as i32 / max_hi as i32;
                    let offset_y = iy as i32 * c.vi as i32 / max_vi as i32;
                    v[k] = c.plane[(offset_y * c.stride + offset_x) as usize] as f64;
                }
                let r = tr255(v[0] + 1.402 * (v[2] - 128.));
                let g = tr255(v[0] - 0.34414 * (v[1] - 128.) - 0.71414 * (v[2] - 128.));
                let b = tr255(v[0] + 1.772 * (v[1] - 128.));
                vec.push(r as u8);
                vec.push(g as u8);
                vec.push(b as u8);
                if alpha {
                    vec.push(255)
                }
            }
        }
        vec
    }
    pub fn outputppm<T2: Write>(&self, w: &mut T2) -> Result<()> {
        writeln!(w, "P6")?;
        writeln!(w, "{} {}", self.width, self.height)?;
        writeln!(w, "255")?;
        w.write(&self.get_rgb_vec(false))?;
        Ok(())
    }
    pub fn decode(&mut self) -> Result<()> {
        check_soi(&mut self.reader)?;
        info!("SOI found");
        loop {
            match self.next_marker()? {
                0xe0 => self.parse_app0()?,
                m @ 0xe1..=0xef => self.parse_app(m - 0xe0)?,
                0xdb => self.parse_dqt()?,
                0xc0 => self.parse_sof0()?,
                0xc4 => self.parse_dht()?,
                0xda => self.parse_sos()?,
                0xdd => self.parse_dri()?,
                0xd9 => {
                    info!("reached EOI");
                    return Ok(());
                }
                m => return Err(format_err!("unknown marker {:x}", m)),
            }
        }
    }
    pub fn get_height(&self) -> u16 {
        self.height
    }
    pub fn get_width(&self) -> u16 {
        self.width
    }
}
