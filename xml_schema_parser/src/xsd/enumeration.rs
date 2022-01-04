use super::{XMLElementWrapper, XsdError};

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct Enumeration {
  pub value: String,
}

impl Enumeration {
  pub fn parse(mut element: XMLElementWrapper) -> Result<Self, XsdError> {
    element.check_name("enumeration")?;

    let output = Self {
      value: element.get_attribute("value")?,
    };

    element.finalize(false, false)?;

    Ok(output)
  }
}
