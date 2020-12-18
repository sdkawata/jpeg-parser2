
use std::io::Read;
use failure::format_err;
use failure::Error;

pub struct HaffTable {
    pub tc: u8,
    pub id: u8,
    bits: [u8;16],
    values: [u8;256],
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

pub struct HaffDecoder {
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
    pub fn reset(&mut self) {
        self.ptr = 0;
        self.buf = 0;
    }
    pub fn parse_coeffs<T:Read>(&mut self, rd:&mut T, dc_haff: &HaffTable, ac_haff: &HaffTable) -> Result<[i32;64], Error> {
        let mut buf = [0;64];
        let ssss = self.parse_haff(rd, dc_haff)?;
        buf[0]=self.read_ssss_bits(ssss, rd)?;
        let mut ptr = 1;
        while ptr < 64 {
            let r = self.parse_haff(rd, ac_haff)?;
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
                buf[ptr] = self.read_ssss_bits(ssss, rd)?;
                ptr+=1;
            }
        }
        Ok(buf)
    }
    fn read_ssss_bits<T:Read>(&mut self, ssss: u8, rd:&mut T) -> Result<i32, Error> {
        if ssss == 0 {
            return Ok(0)
        }
        let mut r = 0;
        for _ in 0..ssss {
            r = (r << 1) + self.read_bit(rd)? as i32
        }
        if r < (1 << (ssss - 1)) {
            return Ok(r - (1 << ssss) + 1);
        }
        Ok(r)
    }
    fn parse_haff<T:Read>(&mut self, r:&mut T, haff: &HaffTable) -> Result<u8, Error> {
        let mut cur_bit = 0 as i32;
        for i in 0..16 {
            cur_bit = (cur_bit << 1) + self.read_bit(r)? as i32;
            if haff.indices[i] == -1 {
                continue;
            }
            if haff.mincodes[i] <=cur_bit && cur_bit <= haff.maxcodes[i] {
                //println!("haff:{}",haff.values[(haff.indices[i] + cur_bit - haff.mincodes[i]) as usize]);
                return Ok(haff.values[(haff.indices[i] + cur_bit - haff.mincodes[i]) as usize])
            }
        }
        Err(format_err!("haff parse error"))
    }
    fn read_bit<T:Read>(&mut self, r:&mut T) -> Result<u8, Error> {
        if self.ptr == 0 {
            let mut buf = [0];
            r.read_exact(&mut buf)?;
            self.buf = buf[0];
            if buf[0] == 0xff {
                r.read_exact(&mut buf)?;
                if buf[0] != 0x00 {
                    return Err(format_err!("found marker {:x} while reading image", buf[0]));
                }
                // println!("skpped 0xff 0x00");
            }
            self.ptr = 8;
        }
        self.ptr-=1;
        Ok((self.buf >> self.ptr) & 0x1)
    }
}
