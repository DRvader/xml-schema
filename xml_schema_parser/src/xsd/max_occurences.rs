use std::{io::prelude::*, str::FromStr};
use xml::reader::XmlEvent;
use xsd_codegen::FromXmlString;
#[derive(Clone, Debug, PartialEq)]
pub enum MaxOccurences {
  Unbounded,
  Number { value: u32 },
}

impl Default for MaxOccurences {
  fn default() -> Self {
    MaxOccurences::Number { value: 1 }
  }
}

impl FromXmlString for MaxOccurences {
  fn from_xml(s: &str) -> Result<Self, String> {
    if s == "unbounded" {
      Ok(MaxOccurences::Unbounded)
    } else {
      let number = s.parse::<u32>().map_err(|e| e.to_string())?;
      Ok(MaxOccurences::Number { value: number })
    }
  }
}
