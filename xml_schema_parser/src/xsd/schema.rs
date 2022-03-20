use std::collections::BTreeMap;

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

    let target_namespace: Option<String> = element.try_get_attribute("targetNamespace")?;

    element.default_namespace = target_namespace.clone();

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
      target_namespace,
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

    context.xml_schema_prefix = self.target_namespace.clone();

    let mut top_level_names = vec![];

    let mut to_run = BTreeMap::new();

    for (index, ty) in self.imports.iter().enumerate() {
      to_run.insert(
        XsdName {
          namespace: None,
          local_name: ty
            .schema_location
            .as_ref()
            .unwrap_or_else(|| ty.namespace.as_ref().unwrap())
            .clone(),
          ty: XsdType::Import,
        },
        (Some(index), 0),
      );
    }

    for (index, _) in self.annotations.iter().enumerate() {
      to_run.insert(
        XsdName {
          namespace: None,
          local_name: index.to_string(),
          ty: XsdType::Annotation,
        },
        (Some(index), 0),
      );
    }

    for (index, ty) in self.elements.iter().enumerate() {
      to_run.insert(ty.name.as_ref().unwrap().clone(), (Some(index), 0));
    }

    for (index, ty) in self.simple_type.iter().enumerate() {
      to_run.insert(ty.name.as_ref().unwrap().clone(), (Some(index), 0));
    }

    for (index, ty) in self.complex_type.iter().enumerate() {
      to_run.insert(ty.name.as_ref().unwrap().clone(), (Some(index), 0));
    }

    for (index, ty) in self.simple_type.iter().enumerate() {
      to_run.insert(ty.name.as_ref().unwrap().clone(), (Some(index), 0));
    }

    for (index, ty) in self.attributes.iter().enumerate() {
      to_run.insert(ty.name.as_ref().unwrap().clone(), (Some(index), 0));
    }

    for (index, ty) in self.attribute_group.iter().enumerate() {
      to_run.insert(ty.name.as_ref().unwrap().clone(), (Some(index), 0));
    }

    for (index, ty) in self.groups.iter().enumerate() {
      to_run.insert(ty.name.as_ref().unwrap().clone(), (Some(index), 0));
    }

    let mut next_to_run = BTreeMap::new();

    let mut changed = true;
    while changed {
      changed = false;

      for (type_to_run, (index, error)) in &to_run {
        if let Some(index) = index {
          let result = match &type_to_run.ty {
            XsdType::Import => {
              self.imports[*index].get_implementation(context)?;
              None
            }
            XsdType::Annotation => {
              self.annotations[*index].get_doc();
              None
            }
            XsdType::Element => Some(self.elements[*index].get_implementation(context)),
            XsdType::SimpleType => Some(self.simple_type[*index].get_implementation(context)),
            XsdType::ComplexType => {
              Some(self.complex_type[*index].get_implementation(true, None, context))
            }
            XsdType::Attribute => Some(self.attributes[*index].get_implementation(context)),
            XsdType::AttributeGroup => {
              Some(self.attribute_group[*index].get_implementation(None, context))
            }
            XsdType::Group => Some(self.groups[*index].get_implementation(None, context)),
            ty => unreachable!("Unexpected top-level type {ty:?}"),
          };
          if let Some(result) = result {
            match result {
              Ok(temp) => {
                changed = true;
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
                context.insert_impl(temp.name.clone(), temp);
              }
              Err(ty) => match ty {
                XsdError::XsdImplNotFound(name) => {
                  if &name != type_to_run {
                    next_to_run.insert(type_to_run.clone(), (Some(*index), *error + 1));
                  }

                  let curr = to_run
                    .get(&name)
                    .map(|v| (v.0, v.1 + 1))
                    .unwrap_or_else(|| (None, 0));
                  next_to_run.insert(name, curr);
                }
                _ => return Err(ty),
              },
            }
          }
        }
      }

      std::mem::swap(&mut to_run, &mut next_to_run);
      next_to_run.clear();
    }

    let mut error_msg = String::new();
    for (name, (index, error)) in to_run {
      error_msg.push_str(&format!(
        "\n[{:?}] {}{name} [{error}]",
        name.ty,
        if index.is_some() { "*" } else { "" }
      ));
    }

    if !error_msg.is_empty() {
      return Err(XsdError::XsdParseError(format!(
        "COULD NOT FIND:{}",
        error_msg
      )));
    }

    dbg!(&top_level_names);

    Ok(top_level_names)
  }

  pub fn generate(&self, context: &mut XsdContext) -> Result<String, XsdError> {
    let top_level_names = self.fill_context(context, None)?;

    let mut dst = String::new();
    dst.push_str("use xml_schema_parser::{XsdError, XMLElementWrapper, XsdParse};\n\n");
    let mut formatter = crate::codegen::Formatter::new(&mut dst);
    for name in top_level_names {
      context.search(&name).unwrap().fmt(&mut formatter).unwrap();
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
