use xsd_codegen::{Field, FromXmlString, Impl, Type, XMLElement};
use xsd_types::{XsdIoError, XsdName, XsdParseError, XsdType};

use super::{
  annotation::Annotation,
  general_xsdgen,
  xsd_context::{XsdElement, XsdImpl},
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
        msg: format!("name and ref cannot both present"),
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
        msg: format!("type | simpleType cannot be present when ref is present",),
      }));
    }

    if simple_type.is_some() && r#type.is_some() {
      return Err(XsdIoError::XsdParseError(XsdParseError {
        node_name: element.node_name(),
        msg: format!("simpleType and type cannot both present"),
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
        if let Some(inner) = context.search(&reference) {
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
            element: XsdElement::Type(inner.element.get_type()),
            fieldname_hint: Some(name.to_field_name()),
            inner: vec![],
            implementation: vec![],
          }
        } else {
          return Err(XsdError::XsdImplNotFound(reference.clone()));
        }
      }
      (None, Some(kind), None) => {
        if let Some(inner) = context.search(kind) {
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
            XsdElement::TypeAlias(
              Type::new(None, &name.to_struct_name()),
              inner.element.get_type(),
            )
          } else {
            XsdElement::Field(
              Field::new(
                Some(name.clone()),
                &name.to_field_name(),
                inner.element.get_type(),
              )
              .vis("pub"),
            )
          };

          XsdImpl {
            name: name.clone(),
            element,
            fieldname_hint: Some(name.to_field_name()),
            inner: vec![],
            implementation: vec![],
          }
        } else {
          return Err(XsdError::XsdImplNotFound(kind.clone()));
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
          XsdElement::TypeAlias(
            Type::new(None, &name.to_struct_name()),
            inner.element.get_type().path(&name.to_field_name()),
          )
        } else {
          XsdElement::Field(
            Field::new(
              Some(name.clone()),
              &name.to_field_name(),
              inner.element.get_type().path(&name.to_field_name()),
            )
            .vis("pub"),
          )
        };

        XsdImpl {
          name: name.clone(),
          element,
          fieldname_hint: Some(name.to_field_name()),
          inner: vec![inner],
          implementation: vec![],
        }
      }
      (_, _, _) => panic!("Not implemented Rust type for: {:?}", self),
    };

    let rust_type = if self.required == Required::Optional {
      generated_impl.element.get_type().wrap("Option")
    } else {
      generated_impl.element.get_type()
    };

    if let Some(doc) = &self.annotation {
      generated_impl.element.add_doc(&doc.get_doc().join(""));
    }

    let generated_impl = general_xsdgen(generated_impl);

    Ok(generated_impl)
  }
}
