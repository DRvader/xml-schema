use crate::xsd::{extension::Extension, xsd_context::XsdContext};
use log::debug;
use std::io::Read;
use yaserde::YaDeserialize;

use super::xsd_context::{XsdImpl, XsdName};

#[derive(Clone, Default, Debug, PartialEq, YaDeserialize)]
#[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct ComplexContent {
  pub extension: Option<Extension>,
  pub restriction: Option<Extension>,
}

impl ComplexContent {
  pub fn get_implementation(&self, parent_name: XsdName, context: &mut XsdContext) -> XsdImpl {
    match (&self.extension, &self.restriction) {
      (None, Some(restriction)) => restriction.get_implementation(parent_name, context),
      (Some(extension), None) => extension.get_implementation(parent_name, context),
      _ => {
        unimplemented!("The source xsd is invalid.")
      }
    }
  }
}
