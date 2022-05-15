use std::collections::BTreeMap;

use xsd_codegen::{Formatter, XMLElement};
use xsd_types::{XsdIoError, XsdName, XsdType};

use crate::xsd::{
  attribute, attribute_group, complex_type, element, group, import, qualification, simple_type,
  XsdContext,
};

use super::{annotation, XsdError};

#[derive(Clone, Debug, PartialEq)]
pub enum SchemaOptions {
  Import(import::Import),
  Annotation(annotation::Annotation),
  Element(element::Element),
  SimpleType(simple_type::SimpleType),
  ComplexType(complex_type::ComplexType),
  Attribute(attribute::Attribute),
  AttributeGroup(attribute_group::AttributeGroup),
  Group(group::Group),
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Schema {
  pub target_namespace: Option<String>,
  pub element_form_default: qualification::Qualification,
  pub attribute_form_default: qualification::Qualification,
  pub children: Vec<SchemaOptions>,
  pub extra: Vec<(String, String)>,
}

impl Schema {
  pub fn parse(mut element: XMLElement) -> Result<Self, XsdIoError> {
    element.check_name("schema")?;

    let target_namespace: Option<String> = element.try_get_attribute("targetNamespace")?;

    element.default_namespace = target_namespace.clone();

    let mut children = vec![];
    for child in element.get_all_children() {
      children.push(match child.element.name.as_str() {
        "annotation" => SchemaOptions::Annotation(annotation::Annotation::parse(child)?),
        "import" => SchemaOptions::Import(import::Import::parse(child)?),
        "element" => SchemaOptions::Element(element::Element::parse(child, true)?),
        "simpleType" => SchemaOptions::SimpleType(simple_type::SimpleType::parse(child, true)?),
        "complexType" => SchemaOptions::ComplexType(complex_type::ComplexType::parse(child)?),
        "attribute" => SchemaOptions::Attribute(attribute::Attribute::parse(child)?),
        "attributeGroup" => {
          SchemaOptions::AttributeGroup(attribute_group::AttributeGroup::parse(child)?)
        }
        "group" => SchemaOptions::Group(group::Group::parse(child)?),
        name => unreachable!("Unexpected child name {name}"),
      });
    }

    let output = Self {
      target_namespace,
      element_form_default: element.get_attribute_default("elementFormDefault")?,
      attribute_form_default: element.get_attribute_default("attributeFormDefault")?,
      children,
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

    for (index, child) in self.children.iter().enumerate() {
      match child {
        SchemaOptions::Import(ty) => {
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
        SchemaOptions::Annotation(_) => {
          to_run.insert(
            XsdName {
              namespace: None,
              local_name: index.to_string(),
              ty: XsdType::Annotation,
            },
            (Some(index), 0),
          );
        }
        SchemaOptions::Element(ty) => {
          to_run.insert(ty.name.as_ref().unwrap().clone(), (Some(index), 0));
        }
        SchemaOptions::SimpleType(ty) => {
          to_run.insert(ty.name.as_ref().unwrap().clone(), (Some(index), 0));
        }
        SchemaOptions::ComplexType(ty) => {
          to_run.insert(ty.name.as_ref().unwrap().clone(), (Some(index), 0));
        }
        SchemaOptions::Attribute(ty) => {
          to_run.insert(ty.name.as_ref().unwrap().clone(), (Some(index), 0));
        }
        SchemaOptions::AttributeGroup(ty) => {
          to_run.insert(ty.name.as_ref().unwrap().clone(), (Some(index), 0));
        }
        SchemaOptions::Group(ty) => {
          to_run.insert(ty.name.as_ref().unwrap().clone(), (Some(index), 0));
        }
      }
    }

    let mut next_to_run = BTreeMap::new();

    let mut changed = true;
    while changed {
      changed = false;

      for (type_to_run, (index, _error)) in &to_run {
        if let Some(index) = index {
          let result = match &self.children[*index] {
            SchemaOptions::Import(import) => {
              import.get_implementation(context)?;
              None
            }
            SchemaOptions::Annotation(annotation) => {
              annotation.get_doc();
              None
            }
            SchemaOptions::Element(element) => Some(element.get_implementation(context)),
            SchemaOptions::SimpleType(simple_type) => {
              Some(simple_type.get_implementation(None, context))
            }
            SchemaOptions::ComplexType(complex_type) => {
              Some(complex_type.get_implementation(true, None, context))
            }
            SchemaOptions::Attribute(attribute) => {
              Some(attribute.get_implementation(context, true))
            }
            SchemaOptions::AttributeGroup(attribute_group) => {
              Some(attribute_group.get_implementation(None, context))
            }
            SchemaOptions::Group(group) => Some(group.get_implementation(None, context)),
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

                // It's possible that a type was missed earlier in the loop and
                // added to the need to run queue. If we found it now, we can just remove it.
                next_to_run.remove(&temp.name);

                context.insert_impl(temp.name.clone(), temp);
              }
              Err(ty) => match ty {
                XsdError::XsdImplNotFound(name) => {
                  let curr = to_run
                    .get(&name)
                    .map(|v| (v.0, v.1 + 1))
                    .unwrap_or_else(|| (None, 0));
                  next_to_run.insert(name, curr);

                  let curr = to_run
                    .get(type_to_run)
                    .map(|v| (v.0, v.1 + 1))
                    .unwrap_or_else(|| (None, 0));
                  next_to_run.insert(type_to_run.clone(), curr);
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
      return Err(XsdError::XsdMissing(format!(
        "COULD NOT FIND:{}",
        error_msg
      )));
    }

    Ok(top_level_names)
  }

  pub fn generate(&self, context: &mut XsdContext) -> Result<String, XsdError> {
    let top_level_names = self.fill_context(context, None)?;

    let mut dst = String::new();
    dst.push_str(
      "use xml_schema_parser::{XsdIoError, XsdGenError, XMLElement, XsdType, XsdGen, GenState, GenType, Date, FromXmlString, RestrictedVec};\n\n",
    );
    let mut formatter = Formatter::new(&mut dst);
    // for name in top_level_names {
    //   context.search(&name).unwrap().fmt(&mut formatter).unwrap();
    // }

    for value in context.structs.values() {
      value.fmt(&mut formatter).unwrap()
    }

    Ok(dst)
  }
}
