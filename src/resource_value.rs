use num_enum::{TryFromPrimitive, IntoPrimitive};
use std::convert::TryFrom;
use std::io::{Read, Seek, Write};
use std::rc::Rc;

use crate::stringpool::StringPool;
use crate::{read_u16, read_u32, read_u8, ParseError, write_u16, write_u32, write_u8};

#[derive(Debug)]
pub(crate) struct ResourceValue {
    pub(crate) size: u16,
    pub(crate) res: u8,
    pub(crate) data_type: ResourceValueType,
    pub(crate) data: u32,
}

impl ResourceValue {
    pub(crate) fn read_from_file<F: Read + Seek>(input: &mut F) -> Result<Self, ParseError> {
        let size = read_u16(input)?;
        let res = read_u8(input)?;
        let data_type = ResourceValueType::try_from(read_u8(input)?).unwrap();
        let data = read_u32(input)?;

        Ok(Self {
            size,
            res,
            data_type,
            data,
        })
    }

    pub(crate) fn get_value(&self, string_pool: &StringPool) -> Rc<String> {
        match &self.data_type {
            ResourceValueType::String => string_pool
                .get(usize::try_from(self.data).unwrap())
                .unwrap(),
            ResourceValueType::Dec => Rc::new(self.data.to_string()),
            ResourceValueType::Hex => Rc::new(format!("0x{}", self.data)),
            ResourceValueType::Boolean => Rc::new(match self.data {
                0 => "false".to_string(),
                _ => "true".to_string(),
            }),
            n => Rc::new(format!("ResourceValueType::{:?}/{}", n, self.data)),
        }
    }

    pub(crate) fn write_to_file<F: Write + Seek>(self:&Self, output: &mut F) -> Result<usize, std::io::Error> {
        let n = write_u16(output, self.size)?;
        let n = n + write_u8(output, self.res)?;
        let n = n + write_u8(output, self.data_type as u8)?;
        let n = n + write_u32(output, self.data)?;
        Ok(n)
    }
}

#[repr(u8)]
#[derive(Debug, PartialEq, TryFromPrimitive, Copy, Clone)]
pub(crate) enum ResourceValueType {
    Null = 0x00,
    Reference = 0x01,
    Attribute = 0x02,
    String = 0x03,
    Float = 0x04,
    Dimension = 0x05,
    Fraction = 0x06,
    Dec = 0x10,
    Hex = 0x11,
    Boolean = 0x12,
    ColorArgb8 = 0x1c,
    ColorRgb8 = 0x1d,
    ColorArgb4 = 0x1e,
    ColorRgb4 = 0x1f,
}
