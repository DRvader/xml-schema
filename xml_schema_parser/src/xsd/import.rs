use super::{XMLElementWrapper, XsdError};

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Import {
  pub id: Option<String>,
  pub namespace: Option<String>,
  pub schema_location: Option<String>,
}

impl Import {
  pub fn parse(mut element: XMLElementWrapper) -> Result<Self, XsdError> {
    Ok(Self {
      id: element.try_get_attribute("id")?,
      namespace: element.try_get_attribute("namespace")?,
      schema_location: element.try_get_attribute("schemaLocation")?,
    })
  }
}
