use xsd_codegen::XMLElement;

use super::XsdError;

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Enumeration {
  pub value: String,
}

impl Enumeration {
  pub fn parse(mut element: XMLElement) -> Result<Self, XsdError> {
    element.check_name("enumeration")?;

    let output = Self {
      value: element.get_attribute("value")?,
    };

    element.finalize(false, false)?;

    Ok(output)
  }
}
