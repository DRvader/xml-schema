use std::collections::BTreeMap;

use crate::Xsd;

use super::{xsd_context::XsdContext, XMLElementWrapper, XsdError};

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

  pub fn get_implementation(&self, context: &mut XsdContext) -> Result<(), XsdError> {
    let mut xsd = Xsd::new_from_file(self.schema_location.as_ref().unwrap(), &BTreeMap::new())?;
    let top_level_names = xsd.schema.fill_context(
      &mut xsd.context,
      self.namespace.as_ref().map(|v| v.as_str()),
    )?;

    for name in top_level_names {
      let gen = xsd.context.structs.remove(&name).unwrap();
      context.structs.insert(name, gen);
    }

    Ok(())
  }
}
