use crate::xsd::{
  attribute, attribute_group, complex_type, element, group, import, qualification, simple_type,
  xsd_context::XsdName, XsdContext,
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

    let annotations = element.get_children_with("annotation", annotation::Annotation::parse)?;
    let imports = element.get_children_with("import", import::Import::parse)?;
    let elements =
      element.get_children_with("element", |child| element::Element::parse(child, true))?;
    let simple_type = element.get_children_with("simpleType", |child| {
      simple_type::SimpleType::parse(child, true)
    })?;
    let complex_type = element.get_children_with("complexType", |child| {
      complex_type::ComplexType::parse(child)
    })?;
    let attributes = element.get_children_with("attribute", attribute::Attribute::parse)?;
    let attribute_group = element.get_children_with("attributeGroup", |child| {
      attribute_group::AttributeGroup::parse(child)
    })?;
    let groups = element.get_children_with("group", group::Group::parse)?;

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

  pub fn generate(&self, context: &mut XsdContext) -> Result<String, XsdError> {
    // let namespace_definition = generate_namespace_definition(target_prefix, &self.target_namespace);

    let mut top_level_names = vec![];

    let mut simple_type_to_run = (0..self.simple_type.len())
      .into_iter()
      .map(|i| (i, XsdName::new("")))
      .collect::<Vec<_>>();
    let mut attr_group_to_run = (0..self.attribute_group.len())
      .into_iter()
      .map(|i| (i, XsdName::new("")))
      .collect::<Vec<_>>();
    let mut group_to_run = (0..self.groups.len())
      .into_iter()
      .map(|i| (i, XsdName::new("")))
      .collect::<Vec<_>>();
    let mut element_to_run = (0..self.elements.len())
      .into_iter()
      .map(|i| (i, XsdName::new("")))
      .collect::<Vec<_>>();
    let mut complex_type_to_run = (0..self.complex_type.len())
      .into_iter()
      .map(|i| (i, XsdName::new("")))
      .collect::<Vec<_>>();

    let mut new = vec![];

    let mut changed = true;
    while changed {
      changed = false;

      new.clear();
      for index in &simple_type_to_run {
        let simple_type = &self.simple_type[index.0];
        match simple_type.get_implementation(context) {
          Ok(temp) => {
            top_level_names.push(temp.name.clone().unwrap());
            context.structs.insert(temp.name.clone().unwrap(), temp);
          }
          Err(ty) => match ty {
            XsdError::XsdImplNotFound(name) => {
              new.push((index.0, name));
            }
            _ => return Err(ty),
          },
        }
      }
      if simple_type_to_run.len() != new.len() {
        changed = true;
      }
      simple_type_to_run = new.clone();

      new.clear();
      for index in &attr_group_to_run {
        let attr_group = &self.attribute_group[index.0];
        match attr_group.get_implementation(None, context) {
          Ok(temp) => {
            top_level_names.push(temp.name.clone().unwrap());
            context.structs.insert(temp.name.clone().unwrap(), temp);
          }
          Err(ty) => match ty {
            XsdError::XsdImplNotFound(name) => {
              new.push((index.0, name));
            }
            _ => return Err(ty),
          },
        }
      }
      if attr_group_to_run.len() != new.len() {
        changed = true;
      }
      attr_group_to_run = new.clone();

      new.clear();
      for index in &group_to_run {
        let group = &self.groups[index.0];
        match group.get_implementation(None, context) {
          Ok(temp) => {
            top_level_names.push(temp.name.clone().unwrap());
            context.structs.insert(temp.name.clone().unwrap(), temp);
          }
          Err(ty) => match ty {
            XsdError::XsdImplNotFound(name) => {
              new.push((index.0, name));
            }
            _ => return Err(ty),
          },
        }
      }
      if group_to_run.len() != new.len() {
        changed = true;
      }
      group_to_run = new.clone();

      new.clear();
      for index in &element_to_run {
        let element = &self.elements[index.0];
        match element.get_implementation(context) {
          Ok(temp) => {
            top_level_names.push(temp.name.clone().unwrap());
            context.structs.insert(temp.name.clone().unwrap(), temp);
          }
          Err(ty) => match ty {
            XsdError::XsdImplNotFound(name) => {
              new.push((index.0, name));
            }
            _ => return Err(ty),
          },
        }
      }
      if element_to_run.len() != new.len() {
        changed = true;
      }
      element_to_run = new.clone();

      new.clear();
      for index in &complex_type_to_run {
        let complex_type = &self.complex_type[index.0];
        match complex_type.get_implementation(context) {
          Ok(temp) => {
            top_level_names.push(temp.name.clone().unwrap());
            context.structs.insert(temp.name.clone().unwrap(), temp);
          }
          Err(ty) => match ty {
            XsdError::XsdImplNotFound(name) => {
              new.push((index.0, name));
            }
            _ => return Err(ty),
          },
        }
      }
      if complex_type_to_run.len() != new.len() {
        changed = true;
      }
      complex_type_to_run = new.clone()
    }

    let mut error = String::new();
    for (_, v) in simple_type_to_run {
      error.push_str(&format!("\nsimple_type::{}", v));
    }
    for (_, v) in attr_group_to_run {
      error.push_str(&format!("\nattribute_group::{}", v));
    }
    for (_, v) in group_to_run {
      error.push_str(&format!("\ngroup::{}", v));
    }
    for (_, v) in element_to_run {
      error.push_str(&format!("\nelement::{}", v));
    }
    for (_, v) in complex_type_to_run {
      error.push_str(&format!("\ncomplex_type::{}", v));
    }

    if !error.is_empty() {
      return Err(XsdError::XsdParseError(format!("COULD NOT FIND:{}", error)));
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

    std::fs::write("tmp.log", &dst);

    Ok(dst)
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

    let implementation = schema.generate(&mut context).unwrap();
    assert!(implementation.is_empty());
  }

  #[test]
  #[should_panic]
  fn missing_prefix() {
    let schema = Schema {
      target_namespace: Some("http://example.com".to_string()),
      ..Schema::default()
    };

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    schema.generate(&mut context).unwrap();
  }

  #[test]
  #[should_panic]
  fn missing_target_namespace() {
    let schema = Schema::default();

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    schema.generate(&mut context).unwrap();
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
