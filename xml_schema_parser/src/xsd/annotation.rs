use crate::xsd::attribute::Attribute;
use std::io::prelude::*;
use yaserde::YaDeserialize;

#[derive(Clone, Default, Debug, PartialEq, YaDeserialize)]
#[yaserde(
    rename = "annotation"
    prefix = "xs",
    namespace = "xs: http://www.w3.org/2001/XMLSchema"
  )]
pub struct Annotation {
  #[yaserde(attribute)]
  pub id: Option<String>,
  #[yaserde(rename = "attribute")]
  pub attributes: Vec<Attribute>,
  #[yaserde(
      rename = "documentation"
      prefix = "xs",
      namespace = "xs: http://www.w3.org/2001/XMLSchema"
    )]
  pub documentation: Vec<String>,
}

impl Annotation {
  pub fn get_doc(&self) -> Vec<String> {
    return self.documentation.clone();
  }
}
