use crate::xsd::{
  attribute, attribute_group, complex_type, element, group, import, qualification, simple_type,
  XsdContext,
};
use log::debug;
use proc_macro2::TokenStream;
use std::io::prelude::*;
use yaserde::YaDeserialize;

#[derive(Clone, Default, Debug, PartialEq, YaDeserialize)]
#[yaserde(
  root="schema"
  prefix="xs",
  namespace="xs: http://www.w3.org/2001/XMLSchema",
)]
pub struct Schema {
  #[yaserde(rename = "targetNamespace", attribute)]
  pub target_namespace: Option<String>,
  #[yaserde(rename = "elementFormDefault", attribute)]
  pub element_form_default: qualification::Qualification,
  #[yaserde(rename = "attributeFormDefault", attribute)]
  pub attribute_form_default: qualification::Qualification,
  #[yaserde(rename = "import")]
  pub imports: Vec<import::Import>,
  #[yaserde(rename = "element")]
  pub elements: Vec<element::Element>,
  #[yaserde(rename = "simpleType")]
  pub simple_type: Vec<simple_type::SimpleType>,
  #[yaserde(rename = "complexType")]
  pub complex_type: Vec<complex_type::ComplexType>,
  #[yaserde(rename = "attribute")]
  pub attributes: Vec<attribute::Attribute>,
  #[yaserde(rename = "attributeGroup")]
  pub attribute_group: Vec<attribute_group::AttributeGroup>,
  #[yaserde(rename = "group")]
  pub groups: Vec<group::Group>,
}

impl Schema {
  pub fn generate(&self, context: &mut XsdContext) -> String {
    // let namespace_definition = generate_namespace_definition(target_prefix, &self.target_namespace);

    let mut top_level_names = vec![];

    dbg!("Generating GROUPS");
    for group in &self.groups {
      let temp = group.get_implementation(None, context);
      top_level_names.push(temp.name.clone().unwrap());
      context.structs.insert(temp.name.clone().unwrap(), temp);
    }

    dbg!("Generating ELEMENTS");
    for element in &self.elements {
      let temp = element.get_implementation(context);
      top_level_names.push(temp.name.clone().unwrap());
      context.structs.insert(temp.name.clone().unwrap(), temp);
    }

    dbg!("Generating SIMPLE TYPE");
    for simple_type in &self.simple_type {
      let temp = simple_type.get_implementation(context);
      top_level_names.push(temp.name.clone().unwrap());
      context.structs.insert(temp.name.clone().unwrap(), temp);
    }

    dbg!("Generating COMPLEX TYPE");
    for complex_type in &self.complex_type {
      let temp = complex_type.get_implementation(context);
      top_level_names.push(temp.name.clone().unwrap());
      context.structs.insert(temp.name.clone().unwrap(), temp);
    }

    let mut dst = String::new();
    let mut formatter = crate::codegen::Formatter::new(&mut dst);
    for name in top_level_names {
      context
        .structs
        .get(&name)
        .unwrap()
        .fmt(&mut formatter)
        .unwrap();
    }

    dst
  }
}

fn generate_namespace_definition(
  target_prefix: &Option<String>,
  target_namespace: &Option<String>,
) -> TokenStream {
  match (target_prefix, target_namespace) {
    (None, None) => quote!(),
    (None, Some(_target_namespace)) => {
      panic!("undefined prefix attribute, a target namespace is defined")
    }
    (Some(_prefix), None) => panic!(
      "a prefix attribute, but no target namespace is defined, please remove the prefix parameter"
    ),
    (Some(prefix), Some(target_namespace)) => {
      let namespace = format!("{}: {}", prefix, target_namespace);
      quote!(#[yaserde(prefix=#prefix, namespace=#namespace)])
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_schema_implementation() {
    let schema = Schema::default();

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    let implementation = format!("{}", schema.generate(&mut context));
    assert_eq!(implementation, "");
  }

  #[test]
  #[should_panic]
  fn missing_prefix() {
    let mut schema = Schema::default();
    schema.target_namespace = Some("http://example.com".to_string());

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    schema.generate(&mut context);
  }

  #[test]
  #[should_panic]
  fn missing_target_namespace() {
    let schema = Schema::default();

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    schema.generate(&mut context);
  }

  #[test]
  fn generate_namespace() {
    let definition = generate_namespace_definition(
      &Some("prefix".to_string()),
      &Some("http://example.com".to_string()),
    );

    let implementation = format!("{}", definition);

    assert_eq!(
      implementation,
      r#"# [ yaserde ( prefix = "prefix" , namespace = "prefix: http://example.com" ) ]"#
    );
  }
}
