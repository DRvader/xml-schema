use xsd_codegen::{FromXmlString, Type, TypeAlias, XMLElement};
use xsd_types::{XsdIoError, XsdName, XsdParseError, XsdType};

use super::{
  annotation::Annotation,
  general_xsdgen,
  xsd_context::{XsdImpl, XsdImplType},
  XsdError,
};
use crate::xsd::{simple_type::SimpleType, XsdContext};

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Attribute {
  pub annotation: Option<Annotation>,
  pub name: Option<XsdName>,
  pub r#type: Option<XsdName>,
  pub default: Option<String>,
  pub fixed: Option<String>,
  pub required: Required,
  pub reference: Option<XsdName>,
  pub simple_type: Option<SimpleType>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Required {
  Optional,
  Required,
}

impl FromXmlString for Required {
  fn from_xml(s: &str) -> Result<Self, String> {
    match s {
      "optional" => Ok(Required::Optional),
      "required" => Ok(Required::Required),
      err => Err(format!(
        "{} is not a valid value for optional|required",
        err
      )),
    }
  }
}

impl Default for Required {
  fn default() -> Self {
    Required::Optional
  }
}

impl Attribute {
  pub fn parse(mut element: XMLElement) -> Result<Self, XsdIoError> {
    element.check_name("attribute")?;

    let name = element
      .try_get_attribute("name")?
      .map(|v: String| element.new_name(&v, XsdType::Attribute));
    let reference = element
      .try_get_attribute("ref")?
      .map(|v: String| element.new_name(&v, XsdType::Attribute));

    if name.is_some() && reference.is_some() {
      return Err(XsdIoError::XsdParseError(XsdParseError {
        node_name: element.node_name(),
        msg: "name and ref cannot both present".to_string(),
      }));
    }

    let r#type = element
      .try_get_attribute("type")?
      .map(|v: String| XsdName::new(&v, XsdType::SimpleType));

    let simple_type =
      element.try_get_child_with("simpleType", |child| SimpleType::parse(child, false))?;

    let required = element.get_attribute_default("use")?;

    if reference.is_some() && (simple_type.is_some() || r#type.is_some()) {
      return Err(XsdIoError::XsdParseError(XsdParseError {
        node_name: element.node_name(),
        msg: "type | simpleType cannot be present when ref is present".to_string(),
      }));
    }

    if simple_type.is_some() && r#type.is_some() {
      return Err(XsdIoError::XsdParseError(XsdParseError {
        node_name: element.node_name(),
        msg: "simpleType and type cannot both present".to_string(),
      }));
    }

    let output = Self {
      annotation: element.try_get_child_with("annotation", Annotation::parse)?,
      name,
      default: element.try_get_attribute("default")?,
      fixed: element.try_get_attribute("fixed")?,
      reference,
      r#type,
      required,
      simple_type,
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  #[tracing::instrument(skip_all)]
  pub fn get_implementation(
    &self,
    context: &mut XsdContext,
    parent_is_schema: bool,
  ) -> Result<XsdImpl, XsdError> {
    let mut generated_impl = match (
      self.reference.as_ref(),
      self.r#type.as_ref(),
      self.simple_type.as_ref(),
    ) {
      (Some(reference), None, None) => {
        if let Some(inner) = context.search(reference) {
          let name = if let Some(name) = &self.name {
            name.clone()
          } else {
            XsdName {
              namespace: reference.namespace.clone(),
              local_name: inner.infer_type_name(),
              ty: XsdType::Attribute,
            }
          };

          XsdImpl {
            name: name.clone(),
            element: XsdImplType::Type(inner.element.get_type()),
            fieldname_hint: Some(name.to_field_name()),
            inner: vec![],
            implementation: vec![],
            flatten: self.name.is_none(),
          }
        } else {
          return Err(XsdError::XsdImplNotFound(reference.clone()));
        }
      }
      (None, Some(r#type), None) => {
        if let Some(inner) = context.search(r#type) {
          let name = if let Some(name) = &self.name {
            name.clone()
          } else {
            XsdName {
              namespace: context.xml_schema_prefix.clone(),
              local_name: inner.name.local_name.clone(),
              ty: XsdType::Attribute,
            }
          };

          let element = if parent_is_schema {
            XsdImplType::TypeAlias(TypeAlias {
              doc: None,
              alias: Type::new(Some(name.clone()), &name.to_struct_name()),
              value: inner.element.get_type(),
            })
          } else {
            XsdImplType::Type(inner.element.get_type().xml_name(Some(name.clone())))
          };

          XsdImpl {
            name: name.clone(),
            element,
            fieldname_hint: Some(name.to_field_name()),
            inner: vec![],
            implementation: vec![],
            flatten: false,
          }
        } else {
          return Err(XsdError::XsdImplNotFound(r#type.clone()));
        }
      }
      (None, None, Some(simple_type)) => {
        let inner = simple_type.get_implementation(self.name.clone(), context)?;

        let name = if let Some(name) = &self.name {
          name.clone()
        } else {
          XsdName {
            namespace: context.xml_schema_prefix.clone(),
            local_name: inner.name.local_name.clone(),
            ty: XsdType::Attribute,
          }
        };

        let element = if parent_is_schema {
          XsdImplType::TypeAlias(TypeAlias {
            doc: None,
            alias: Type::new(Some(name.clone()), &name.to_struct_name()),
            value: inner.element.get_type().path(&name.to_field_name()),
          })
        } else {
          XsdImplType::Type(
            inner
              .element
              .get_type()
              .path(&name.to_field_name())
              .xml_name(Some(name.clone())),
          )
        };

        XsdImpl {
          name: name.clone(),
          element,
          fieldname_hint: Some(name.to_field_name()),
          inner: vec![inner],
          implementation: vec![],
          flatten: false,
        }
      }
      (_, _, _) => panic!("Not implemented Rust type for: {:?}", self),
    };

    if let Some(doc) = &self.annotation {
      generated_impl.element.add_doc(&doc.get_doc().join(""));
    }

    let mut generated_impl = general_xsdgen(generated_impl);

    let generated_impl = if !parent_is_schema {
      if let Required::Optional = self.required {
        let old_name = generated_impl.name.clone();
        let outer_element = generated_impl.element.get_type().wrap("Option");
        generated_impl.name.local_name = format!("inner-{}", old_name.local_name);
        XsdImpl {
          name: old_name,
          fieldname_hint: Some(generated_impl.fieldname_hint.clone().unwrap()),
          element: XsdImplType::Type(outer_element),
          inner: vec![generated_impl],
          implementation: vec![],
          flatten: false,
        }
      } else {
        generated_impl
      }
    } else {
      generated_impl
    };

    Ok(generated_impl)
  }
}
