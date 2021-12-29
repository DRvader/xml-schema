use crate::xsd::{element::Element, Implementation, XsdContext};
use log::{debug, info};
use proc_macro2::TokenStream;
use std::io::prelude::*;
use yaserde::YaDeserialize;

use super::group::Group;

#[derive(Clone, Default, Debug, PartialEq, YaDeserialize)]
#[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct Sequence {
  #[yaserde(rename = "element")]
  pub elements: Vec<Element>,
  #[yaserde(rename = "group")]
  pub groups: Vec<Group>,
}

impl Implementation for Sequence {
  fn implement(
    &self,
    _namespace_definition: &TokenStream,
    prefix: &Option<String>,
    context: &XsdContext,
  ) -> TokenStream {
    info!("Generate elements");
    self
      .elements
      .iter()
      .map(|element| element.get_field_implementation(context, prefix, false))
      .chain(
        self
          .groups
          .iter()
          .map(|element| element.get_field_implementation(context, prefix, false)),
      )
      .collect()
  }
}

impl Sequence {
  pub fn get_sub_types_implementation(
    &self,
    context: &XsdContext,
    namespace_definition: &TokenStream,
    prefix: &Option<String>,
  ) -> TokenStream {
    info!("Generate sub types implementation");
    self
      .elements
      .iter()
      .map(|element| element.get_subtypes_implementation(namespace_definition, prefix, context))
      .collect()
  }

  pub fn get_field_implementation(
    &self,
    context: &XsdContext,
    prefix: &Option<String>,
  ) -> TokenStream {
    self
      .elements
      .iter()
      .map(|element| element.get_field_implementation(context, prefix, true))
      .chain(
        self
          .groups
          .iter()
          .map(|element| element.get_field_implementation(context, prefix, false)),
      )
      .collect()
  }
}
