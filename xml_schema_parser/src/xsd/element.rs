use crate::{
  codegen::{Field, Struct},
  xsd::{
    annotation::Annotation,
    complex_type::ComplexType,
    max_occurences::MaxOccurences,
    simple_type::SimpleType,
    xsd_context::{XsdElement, XsdImpl, XsdName},
    XsdContext,
  },
};
use heck::{CamelCase, SnakeCase};
use log::{debug, info};
use proc_macro2::Span;
use std::io::prelude::*;
use syn::Ident;
use yaserde::YaDeserialize;

#[derive(Clone, Default, Debug, PartialEq, YaDeserialize)]
#[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct Element {
  #[yaserde(attribute)]
  pub name: String,
  #[yaserde(rename = "type", attribute)]
  pub kind: Option<String>,
  #[yaserde(rename = "ref", attribute)]
  pub refers: Option<String>,
  #[yaserde(rename = "minOccurs", attribute)]
  pub min_occurences: Option<u64>,
  #[yaserde(rename = "maxOccurs", attribute)]
  pub max_occurences: Option<MaxOccurences>,
  #[yaserde(rename = "complexType")]
  pub complex_type: Option<ComplexType>,
  #[yaserde(rename = "simpleType")]
  pub simple_type: Option<SimpleType>,
  #[yaserde(rename = "annotation")]
  pub annotation: Option<Annotation>,
  #[yaserde(rename = "unique")]
  pub uniques: Vec<String>,
  #[yaserde(rename = "key")]
  pub keys: Vec<String>,
  #[yaserde(rename = "keyref")]
  pub keyrefs: Vec<String>,
}

impl Element {
  fn is_multiple(&self) -> bool {
    self
      .max_occurences
      .as_ref()
      .map(|v| match v {
        MaxOccurences::Unbounded => true,
        MaxOccurences::Number { value } => *value > 0,
      })
      .unwrap_or(false)
      || self.min_occurences.unwrap_or(1) > 0
  }

  fn could_be_none(&self) -> bool {
    self
      .max_occurences
      .as_ref()
      .map(|v| match v {
        MaxOccurences::Unbounded => false,
        MaxOccurences::Number { value } => *value == 1,
      })
      .unwrap_or(true)
      && self.min_occurences.unwrap_or(1) == 0
  }

  pub fn get_implementation(&self, context: &mut XsdContext) -> XsdImpl {
    assert!(self.uniques.is_empty(), "Unique content is not supported.");
    assert!(self.keys.is_empty(), "Key content is not supported.");
    assert!(self.keyrefs.is_empty(), "Keyref content is not supported.");

    let type_name = self.name.replace(".", "_").to_camel_case();

    let generated_impl = if self.is_multiple() || self.could_be_none() {
      let mut generated_field = self.get_field(context);
      let docs = generated_field.documentation.join("\n");
      generated_field.documentation = vec![];

      let generated_struct = Struct::new(&type_name)
        .push_field(generated_field)
        .doc(&docs)
        .to_owned();

      XsdImpl {
        element: XsdElement::Struct(generated_struct),
        ..Default::default()
      }
    } else {
      let docs = self
        .annotation
        .as_ref()
        .map(|annotation| annotation.get_doc());

      let generated_impl = match (&self.simple_type, &self.complex_type) {
        (None, Some(complex_type)) => complex_type.get_implementation(context),
        (Some(simple_type), None) => simple_type.get_implementation(context),
        _ => unreachable!("Invalid Xsd."),
      };

      generated_impl
    };

    generated_impl
  }

  pub fn get_field(&self, context: &mut XsdContext) -> Field {
    assert!(self.uniques.is_empty(), "Unique content is not supported.");
    assert!(self.keys.is_empty(), "Key content is not supported.");
    assert!(self.keyrefs.is_empty(), "Keyref content is not supported.");

    let mut field_type = match (&self.simple_type, &self.complex_type) {
      (None, Some(complex_type)) => complex_type.get_implementation(context).element.get_type(),
      (Some(simple_type), None) => simple_type.get_implementation(context).element.get_type(),
      _ => unreachable!("Invalid Xsd."),
    };

    let field_name = XsdName::new(&self.name).to_field_name();

    let multiple = self.is_multiple();

    let field_name = if multiple {
      format!("{}s", field_name)
    } else {
      field_name
    };

    let yaserde_rename = &self.name;

    if multiple {
      field_type.wrap("Vec");
    }

    if self.could_be_none() {
      field_type.wrap("Option");
    }

    let mut generated_field = Field::new(&field_name, field_type)
      .vis("pub")
      .annotation(vec![&format!("yaserde(rename={})", yaserde_rename)])
      .to_owned();

    let docs = self
      .annotation
      .as_ref()
      .map(|annotation| annotation.get_doc());

    if let Some(docs) = docs {
      generated_field.doc(docs.iter().map(|f| f.as_str()).collect());
    }

    generated_field
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  static DERIVES: &str =
    "# [ derive ( Clone , Debug , Default , PartialEq , YaDeserialize , YaSerialize ) ] ";

  static DOCS: &str = r#"# [ doc = "Loudness measured in Decibels" ] "#;

  #[test]
  fn extern_type() {
    let element = Element {
      name: "volume".to_string(),
      kind: Some("books:volume-type".to_string()),
      refers: None,
      min_occurences: None,
      max_occurences: None,
      complex_type: None,
      simple_type: None,
      annotation: Some(Annotation {
        id: None,
        attributes: vec![],
        documentation: vec!["Loudness measured in Decibels".to_string()],
      }),
      ..Default::default()
    };

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    let value = element
      .get_implementation(&mut context)
      .to_string()
      .unwrap();
    let ts = quote!(#value).to_string();

    assert_eq!(
      ts.to_string(),
      format!(
        "{}{}pub struct Volume {{ # [ yaserde ( flatten ) ] pub content : VolumeType , }}",
        DOCS, DERIVES
      )
    );
  }

  #[test]
  fn xs_string_element() {
    let element = Element {
      name: "volume".to_string(),
      kind: Some("xs:string".to_string()),
      refers: None,
      min_occurences: None,
      max_occurences: None,
      complex_type: None,
      simple_type: None,
      annotation: Some(Annotation {
        id: None,
        attributes: vec![],
        documentation: vec!["Loudness measured in Decibels".to_string()],
      }),
      ..Default::default()
    };

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    let value = element
      .get_implementation(&mut context)
      .to_string()
      .unwrap();
    let ts = quote!(#value).to_string();

    assert_eq!(
      ts.to_string(),
      format!(
        "{}{}pub struct Volume {{ # [ yaserde ( text ) ] pub content : String , }}",
        DOCS, DERIVES
      )
    );
  }
}
