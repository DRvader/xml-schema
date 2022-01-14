use crate::xsd::{extension::Extension, xsd_context::XsdContext};

use super::{
  restriction::{Restriction, RestrictionParentType},
  xsd_context::{XsdImpl, XsdName},
  XMLElementWrapper, XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct ComplexContent {
  pub extension: Option<Extension>,
  pub restriction: Option<Restriction>,
}

impl ComplexContent {
  pub fn parse(mut element: XMLElementWrapper) -> Result<Self, XsdError> {
    element.check_name("complexContent")?;

    let output = Self {
      extension: element.try_get_child_with("extension", Extension::parse)?,
      restriction: element.try_get_child_with("restriction", |child| {
        Restriction::parse(RestrictionParentType::ComplexContent, child)
      })?,
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  pub fn get_implementation(
    &self,
    parent_name: XsdName,
    context: &mut XsdContext,
  ) -> Result<XsdImpl, XsdError> {
    log::debug!("Entered Complex Content: {:?}", &parent_name);

    let generated_struct = match (&self.extension, &self.restriction) {
      (None, Some(restriction)) => restriction.get_implementation(
        parent_name.clone(),
        RestrictionParentType::ComplexContent,
        context,
      ),
      (Some(extension), None) => extension.get_implementation(parent_name.clone(), context),
      _ => {
        unimplemented!("The source xsd is invalid.")
      }
    };

    log::debug!("Exited Complex Content: {:?}", &parent_name);

    generated_struct
  }
}
