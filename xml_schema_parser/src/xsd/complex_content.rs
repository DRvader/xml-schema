use xsd_codegen::XMLElement;
use xsd_types::{XsdIoError, XsdName, XsdType};

use crate::xsd::{extension::Extension, xsd_context::XsdContext};

use super::{
  restriction::{Restriction, RestrictionParentType},
  xsd_context::XsdImpl,
  XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
pub struct ComplexContent {
  pub extension: Option<Extension>,
  pub restriction: Option<Restriction>,
}

impl ComplexContent {
  pub fn parse(mut element: XMLElement) -> Result<Self, XsdIoError> {
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

  #[tracing::instrument(skip_all)]
  pub fn get_implementation(
    &self,
    parent_name: XsdName,
    context: &mut XsdContext,
  ) -> Result<XsdImpl, XsdError> {
    let mut gen = match (&self.extension, &self.restriction) {
      (None, Some(restriction)) => {
        restriction.get_implementation(parent_name, RestrictionParentType::ComplexContent, context)
      }
      (Some(extension), None) => extension.get_implementation(parent_name, context),
      _ => {
        unimplemented!("The source xsd is invalid.")
      }
    }?;

    gen.name.ty = XsdType::ComplexContent;

    Ok(gen)
  }
}
