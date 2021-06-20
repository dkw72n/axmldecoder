//!Decoder for the binary XML format used by Android.
//!
//!This library implements the minimal amount of parsing required obtain
//!useful information from a binary `AndroidManifest.xml`. It does not
//!support parsing generic binary XML documents and does not have
//!support for decoding resource identifiers. In return, the compiled
//!footprint of the library is _much_ lighter as it does not have to
//!link in Android's `resources.arsc` file.
//!
//!For a full-featured Rust binary XML parser,
//![abxml-rs](https://github.com/SUPERAndroidAnalyzer/abxml-rs)
//!is highly recommended if it is acceptable to link a 30MB `resources.arsc`
//!file into your compiled binary.
//!
//!Please file an issue with the relevant binary `AndroidManifest.xml` if
//!if any issues are encountered.

mod binaryxml;
mod resource_value;
mod stringpool;
mod xml;

use byteorder::ByteOrder;
use byteorder::LittleEndian;
use byteorder::WriteBytesExt;
use std::io::{Read, Seek, Write};
use thiserror::Error;
use std::fs::File;

pub use crate::binaryxml::BinaryXmlDocument;
pub use crate::xml::{Cdata, Element, Node, XmlDocument};

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("invalid file")]
    InvalidFile,

    #[error("missing StringPool chunk")]
    MissingStringPoolChunk,

    #[error("missing ResourceMap chunk")]
    MissingResourceMapChunk,

    #[error("StringPool missing index: {0}")]
    StringNotFound(u32),

    #[error("Namespace missing: {0}")]
    NamespaceNotFound(String),

    #[error("ResourceMap missing index: {0}")]
    ResourceIdNotFound(u32),

    #[error("Unknown resource string: {0}")]
    UnknownResourceString(u32),

    #[error(transparent)]
    Utf8StringParseError(std::string::FromUtf8Error),

    #[error(transparent)]
    Utf16StringParseError(std::string::FromUtf16Error),

    #[error(transparent)]
    IoError(std::io::Error),
}

///Parses an Android binary XML and returns a [XmlDocument] object.
///
///```rust
///use axmldecoder::parse;
///# use axmldecoder::ParseError;
///# let manifest_file = "examples/AndroidManifest.xml";
///let mut f = std::fs::File::open(manifest_file).unwrap();
///parse(&mut f)?;
///# Ok::<(), ParseError>(())
///```
pub fn parse<F: Read + Seek>(input: &mut F) -> Result<XmlDocument, ParseError> {
    let binaryxml = BinaryXmlDocument::read_from_file(input)?;

    //let mut out = File::create("test.xml").unwrap();
    // binaryxml.write_to_file(&mut out).unwrap();

    XmlDocument::new(
        binaryxml.elements,
        binaryxml.string_pool,
        binaryxml.resource_map,
    )
}

fn read_u8<F: Read + Seek>(input: &mut F) -> Result<u8, ParseError> {
    let mut buf = [0; 1];
    input.read_exact(&mut buf).map_err(ParseError::IoError)?;

    Ok(buf[0])
}

fn read_u16<F: Read + Seek>(input: &mut F) -> Result<u16, ParseError> {
    let mut buf = [0; 2];
    input.read_exact(&mut buf).map_err(ParseError::IoError)?;

    Ok(LittleEndian::read_u16(&buf))
}

fn read_u32<F: Read + Seek>(input: &mut F) -> Result<u32, ParseError> {
    let mut buf = [0; 4];
    input.read_exact(&mut buf).map_err(ParseError::IoError)?;

    Ok(LittleEndian::read_u32(&buf))
}

fn write_u8<F: Write + Seek>(output: &mut F, v: u8) -> Result<usize, std::io::Error> {
    output.write_u8(v)?;
    Ok(1)
}

fn write_u16<F: Write + Seek>(output: &mut F, v: u16) -> Result<usize, std::io::Error> {
    output.write_u16::<LittleEndian>(v)?;
    Ok(2)
}

fn write_u32<F: Write + Seek>(output: &mut F, v: u32) -> Result<usize, std::io::Error> {
    output.write_u32::<LittleEndian>(v)?;
    Ok(4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::path::PathBuf;

    /*
    #[test]
    fn test_parse() {
        let mut examples = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        examples.push("examples");

        for entry in std::fs::read_dir(examples).unwrap() {
            let entry = entry.unwrap();
            let mut f = File::open(entry.path()).unwrap();
            parse(&mut f).expect(&format!("{} failed to parse", entry.path().display()));
        }
    }
    */


    #[test]
    fn test_u8_rw() {
        let src = [42u8;1];
        let mut dst: Vec<u8> = vec![];
        let mut cursor = std::io::Cursor::new(src);
        let v = read_u8(&mut cursor).unwrap();
        assert_eq!(v, 42u8);
        let mut cursor = std::io::Cursor::new(&mut dst);
        let n = write_u8(&mut cursor, v).unwrap();
        assert_eq!(n, 1);
        assert_eq!(&src, dst.as_slice());

    }

    #[test]
    fn test_u16_rw() {
        let src = [43u8, 99u8];
        let mut dst: Vec<u8> = vec![];
        let mut cursor = std::io::Cursor::new(src);
        let v = read_u16(&mut cursor).unwrap();
        assert_eq!(v, 99 * 256 + 43);
        let mut cursor = std::io::Cursor::new(&mut dst);
        let n = write_u16(&mut cursor, v).unwrap();
        assert_eq!(n, 2);
        assert_eq!(&src, dst.as_slice());
    }

    #[test]
    fn test_u32_rw() {
        let src = [0xfau8, 0xceu8, 0xb0u8, 0x0cu8];
        let mut dst: Vec<u8> = vec![];
        let mut cursor = std::io::Cursor::new(src);
        let v = read_u32(&mut cursor).unwrap();
        assert_eq!(v, 0x0cb0cefa);
        let mut cursor = std::io::Cursor::new(&mut dst);
        let n = write_u32(&mut cursor, v).unwrap();
        assert_eq!(n, 4);
        assert_eq!(&src, dst.as_slice());
    }
}
