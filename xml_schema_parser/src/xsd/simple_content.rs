use crate::xsd::{extension::Extension, XsdContext};
use log::debug;
use std::io::prelude::*;
use yaserde::YaDeserialize;

use super::{
  restriction::{Restriction, RestrictionParentType},
  xsd_context::{XsdImpl, XsdName},
};

#[derive(Clone, Default, Debug, PartialEq, YaDeserialize)]
#[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct SimpleContent {
  #[yaserde(prefix = "xs")]
  pub restriction: Option<Restriction>,
  #[yaserde(prefix = "xs")]
  pub extension: Option<Extension>,
}

impl SimpleContent {
  pub fn get_implementation(&self, parent_name: XsdName, context: &mut XsdContext) -> XsdImpl {
    match (&self.restriction, &self.extension) {
      (None, Some(extension)) => extension.get_implementation(parent_name, context),
      (Some(restriction), None) => {
        restriction.get_implementation(parent_name, RestrictionParentType::SimpleContent, context)
      }
      _ => unreachable!("Xsd is invalid!"),
    }
  }
}
