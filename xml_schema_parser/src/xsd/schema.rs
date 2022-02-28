use crate::xsd::{
  attribute, attribute_group, complex_type, element, group, import, qualification, simple_type,
  xsd_context::XsdName, XsdContext,
};
use proc_macro2::TokenStream;

use super::{
  annotation,
  xsd_context::{XsdImpl, XsdType},
  XMLElementWrapper, XsdError,
};

fn parse_type(
  to_run: Vec<(usize, XsdName, i32)>,
  top_level_names: &mut Vec<XsdName>,
  context: &mut XsdContext,
  namespace_filter: Option<&str>,
  mut parse_fn: impl FnMut(&mut XsdContext, usize) -> Result<XsdImpl, XsdError>,
) -> Result<Vec<(usize, XsdName, i32)>, XsdError> {
  let mut new = vec![];
  for index in to_run {
    match parse_fn(context, index.0) {
      Ok(temp) => {
        let mut include_type = false;
        if let Some(filter) = namespace_filter {
          if let Some(namespace) = &temp.name.namespace {
            if namespace == filter {
              include_type = true;
            }
          }
        } else {
          include_type = true;
        }
        if include_type {
          top_level_names.push(temp.name.clone());
        }
        context.structs.insert(temp.name.clone(), temp);
      }
      Err(ty) => match ty {
        XsdError::XsdImplNotFound(name) => {
          new.push((index.0, name, index.2 + 1));
        }
        _ => return Err(ty),
      },
    }
  }

  Ok(new)
}

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
  pub extra: Vec<(String, String)>,
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
      extra: element.get_remaining_attributes(),
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  pub fn fill_context(
    &self,
    context: &mut XsdContext,
    namespace_filter: Option<&str>,
  ) -> Result<Vec<XsdName>, XsdError> {
    // let namespace_definition = generate_namespace_definition(target_prefix, &self.target_namespace);

    for import in &self.imports {
      import.get_implementation(context)?;
    }

    let mut top_level_names = vec![];

    let mut simple_type_to_run = (0..self.simple_type.len())
      .into_iter()
      .map(|i| {
        (
          i,
          XsdName {
            namespace: None,
            local_name: String::new(),
            ty: XsdType::SimpleType,
          },
          0,
        )
      })
      .collect::<Vec<_>>();
    let mut attr_group_to_run = (0..self.attribute_group.len())
      .into_iter()
      .map(|i| {
        (
          i,
          XsdName {
            namespace: None,
            local_name: String::new(),
            ty: XsdType::AttributeGroup,
          },
          0,
        )
      })
      .collect::<Vec<_>>();
    let mut group_to_run = (0..self.groups.len())
      .into_iter()
      .map(|i| {
        (
          i,
          XsdName {
            namespace: None,
            local_name: String::new(),
            ty: XsdType::Group,
          },
          0,
        )
      })
      .collect::<Vec<_>>();
    let mut element_to_run = (0..self.elements.len())
      .into_iter()
      .map(|i| {
        (
          i,
          XsdName {
            namespace: None,
            local_name: String::new(),
            ty: XsdType::Element,
          },
          0,
        )
      })
      .collect::<Vec<_>>();
    let mut complex_type_to_run = (0..self.complex_type.len())
      .into_iter()
      .map(|i| {
        (
          i,
          XsdName {
            namespace: None,
            local_name: String::new(),
            ty: XsdType::ComplexType,
          },
          0,
        )
      })
      .collect::<Vec<_>>();

    let mut changed = true;
    while changed {
      changed = false;

      let initial_len = simple_type_to_run.len();
      simple_type_to_run = parse_type(
        simple_type_to_run,
        &mut top_level_names,
        context,
        namespace_filter,
        |context, index| self.simple_type[index].get_implementation(context),
      )?;
      if simple_type_to_run.len() != initial_len {
        changed = true;
      }

      let initial_len = attr_group_to_run.len();
      attr_group_to_run = parse_type(
        attr_group_to_run,
        &mut top_level_names,
        context,
        namespace_filter,
        |context, index| self.attribute_group[index].get_implementation(None, context),
      )?;
      if attr_group_to_run.len() != initial_len {
        changed = true;
      }

      let initial_len = group_to_run.len();
      group_to_run = parse_type(
        group_to_run,
        &mut top_level_names,
        context,
        namespace_filter,
        |context, index| self.groups[index].get_implementation(None, context),
      )?;
      if group_to_run.len() != initial_len {
        changed = true;
      }

      let initial_len = element_to_run.len();
      element_to_run = parse_type(
        element_to_run,
        &mut top_level_names,
        context,
        namespace_filter,
        |context, index| self.elements[index].get_implementation(context),
      )?;
      if element_to_run.len() != initial_len {
        changed = true;
      }

      let initial_len = complex_type_to_run.len();
      complex_type_to_run = parse_type(
        complex_type_to_run,
        &mut top_level_names,
        context,
        namespace_filter,
        |context, index| self.complex_type[index].get_implementation(true, None, context),
      )?;
      if complex_type_to_run.len() != initial_len {
        changed = true;
      }
    }

    let mut error = String::new();
    for (_, v, c) in simple_type_to_run {
      error.push_str(&format!("\nsimple_type::{v} [{c}]"));
    }
    for (_, v, c) in attr_group_to_run {
      error.push_str(&format!("\nattribute_group::{v} [{c}]"));
    }
    for (_, v, c) in group_to_run {
      error.push_str(&format!("\ngroup::{v} [{c}]"));
    }
    for (_, v, c) in element_to_run {
      error.push_str(&format!("\nelement::{v} [{c}]"));
    }
    for (_, v, c) in complex_type_to_run {
      error.push_str(&format!("\ncomplex_type::{v} [{c}]"));
    }

    if !error.is_empty() {
      return Err(XsdError::XsdParseError(format!("COULD NOT FIND:{}", error)));
    }

    Ok(top_level_names)
  }

  pub fn generate(&self, context: &mut XsdContext) -> Result<String, XsdError> {
    let top_level_names = self.fill_context(context, None)?;

    let mut dst = String::new();
    dst.push_str("use xml_schema_parser::{XsdError, XMLElementWrapper, XsdParse};\n\n");
    let mut formatter = crate::codegen::Formatter::new(&mut dst);
    for name in top_level_names {
      context
        .structs
        .get(&name)
        .unwrap()
        .fmt(&mut formatter)
        .unwrap();
    }

    std::fs::write("../../musicxml-rs/src/musicxml_sys/musicxml.rs", &dst).unwrap();

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
