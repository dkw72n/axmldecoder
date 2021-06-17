use std::convert::{TryFrom, TryInto};
use std::io::{Read, Seek, Write, SeekFrom};

use crate::resource_value::ResourceValue;
use crate::stringpool::StringPool;
use crate::{ParseError, read_u16, read_u32, write_u16, write_u32};
use byteorder::WriteBytesExt;
use num_enum::{TryFromPrimitive, IntoPrimitive};

pub(crate) struct BinaryXmlDocument {
    pub(crate) elements: Vec<XmlElement>,
    pub(crate) string_pool: StringPool,
    pub(crate) resource_map: Vec<u32>,
}

impl BinaryXmlDocument {
    pub(crate) fn read_from_file<F: Read + Seek>(input: &mut F) -> Result<Self, ParseError> {
        let header = ChunkHeader::read_from_file(input)?;

        if header.typ != ResourceType::Xml {
            return Err(ParseError::InvalidFile);
        }

        let mut elements = Vec::new();
        let mut string_pool = None;
        let mut resource_map = None;

        loop {
            let header = ChunkHeader::read_from_file(input);
            if let Err(ParseError::IoError(_)) = &header {
                break;
            }
            let header = header?;

            match header.typ {
                ResourceType::StringPool => {
                    string_pool = Some(StringPool::read_from_file(input, &header)?);
                }
                ResourceType::XmlResourceMap => {
                    resource_map = Some(parse_resource_map(input, &header)?);
                }
                ResourceType::XmlStartNameSpace => {
                    elements.push(XmlElement::XmlStartNameSpace(
                        XmlStartNameSpace::read_from_file(input, &header)?,
                    ));
                }
                ResourceType::XmlEndNameSpace => {
                    elements.push(XmlElement::XmlEndNameSpace(
                        XmlEndNameSpace::read_from_file(input, &header)?,
                    ));
                }
                ResourceType::XmlStartElement => {
                    elements.push(XmlElement::XmlStartElement(
                        XmlStartElement::read_from_file(input, &header)?,
                    ));
                }
                ResourceType::XmlEndElement => {
                    elements.push(XmlElement::XmlEndElement(XmlEndElement::read_from_file(
                        input, &header,
                    )?));
                }
                ResourceType::XmlCdata => {
                    elements.push(XmlElement::XmlCdata(XmlCdata::read_from_file(
                        input, &header,
                    )?));
                }
                _ => return Err(ParseError::InvalidFile),
            }
        }

        Ok(Self {
            elements,
            string_pool: string_pool.ok_or(ParseError::MissingStringPoolChunk)?,
            resource_map: resource_map.ok_or(ParseError::MissingResourceMapChunk)?,
        })
    }

    pub(crate) fn write_to_file<F: Write + Seek>(self:&Self, output: &mut F) -> Result<usize, std::io::Error> {
        let header = ChunkHeader {
            typ: ResourceType::Xml,
            header_size: 8,
            size: 0,
        };
        let offset = output.seek(SeekFrom::Current(0))?;
        let n = header.write_to_file(output)?;
        let mut n = self.string_pool.write_to_file(output)? + n;
        // let n = self.resource_map.write_to_file(output)? + n;
        let resource_header = ChunkHeader {
            typ: ResourceType::XmlResourceMap,
            header_size: 8,
            size: self.resource_map.len() as u32 * 4 + 8
        };
        n += resource_header.write_to_file(output)?;
        for i in &self.resource_map {
            n += write_u32(output,*i)?;
        }
        for el in &self.elements {
            n += el.write_to_file(output)?;
        }
        output.seek(SeekFrom::Start(offset + 4))?;
        write_u32(output,n as u32)?;
        Ok(n)
    }
}

#[repr(u16)]
#[derive(Debug, PartialEq, Copy, Clone, TryFromPrimitive, IntoPrimitive)]
pub(crate) enum ResourceType {
    NullType = 0x000,
    StringPool = 0x0001,
    Table = 0x0002,
    Xml = 0x0003,
    XmlStartNameSpace = 0x0100,
    XmlEndNameSpace = 0x101,
    XmlStartElement = 0x0102,
    XmlEndElement = 0x0103,
    XmlCdata = 0x0104,
    XmlLastChunk = 0x017f,
    XmlResourceMap = 0x0180,
    TablePackage = 0x0200,
    TableType = 0x0201,
    TableTypeSpec = 0x0202,
    TableLibrary = 0x0203,
}

#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub(crate) struct ChunkHeader {
    pub(crate) typ: ResourceType,
    pub(crate) header_size: u16,
    pub(crate) size: u32,
}

impl ChunkHeader {
    pub(crate) fn read_from_file<F: Read + Seek>(input: &mut F) -> Result<Self, ParseError> {
        let typ = ResourceType::try_from(read_u16(input)?).map_err(|_| ParseError::InvalidFile)?;
        let header_size = read_u16(input)?;
        let size = read_u32(input)?;

        let header = ChunkHeader {
            typ,
            header_size,
            size,
        };

        Ok(header)
    }

    pub(crate) fn write_to_file<F: Write + Seek>(self:&Self, output: &mut F) -> Result<usize, std::io::Error> {
        let t: u16 = self.typ.try_into().unwrap();
        let n = write_u16(output, t)?;
        let n = write_u16(output, self.header_size)? + n;
        let n = write_u32(output, self.size)? + n;

        Ok(n)
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
pub(crate) enum XmlElement {
    XmlStartNameSpace(XmlStartNameSpace),
    XmlEndNameSpace(XmlEndNameSpace),
    XmlStartElement(XmlStartElement),
    XmlEndElement(XmlEndElement),
    XmlCdata(XmlCdata),
}

pub(crate) fn parse_resource_map<F: Read + Seek>(
    input: &mut F,
    header: &ChunkHeader,
) -> Result<Vec<u32>, ParseError> {
    let id_count = (header.size - u32::from(header.header_size)) / 4;

    let mut ids = Vec::with_capacity(usize::try_from(id_count).unwrap());
    for _ in 0..id_count {
        ids.push(read_u32(input)?);
    }

    Ok(ids)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct XmlNodeHeader {
    pub(crate) chunk_header: ChunkHeader,
    pub(crate) line_no: u32,
    pub(crate) comment: u32,
}

impl XmlNodeHeader {
    fn read_from_file<F: Read + Seek>(
        input: &mut F,
        chunk_header: &ChunkHeader,
    ) -> Result<Self, ParseError> {
        let chunk_header = chunk_header.clone();
        let line_no = read_u32(input)?;
        let comment = read_u32(input)?;

        let header = Self {
            chunk_header,
            line_no,
            comment,
        };

        Ok(header)
    }
    fn write_to_file<F: Write + Seek>(self:&Self, output: &mut F) -> Result<usize, std::io::Error> {
        let n = self.chunk_header.write_to_file(output)?;
        let n = n + write_u32(output,self.line_no)?;
        let n = n + write_u32(output,self.comment)?;
        Ok(n)
    }
}

#[derive(Debug)]
pub(crate) struct XmlStartNameSpace {
    pub(crate) header: XmlNodeHeader,
    pub(crate) prefix: u32,
    pub(crate) uri: u32,
}

impl XmlStartNameSpace {
    pub(crate) fn read_from_file<F: Read + Seek>(
        input: &mut F,
        chunk_header: &ChunkHeader,
    ) -> Result<Self, ParseError> {
        let header = XmlNodeHeader::read_from_file(input, &chunk_header)?;
        let prefix = read_u32(input)?;
        let uri = read_u32(input)?;

        let node = Self {
            header,
            prefix,
            uri,
        };

        Ok(node)
    }
    pub(crate) fn write_to_file<F: Write + Seek>(self:&Self, output: &mut F) -> Result<usize, std::io::Error> {
        let n = self.header.write_to_file(output)?;
        let n = n + write_u32(output, self.prefix)?;
        let n = n + write_u32(output, self.uri)?;
        Ok(n)
    }
}

#[derive(Debug)]
pub(crate) struct XmlEndNameSpace {
    pub(crate) header: XmlNodeHeader,
    pub(crate) prefix: u32,
    pub(crate) uri: u32,
}

impl XmlEndNameSpace {
    pub(crate) fn read_from_file<F: Read + Seek>(
        input: &mut F,
        chunk_header: &ChunkHeader,
    ) -> Result<Self, ParseError> {
        let header = XmlNodeHeader::read_from_file(input, &chunk_header)?;
        let prefix = read_u32(input)?;
        let uri = read_u32(input)?;

        let node = Self {
            header,
            prefix,
            uri,
        };

        Ok(node)
    }
    pub(crate) fn write_to_file<F: Write + Seek>(self:&Self, output: &mut F) -> Result<usize, std::io::Error> {
        let n = self.header.write_to_file(output)?;
        let n = n + write_u32(output, self.prefix)?;
        let n = n + write_u32(output, self.uri)?;
        Ok(n)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct XmlAttrExt {
    pub(crate) ns: u32,
    pub(crate) name: u32,
    pub(crate) attribute_start: u16,
    pub(crate) attribute_size: u16,
    pub(crate) attribute_count: u16,
    pub(crate) id_index: u16,
    pub(crate) class_index: u16,
    pub(crate) style_index: u16,
}

impl XmlAttrExt {
    fn read_from_file<F: Read + Seek>(input: &mut F) -> Result<Self, ParseError> {
        let ns = read_u32(input)?;
        let name = read_u32(input)?;

        let attribute_start = read_u16(input)?;
        let attribute_size = read_u16(input)?;
        let attribute_count = read_u16(input)?;
        let id_index = read_u16(input)?;
        let class_index = read_u16(input)?;
        let style_index = read_u16(input)?;

        let header = Self {
            ns,
            name,
            attribute_start,
            attribute_size,
            attribute_count,
            id_index,
            class_index,
            style_index,
        };

        Ok(header)
    }

    pub(crate) fn write_to_file<F: Write + Seek>(self:&Self, output: &mut F) -> Result<usize, std::io::Error> {
        let n = write_u32(output, self.ns)?;
        let n = n + write_u32(output, self.name)?;
        let n = n + write_u16(output, self.attribute_start)?;
        let n = n + write_u16(output, self.attribute_size)?;
        let n = n + write_u16(output, self.attribute_count)?;
        let n = n + write_u16(output, self.id_index)?;
        let n = n + write_u16(output, self.class_index)?;
        let n = n + write_u16(output, self.style_index)?;
        Ok(n)
    }

}

#[derive(Debug)]
pub(crate) struct XmlAttribute {
    pub(crate) ns: u32,
    pub(crate) name: u32,
    pub(crate) raw_value: u32,
    pub(crate) typed_value: ResourceValue,
}

impl XmlAttribute {
    fn read_from_file<F: Read + Seek>(input: &mut F) -> Result<Self, ParseError> {
        let ns = read_u32(input)?;
        let name = read_u32(input)?;
        let raw_value = read_u32(input)?; // raw_value stored in the chunk. There does not seem to be any value in keeping it around since `typed_value` is available...
        let typed_value = ResourceValue::read_from_file(input)?;

        let attr = Self {
            ns,
            name,
            raw_value,
            typed_value,
        };

        Ok(attr)
    }

    pub(crate) fn write_to_file<F: Write + Seek>(self:&Self, output: &mut F) -> Result<usize, std::io::Error> {
        let n = write_u32(output, self.ns)?;
        let n = n + write_u32(output, self.name)?;
        let n = n + write_u32(output, self.raw_value)?;
        let n = n + self.typed_value.write_to_file(output)?;
        Ok(n)
    }
}

#[derive(Debug)]
pub(crate) struct XmlStartElement {
    pub(crate) header: XmlNodeHeader,
    pub(crate) attr_ext: XmlAttrExt,
    pub(crate) attributes: Vec<XmlAttribute>,
}

impl XmlStartElement {
    pub(crate) fn read_from_file<F: Read + Seek>(
        input: &mut F,
        chunk_header: &ChunkHeader,
    ) -> Result<Self, ParseError> {
        let header = XmlNodeHeader::read_from_file(input, &chunk_header)?;
        let attr_ext = XmlAttrExt::read_from_file(input)?;

        let mut attributes = Vec::with_capacity(attr_ext.attribute_count.into());
        for _ in 0..attr_ext.attribute_count {
            attributes.push(XmlAttribute::read_from_file(input)?);
        }

        let node = Self {
            header,
            attr_ext,
            attributes,
        };

        Ok(node)
    }

    pub(crate) fn write_to_file<F: Write + Seek>(self:&Self, output: &mut F) -> Result<usize, std::io::Error> {
        
        let mut h = self.header.clone();
        h.chunk_header.size = (std::mem::size_of::<XmlAttrExt>() + std::mem::size_of::<XmlNodeHeader>() + std::mem::size_of::<XmlAttribute>() * self.attributes.len()) as u32;
        let mut attrext = self.attr_ext.clone();
        attrext.attribute_start = std::mem::size_of::<XmlAttrExt>() as u16;
        attrext.attribute_size = std::mem::size_of::<XmlAttribute>() as u16;
        attrext.attribute_count = self.attributes.len() as u16;
        let n = h.write_to_file(output)?;
        let mut n = n + attrext.write_to_file(output)?;
        for attr in &self.attributes {
            n += attr.write_to_file(output)?;
        }
        Ok(n)
    }

}

#[derive(Debug)]
pub(crate) struct XmlEndElement {
    pub(crate) header: XmlNodeHeader,
    pub(crate) ns: u32,
    pub(crate) name: u32,
}

impl XmlEndElement {
    pub(crate) fn read_from_file<F: Read + Seek>(
        input: &mut F,
        chunk_header: &ChunkHeader,
    ) -> Result<Self, ParseError> {
        let header = XmlNodeHeader::read_from_file(input, &chunk_header)?;
        let ns = read_u32(input)?;
        let name = read_u32(input)?;

        let node = Self { header, ns, name };

        Ok(node)
    }

    pub(crate) fn write_to_file<F: Write + Seek>(self:&Self, output: &mut F) -> Result<usize, std::io::Error> {
        let n = self.header.write_to_file(output)?;
        let n = n + write_u32(output, self.ns)?;
        let n = n + write_u32(output, self.name)?;
        Ok(n)
    }
}

#[derive(Debug)]
pub(crate) struct XmlCdata {
    pub(crate) header: XmlNodeHeader,
    pub(crate) data: u32,
    pub(crate) typed_data: ResourceValue,
}

impl XmlCdata {
    pub(crate) fn read_from_file<F: Read + Seek>(
        input: &mut F,
        chunk_header: &ChunkHeader,
    ) -> Result<Self, ParseError> {
        let header = XmlNodeHeader::read_from_file(input, &chunk_header)?;
        let data = read_u32(input)?;
        let typed_data = ResourceValue::read_from_file(input)?;

        let node = Self {
            header,
            data,
            typed_data,
        };

        Ok(node)
    }

    pub(crate) fn write_to_file<F: Write + Seek>(self:&Self, output: &mut F) -> Result<usize, std::io::Error> {
        let n = self.header.write_to_file(output)?;
        let n = n + write_u32(output, self.data)?;
        let n = n + self.typed_data.write_to_file(output)?;
        Ok(n)
    }
}

impl XmlElement {
    pub(crate) fn write_to_file<F: Write + Seek>(self:&Self, output: &mut F) -> Result<usize, std::io::Error> {
        match self {
            XmlElement::XmlStartNameSpace(d) => d.write_to_file(output),
            XmlElement::XmlEndNameSpace(d) => d.write_to_file(output),
            XmlElement::XmlStartElement(d) => d.write_to_file(output),
            XmlElement::XmlEndElement(d) => d.write_to_file(output),
            XmlElement::XmlCdata(d) => d.write_to_file(output),
        }
    }
}