use crate::xsd::{
  attribute, attribute_group, complex_type, element, group, import, qualification, simple_type,
  XsdContext,
};
use proc_macro2::TokenStream;

use super::{annotation, XMLElementWrapper, XsdError};

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(
//   root="schema"
//   prefix="xs",
//   namespace="xs: http://www.w3.org/2001/XMLSchema",
// )]
pub struct Schema {
  pub target_namespace: Option<String>,
  pub element_form_default: qualification::Qualification,
  pub attribute_form_default: qualification::Qualification,
  pub imports: Vec<import::Import>,
  pub annotations: Vec<annotation::Annotation>,
  pub elements: Vec<element::Element>,
  pub simple_type: Vec<simple_type::SimpleType>,
  pub complex_type: Vec<complex_type::ComplexType>,
  pub attributes: Vec<attribute::Attribute>,
  pub attribute_group: Vec<attribute_group::AttributeGroup>,
  pub groups: Vec<group::Group>,
}

impl Schema {
  pub fn parse(mut element: XMLElementWrapper) -> Result<Self, XsdError> {
    element.check_name("schema")?;

    let annotations =
      element.get_children_with("annotation", |child| annotation::Annotation::parse(child))?;
    let imports = element.get_children_with("import", |child| import::Import::parse(child))?;
    let elements = element.get_children_with("element", |child| element::Element::parse(child))?;
    let simple_type = element.get_children_with("simpleType", |child| {
      simple_type::SimpleType::parse(child, true)
    })?;
    let complex_type = element.get_children_with("complexType", |child| {
      complex_type::ComplexType::parse(child)
    })?;
    let attributes =
      element.get_children_with("attribute", |child| attribute::Attribute::parse(child))?;
    let attribute_group = element.get_children_with("attributeGroup", |child| {
      attribute_group::AttributeGroup::parse(child)
    })?;
    let groups = element.get_children_with("group", |child| group::Group::parse(child))?;

    let output = Self {
      target_namespace: element.try_get_attribute("targetNamespace")?,
      element_form_default: element.get_attribute_default("elementFormDefault")?,
      attribute_form_default: element.get_attribute_default("attributeFormDefault")?,
      annotations,
      imports,
      elements,
      simple_type,
      complex_type,
      attributes,
      attribute_group,
      groups,
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  pub fn generate(&self, context: &mut XsdContext) -> String {
    // let namespace_definition = generate_namespace_definition(target_prefix, &self.target_namespace);

    let mut top_level_names = vec![];

    dbg!("Generating ATTR GROUPS");
    for attr_group in &self.attribute_group {
      let temp = attr_group.get_implementation(None, context);
      top_level_names.push(temp.name.clone().unwrap());
      context.structs.insert(temp.name.clone().unwrap(), temp);
    }

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
