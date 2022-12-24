use std::io::{Read, Write};
use std::io::Result;
use derive_getters::Getters;

#[derive(Getters)]
pub struct VarInt {
    int_val: u64,  // Actual max is 5*7 (35 bits)
    byte_arr: [u8; 5]
}

impl VarInt {
    pub fn from_int(i: u64) -> VarInt {
        if i > 34359738367 { panic!("VarInt#from_int: int too large") }

        let mut x = i;
        let mut arr: [u8; 5]  = [0; 5];
        let mut index = 0;
        while x != 0 {
            let value: u8 = (x & 0b1111111) as u8;  // Get lowest 7 bits
            x >>= 7;
            arr[index] = (((x != 0) as u8) << 7) + value;  // Set MSb if more data to come
            index += 1;
        }

        VarInt{ int_val: i, byte_arr: arr }
    }

    pub fn from_stream(s: &mut dyn Read) -> VarInt {
        let mut val: u64 = 0;
        let mut i = 0;
        let mut iter = s.bytes();

        loop {
            let byte = iter.next().expect("Ran out of bytes trying to read VarInt").expect("Ran out of bytes trying to read VarInt2");
            // let byte = iter.next().unwrap().expect("Ran out of bytes trying to read VarInt");
            val |= ((byte & 0b01111111) as u64) << i;

            if (byte & 0b10000000) == 0 { break }

            i += 7;
            if i >= 32 { panic!("VarInt#from_stream: VarInt too large") }
        }
        VarInt::from_int(val)
    }

    pub fn write_to_stream(&self, s: &mut dyn Write) -> Result<()> {
        // Write only bytes that are used (not 0), except if VarInt is 0
        if self.int_val == 0 {
            s.write_all(&[0])?;
        } else {
            let last = self.byte_arr.iter().position(|&x| x == 0).unwrap_or(5);
            s.write_all(&self.byte_arr[0..last])?;
        }
        Ok(())
    }
}
