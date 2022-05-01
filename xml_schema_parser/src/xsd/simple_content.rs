use xsd_codegen::XMLElement;
use xsd_types::{XsdIoError, XsdName, XsdParseError, XsdType};

use crate::xsd::{extension::Extension, XsdContext};

use super::{
  restriction::{Restriction, RestrictionParentType},
  xsd_context::XsdImpl,
  XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
pub struct SimpleContent {
  pub restriction: Option<Restriction>,
  pub extension: Option<Extension>,
}

impl SimpleContent {
  pub fn parse(mut element: XMLElement) -> Result<Self, XsdIoError> {
    element.check_name("simpleContent")?;

    let restriction = element.try_get_child_with("restriction", |child| {
      Restriction::parse(RestrictionParentType::SimpleContent, child)
    })?;
    let extension = element.try_get_child_with("extension", Extension::parse)?;

    if restriction.is_some() && extension.is_some() {
      return Err(
        XsdParseError {
          node_name: element.node_name(),
          msg: "extension and restriction cannot both present".to_string(),
        }
        .into(),
      );
    }

    let output = Self {
      restriction,
      extension,
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
    let mut gen = match (&self.restriction, &self.extension) {
      (None, Some(extension)) => extension.get_implementation(parent_name, context),
      (Some(restriction), None) => {
        restriction.get_implementation(parent_name, RestrictionParentType::SimpleContent, context)
      }
      _ => unreachable!("Xsd is invalid!"),
    }?;

    gen.name.ty = XsdType::SimpleContent;

    Ok(gen)
  }
}
