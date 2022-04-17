use xsd_codegen::{FromXmlString, Impl, Type, XMLElement};
use xsd_types::{XsdName, XsdParseError, XsdType};

use super::{
  annotation::Annotation,
  xsd_context::{XsdElement, XsdImpl},
  XsdError,
};
use crate::xsd::{simple_type::SimpleType, XsdContext};

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Attribute {
  pub annotation: Option<Annotation>,
  pub name: Option<XsdName>,
  pub kind: Option<XsdName>,
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
  pub fn parse(mut element: XMLElement) -> Result<Self, XsdParseError> {
    element.check_name("attribute")?;

    let name = element
      .try_get_attribute("name")?
      .map(|v: String| element.new_name(&v, XsdType::Attribute));
    let reference = element
      .try_get_attribute("ref")?
      .map(|v: String| element.new_name(&v, XsdType::Attribute));

    if name.is_some() && reference.is_some() {
      return Err(XsdParseError {
        node_name: element.node_name(),
        msg: format!("name and ref cannot both present"),
      });
    }

    let kind = element
      .try_get_attribute("type")?
      .map(|v: String| XsdName::new(&v, XsdType::SimpleType));

    let simple_type =
      element.try_get_child_with("simpleType", |child| SimpleType::parse(child, false))?;

    let required = element.get_attribute_default("use")?;

    if reference.is_some() && (simple_type.is_some() || kind.is_some()) {
      return Err(XsdParseError {
        node_name: element.node_name(),
        msg: format!("type | simpleType cannot be present when ref is present",),
      });
    }

    if simple_type.is_some() && kind.is_some() {
      return Err(XsdParseError {
        node_name: element.node_name(),
        msg: format!("simpleType and type cannot both present"),
      });
    }

    let output = Self {
      annotation: element.try_get_child_with("annotation", Annotation::parse)?,
      name,
      default: element.try_get_attribute("default")?,
      fixed: element.try_get_attribute("fixed")?,
      reference,
      kind,
      required,
      simple_type,
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  #[tracing::instrument(skip_all)]
  pub fn get_implementation(&self, context: &mut XsdContext) -> Result<XsdImpl, XsdError> {
    let mut generated_impl = match (
      self.reference.as_ref(),
      self.kind.as_ref(),
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

          XsdImpl {
            name: name.clone(),
            element: XsdElement::TypeAlias(
              Type::new(&name.to_struct_name()),
              inner.element.get_type(),
            ),
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

        XsdImpl {
          name: name.clone(),
          element: XsdElement::TypeAlias(
            Type::new(&name.to_struct_name()),
            inner.element.get_type().path(&name.to_field_name()),
          ),
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

    let mut r#impl = Impl::new(&rust_type);

    // let parse = r#impl.new_fn("parse");
    // parse.arg("mut element", "XMLElementWrapper");
    // parse.ret("Result<Self, XsdError>");

    // parse.line(format!(
    //   "element.get_attribute(\"{}\")",
    //   self.name.as_ref().unwrap()
    // ));
    // parse.line("Ok(output)");

    let docs = self
      .annotation
      .as_ref()
      .map(|annotation| annotation.get_doc())
      .unwrap_or_default();
    generated_impl.element.add_doc(&docs.join(""));

    Ok(generated_impl)
  }
}
