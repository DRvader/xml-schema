use crate::{
  codegen::Struct,
  xsd::{
    annotation::Annotation,
    attribute::Attribute,
    complex_content::ComplexContent,
    sequence::Sequence,
    simple_content::SimpleContent,
    xsd_context::{MergeSettings, XsdElement, XsdImpl, XsdName},
    XsdContext,
  },
};
use heck::CamelCase;
use log::debug;
use std::io::prelude::*;
use yaserde::YaDeserialize;

use super::group::Group;

#[derive(Clone, Default, Debug, PartialEq, YaDeserialize)]
#[yaserde(
  rename = "complexType"
  prefix = "xs",
  namespace = "xs: http://www.w3.org/2001/XMLSchema"
)]
pub struct ComplexType {
  #[yaserde(attribute)]
  pub name: String,
  #[yaserde(rename = "attribute")]
  pub attributes: Vec<Attribute>,
  pub group: Option<Group>,
  pub sequence: Option<Sequence>,
  #[yaserde(rename = "simpleContent")]
  pub simple_content: Option<SimpleContent>,
  #[yaserde(rename = "complexContent")]
  pub complex_content: Option<ComplexContent>,
  #[yaserde(rename = "annotation")]
  pub annotation: Option<Annotation>,
}

impl ComplexType {
  pub fn get_implementation(&self, context: &mut XsdContext) -> XsdImpl {
    let struct_id = XsdName {
      namespace: None,
      local_name: self.name.clone(),
    };

    assert!(
      !context.structs.contains_key(&struct_id),
      "Struct {:?} has already been declared.",
      &struct_id
    );

    let struct_name = self.name.replace(".", "_").to_camel_case();

    let fields = match (
      &self.complex_content,
      &self.simple_content,
      &self.group,
      &self.sequence,
    ) {
      (Some(complex_content), None, None, None) => complex_content.get_implementation(
        XsdName {
          namespace: None,
          local_name: self.name.clone(),
        },
        context,
      ),
      (None, Some(simple_content), None, None) => simple_content.get_implementation(
        XsdName {
          namespace: None,
          local_name: self.name.clone(),
        },
        context,
      ),
      (None, None, Some(group), None) => group.get_implementation(
        Some(XsdName {
          namespace: None,
          local_name: self.name.clone(),
        }),
        context,
      ),
      (None, None, None, Some(sequence)) => sequence.get_implementation(
        XsdName {
          namespace: None,
          local_name: self.name.clone(),
        },
        context,
      ),
      _ => unreachable!("Xsd is invalid."),
    };

    let docs = self
      .annotation
      .as_ref()
      .map(|annotation| annotation.get_doc())
      .unwrap_or_default();

    let mut generated_impl = XsdImpl {
      element: XsdElement::Struct(
        Struct::new(&struct_name)
          .doc(&docs.join("\n"))
          .derive("#[derive(Clone, Debug, Default, PartialEq, YaDeserialize, YaSerialize)]")
          .to_owned(),
      ),
      ..Default::default()
    };

    generated_impl.merge(fields, MergeSettings::default());
    for attribute in &self.attributes {
      if let Some(generated) = attribute.get_implementation(context) {
        generated_impl.merge(
          generated,
          MergeSettings {
            conflict_prefix: Some("attr_"),
          },
        );
      }
    }

    generated_impl
  }
}
