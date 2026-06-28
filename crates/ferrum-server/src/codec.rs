use anyhow::{Context, Result, bail};
use std::io::{self, Read, Write};

const MAX_PACKET_LENGTH: i32 = 2 * 1024 * 1024;

pub(crate) fn read_packet(reader: &mut impl Read) -> io::Result<Vec<u8>> {
    let length = read_varint_io(reader)?;
    if !(0..=MAX_PACKET_LENGTH).contains(&length) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("packet length {length} is outside allowed range"),
        ));
    }
    let mut packet = vec![0; length as usize];
    reader.read_exact(&mut packet)?;
    Ok(packet)
}

pub(crate) fn write_packet(writer: &mut impl Write, packet: &[u8]) -> io::Result<()> {
    write_varint_io(
        writer,
        i32::try_from(packet.len()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "packet length exceeds i32")
        })?,
    )?;
    writer.write_all(packet)
}

pub(crate) fn build_packet(
    packet_id: i32,
    write_body: impl FnOnce(&mut Vec<u8>) -> Result<()>,
) -> Result<Vec<u8>> {
    let mut packet = Vec::new();
    write_varint_vec(&mut packet, packet_id);
    write_body(&mut packet)?;
    Ok(packet)
}

pub(crate) fn write_string(output: &mut Vec<u8>, value: &str) -> Result<()> {
    let length = i32::try_from(value.len()).context("string length exceeds i32")?;
    write_varint_vec(output, length);
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

pub(crate) fn read_varint_io(reader: &mut impl Read) -> io::Result<i32> {
    let mut value = 0i32;
    for position in 0..5 {
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte)?;
        value |= i32::from(byte[0] & 0x7f) << (7 * position);
        if byte[0] & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "VarInt exceeds 5 bytes",
    ))
}

fn write_varint_io(writer: &mut impl Write, value: i32) -> io::Result<()> {
    let mut value = value as u32;
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        writer.write_all(&[byte])?;
        if value == 0 {
            return Ok(());
        }
    }
}

pub(crate) fn write_varint_vec(output: &mut Vec<u8>, value: i32) {
    let mut value = value as u32;
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        output.push(byte);
        if value == 0 {
            break;
        }
    }
}

pub(crate) struct PacketReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> PacketReader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    pub(crate) fn read_varint(&mut self) -> Result<i32> {
        let mut value = 0i32;
        for position in 0..5 {
            let byte = self.read_u8()?;
            value |= i32::from(byte & 0x7f) << (7 * position);
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        bail!("VarInt exceeds 5 bytes")
    }

    pub(crate) fn read_string(&mut self) -> Result<String> {
        let length = self.read_varint()?;
        if length < 0 {
            bail!("negative string length {length}");
        }
        let bytes = self.read_bytes(length as usize)?;
        String::from_utf8(bytes.to_vec()).context("string is not valid UTF-8")
    }

    pub(crate) fn read_u16(&mut self) -> Result<u16> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    pub(crate) fn read_i64(&mut self) -> Result<i64> {
        let bytes = self.read_bytes(8)?;
        Ok(i64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    #[cfg(test)]
    pub(crate) fn read_uuid_bytes(&mut self) -> Result<[u8; 16]> {
        let bytes = self.read_bytes(16)?;
        Ok([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ])
    }

    fn read_u8(&mut self) -> Result<u8> {
        Ok(*self.read_bytes(1)?.first().expect("one byte was just read"))
    }

    fn read_bytes(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self
            .cursor
            .checked_add(length)
            .context("packet cursor overflow")?;
        let bytes = self
            .bytes
            .get(self.cursor..end)
            .with_context(|| format!("packet ended while reading {length} bytes"))?;
        self.cursor = end;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn varint_round_trips_protocol_values() {
        for value in [0, 1, 127, 128, 255, 2_600, i32::MAX, -1] {
            let mut encoded = Vec::new();
            write_varint_vec(&mut encoded, value);
            let mut cursor = Cursor::new(encoded);
            assert_eq!(read_varint_io(&mut cursor).unwrap(), value);
        }
    }

    #[test]
    fn string_round_trips_utf8_payloads() {
        let mut packet = Vec::new();
        write_string(&mut packet, "Ferrum").unwrap();
        let mut reader = PacketReader::new(&packet);
        assert_eq!(reader.read_string().unwrap(), "Ferrum");
    }
}
