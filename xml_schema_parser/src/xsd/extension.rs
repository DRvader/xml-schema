use crate::xsd::{attribute::Attribute, sequence::Sequence, XsdContext};
use log::debug;
use std::io::prelude::*;
use yaserde::YaDeserialize;

use super::{
  choice::Choice,
  group::Group,
  xsd_context::{MergeSettings, XsdImpl, XsdName},
};

#[derive(Clone, Default, Debug, PartialEq, YaDeserialize)]
#[yaserde(
  root = "extension",
  prefix = "xs",
  namespace = "xs: http://www.w3.org/2001/XMLSchema"
)]
pub struct Extension {
  #[yaserde(attribute)]
  pub base: String,
  #[yaserde(rename = "attribute")]
  pub attributes: Vec<Attribute>,
  pub sequence: Option<Sequence>,
  pub group: Option<Group>,
  pub choice: Option<Choice>,
}

impl Extension {
  pub fn get_implementation(&self, parent_name: XsdName, context: &mut XsdContext) -> XsdImpl {
    let mut generated_impl = match (&self.group, &self.sequence, &self.choice) {
      (None, None, Some(choice)) => choice.get_implementation(parent_name, context),
      (None, Some(sequence), None) => sequence.get_implementation(parent_name, context),
      (Some(group), None, None) => group.get_implementation(Some(parent_name), context),
      _ => unreachable!("Invalid Xsd!"),
    };

    for attribute in self
      .attributes
      .iter()
      .filter_map(|attribute| attribute.get_implementation(context))
    {
      generated_impl.merge(attribute, MergeSettings::ATTRIBUTE);
    }

    generated_impl
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extension() {
    let st = Extension {
      base: "xs:string".to_string(),
      ..Default::default()
    };

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    let value = st
      .get_implementation(XsdName::new("test"), &mut context)
      .to_string()
      .unwrap();
    let ts = quote!(#value).to_string();
    assert!(ts == "# [ yaserde ( text ) ] pub content : String ,");
  }

  #[test]
  fn extension_with_attributes() {
    use crate::xsd::attribute::Required;

    let st = Extension {
      base: "xs:string".to_string(),
      attributes: vec![
        Attribute {
          name: Some("attribute_1".to_string()),
          kind: Some("xs:string".to_string()),
          reference: None,
          required: Required::Required,
          simple_type: None,
        },
        Attribute {
          name: Some("attribute_2".to_string()),
          kind: Some("xs:boolean".to_string()),
          reference: None,
          required: Required::Optional,
          simple_type: None,
        },
      ],
      ..Default::default()
    };

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    let value = st
      .get_implementation(XsdName::new("test"), &mut context)
      .to_string()
      .unwrap();
    let ts = quote!(#value).to_string();
    assert!(ts == "struct Test { # [ yaserde ( text ) ] pub content : String , # [ yaserde ( attribute ) ] pub attribute_1 : String , # [ yaserde ( attribute ) ] pub attribute_2 : Option < bool > , }");
  }
}
