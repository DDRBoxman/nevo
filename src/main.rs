use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::Path;

use file_mode::ModePath;
use byteorder::{BigEndian, ReadBytesExt};

const BW_TRAILER_V1_LENGTH: i64 = 48;
const BW_TRAILER_V1_MAGIC: i64 = 4;
const BW_TRAILER_V1_TRAILER_VERSION: i64 = 5;
const BW_TRAILER_V1_COMPRESSION: i64 = 6;
const BW_TRAILER_V1_FLAGS: i64 = 8;
const BW_TRAILER_V1_CONTENTS_OFFSET: i64 = 12;
const BW_TRAILER_V1_CONTENTS_LENGTH: i64 = 16;
const BW_TRAILER_V1_SHA1: i64 = 48;

const BAKEWARE_COMPRESSION_NONE: u8 = 0;
const BAKEWARE_COMPRESSION_ZSTD: u8 = 1;

const CPIO_MAGIC: u32 = 0x070701;
const CPIO_LAST: &str = "TRAILER!!!";
const CPIO_HEADER_SIZE: usize = 110;
const CPIO_MAX_NAME_LEN: u16 = 512;

fn main() -> Result<(), Box<dyn Error>> {
    let f = File::open("./target")?;

    let trailer = read_trailer(&f)?;

    extract_all(&f, trailer);

    Ok(())
}

fn extract_all(mut f: &File, trailer: Trailer) -> Result<(), Box<dyn Error>> {
    f.seek(SeekFrom::Start(trailer.content_offset.try_into().unwrap()))?;

    let path = Path::new("./out");

    fs::create_dir_all(path)?;

    match trailer.compression {
        BAKEWARE_COMPRESSION_ZSTD => {
            let mut buf_read = std::io::BufReader::new(f);
            let mut decoder = ruzstd::StreamingDecoder::new(&mut buf_read).unwrap();

            loop {
                let reader = cpio::NewcReader::new(decoder).unwrap();
                if reader.entry().is_trailer() {
                    break;
                }

                let out_path = path.join(reader.entry().name());

                let mode = reader.entry().mode();

                if reader.entry().file_size() == 0 {
                    if !out_path.is_dir() {
                        fs::create_dir_all(&out_path)?;
                        out_path.set_mode(mode).unwrap();
                    }
                    decoder = reader.finish().unwrap();
                } else {
                    let out = std::fs::File::create(&out_path).unwrap();
                    decoder = reader.to_writer(out).unwrap();
                    out_path.set_mode(mode).unwrap();
                }
            }
        }
        BAKEWARE_COMPRESSION_NONE => todo!("No compression"),
        _ => todo!("Unknown compression"),
    }

    Ok(())
}

#[derive(Debug)]
struct Trailer {
    version: u8,
    compression: u8,
    flags: u16,
    content_offset: i32,
    content_length: i32,
    sha1: [u8; 20],
}

fn read_trailer(mut f: &File) -> Result<Trailer, Box<dyn Error>> {
    f.seek(SeekFrom::End(-BW_TRAILER_V1_MAGIC))?;

    let mut buffer = [0; 4];
    f.read_exact(&mut buffer)?;

    if buffer != "BAKE".as_bytes() {
        return Err(format!(
            "Incorrect Bakeware Magic: {:?} vs {:?}",
            buffer,
            "BAKE".as_bytes()
        )
        .into());
    }

    f.seek(SeekFrom::End(-BW_TRAILER_V1_TRAILER_VERSION))?;
    let version = f.read_u8()?;

    f.seek(SeekFrom::End(-BW_TRAILER_V1_COMPRESSION))?;
    let compression = f.read_u8()?;

    f.seek(SeekFrom::End(-BW_TRAILER_V1_FLAGS))?;
    let flags = f.read_u16::<BigEndian>()?;

    f.seek(SeekFrom::End(-BW_TRAILER_V1_CONTENTS_OFFSET))?;
    let content_offset = f.read_i32::<BigEndian>()?;

    f.seek(SeekFrom::End(-BW_TRAILER_V1_CONTENTS_LENGTH))?;
    let content_length = f.read_i32::<BigEndian>()?;

    f.seek(SeekFrom::End(-BW_TRAILER_V1_SHA1))?;
    let mut sha1 = [0; 20];
    f.read_exact(&mut sha1)?;

    Ok(Trailer {
        version,
        compression,
        flags,
        content_offset,
        content_length,
        sha1,
    })
}
