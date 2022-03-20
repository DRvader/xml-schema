use std::str::FromStr;

use super::{
  annotation::Annotation,
  xsd_context::{to_field_name, XsdElement, XsdImpl, XsdName, XsdType},
  XMLElementWrapper, XsdError,
};
use crate::{
  codegen::{Field, Impl, Type},
  xsd::{simple_type::SimpleType, XsdContext},
};

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(
//   rename = "attribute",
//   prefix = "xs",
//   namespace = "xs: http://www.w3.org/2001/XMLSchema"
// )]
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

impl FromStr for Required {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
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
  pub fn parse(mut element: XMLElementWrapper) -> Result<Self, XsdError> {
    element.check_name("attribute")?;

    let name = element
      .try_get_attribute("name")?
      .map(|v: String| element.new_name(&v, XsdType::Attribute));
    let reference = element
      .try_get_attribute("ref")?
      .map(|v: String| element.new_name(&v, XsdType::Attribute));

    if name.is_some() && reference.is_some() {
      return Err(XsdError::XsdParseError(format!(
        "name and ref cannot both present in {}",
        element.name()
      )));
    }

    let kind = element
      .try_get_attribute("type")?
      .map(|v: String| XsdName::new(&v, XsdType::SimpleType));

    let simple_type =
      element.try_get_child_with("simpleType", |child| SimpleType::parse(child, false))?;

    let required = element.get_attribute_default("use")?;

    if reference.is_some() && (simple_type.is_some() || kind.is_some()) {
      return Err(XsdError::XsdParseError(format!(
        "Error in {} type | simpleType cannot be present when ref is present",
        element.name()
      )));
    }

    if simple_type.is_some() && kind.is_some() {
      return Err(XsdError::XsdParseError(format!(
        "simpleType and type cannot both present in {}",
        element.name()
      )));
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
    let rust_type = match (
      self.reference.as_ref(),
      self.kind.as_ref(),
      self.simple_type.as_ref(),
    ) {
      (Some(reference), None, None) => {
        if let Some(inner) = context.search(&reference) {
          let field_name = if let Some(name) = &self.name {
            name.to_field_name()
          } else if let Some(field_hint) = &inner.fieldname_hint {
            field_hint.clone()
          } else {
            to_field_name(&inner.infer_type_name())
          };

          let name = if let Some(name) = &self.name {
            name.clone()
          } else {
            XsdName {
              namespace: reference.namespace.clone(),
              local_name: inner.infer_type_name(),
              ty: super::xsd_context::XsdType::Attribute,
            }
          };

          XsdImpl {
            name,
            element: XsdElement::Field(
              Field::new(&field_name, inner.element.get_type())
                .vis("pub")
                .to_owned(),
            ),
            fieldname_hint: Some(field_name.to_string()),
            inner: vec![],
            implementation: vec![],
          }
        } else {
          return Err(XsdError::XsdImplNotFound(reference.clone()));
        }
      }
      (None, Some(kind), None) => {
        if let Some(inner) = context.search(kind) {
          let field_name = if let Some(name) = &self.name {
            name.to_field_name()
          } else if let Some(field_hint) = &inner.fieldname_hint {
            field_hint.clone()
          } else {
            to_field_name(&inner.infer_type_name())
          };

          let name = if let Some(name) = &self.name {
            name.clone()
          } else {
            XsdName {
              namespace: kind.namespace.clone(),
              local_name: inner.infer_type_name(),
              ty: XsdType::Attribute,
            }
          };

          XsdImpl {
            name,
            element: XsdElement::Field(
              Field::new(&field_name, inner.element.get_type())
                .vis("pub")
                .to_owned(),
            ),
            fieldname_hint: Some(field_name.to_string()),
            inner: vec![],
            implementation: vec![],
          }
        } else {
          return Err(XsdError::XsdImplNotFound(kind.clone()));
        }
      }
      (None, None, Some(simple_type)) => simple_type.get_implementation(context)?,
      (_, _, _) => panic!("Not implemented Rust type for: {:?}", self),
    };

    let rust_type = if self.required == Required::Optional {
      Type::new(&format!("Option<{}>", rust_type.element.get_type().name))
    } else {
      rust_type.element.get_type()
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

    let mut generated_impl = XsdImpl {
      element: XsdElement::Type(rust_type),
      name: self
        .name
        .clone()
        .unwrap_or_else(|| self.reference.clone().unwrap()),
      fieldname_hint: self.name.as_ref().map(|v| v.to_field_name()),
      inner: vec![],
      implementation: vec![],
    };

    let docs = self
      .annotation
      .as_ref()
      .map(|annotation| annotation.get_doc())
      .unwrap_or_default();
    generated_impl.element.add_doc(&docs.join(""));

    Ok(generated_impl)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_required() {
    assert_eq!(Required::default(), Required::Optional);
  }

  #[test]
  fn string_attribute() {
    let attribute = Attribute {
      annotation: None,
      name: Some(XsdName::new("language", XsdType::Attribute)),
      kind: Some(XsdName::new("xs:string", XsdType::SimpleType)),
      default: None,
      fixed: None,
      reference: None,
      required: Required::Required,
      simple_type: None,
    };

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    let value = attribute
      .get_implementation(&mut context)
      .unwrap()
      .to_string()
      .unwrap();
    let implementation = quote!(#value).to_string();
    assert_eq!(
      implementation,
      r#"# [ yaserde ( attribute ) ] pub language : String ,"#
    );
  }

  #[test]
  fn optional_string_attribute() {
    let attribute = Attribute {
      annotation: None,
      name: Some(XsdName::new("language", XsdType::Attribute)),
      kind: Some(XsdName::new("xs:string", XsdType::SimpleType)),
      default: None,
      fixed: None,
      reference: None,
      required: Required::Optional,
      simple_type: None,
    };

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    let value = attribute
      .get_implementation(&mut context)
      .unwrap()
      .to_string()
      .unwrap();
    let implementation = quote!(#value).to_string();
    assert_eq!(
      implementation,
      r#"# [ yaserde ( attribute ) ] pub language : Option < String > ,"#
    );
  }

  #[test]
  fn type_attribute() {
    let attribute = Attribute {
      annotation: None,
      name: Some(XsdName::new("language", XsdType::Attribute)),
      kind: Some(XsdName::new("xs:string", XsdType::SimpleType)),
      default: None,
      fixed: None,
      reference: None,
      required: Required::Optional,
      simple_type: None,
    };

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    let value = attribute
      .get_implementation(&mut context)
      .unwrap()
      .to_string()
      .unwrap();
    let implementation = quote!(#value).to_string();
    assert_eq!(
      implementation,
      r#"# [ yaserde ( attribute , rename = "type" ) ] pub kind : Option < String > ,"#
    );
  }

  #[test]
  fn reference_type_attribute() {
    let attribute = Attribute {
      annotation: None,
      name: Some(XsdName::new("language", XsdType::Attribute)),
      kind: None,
      default: None,
      fixed: None,
      reference: Some(XsdName::new("MyType", XsdType::Attribute)),
      required: Required::Optional,
      simple_type: None,
    };

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    let value = attribute
      .get_implementation(&mut context)
      .unwrap()
      .to_string()
      .unwrap();
    let implementation = quote!(#value).to_string();
    assert_eq!(
      implementation,
      r#"# [ yaserde ( attribute , rename = "type" ) ] pub kind : Option < MyType > ,"#
    );
  }

  #[test]
  #[should_panic]
  fn bad_type_attribute() {
    let attribute = Attribute {
      annotation: None,
      name: Some(XsdName::new("type", XsdType::Attribute)),
      default: None,
      fixed: None,
      kind: None,
      reference: None,
      required: Required::Optional,
      simple_type: None,
    };

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    attribute.get_implementation(&mut context).unwrap();
  }

  #[test]
  fn attribute_without_name() {
    let attribute = Attribute {
      annotation: None,
      name: None,
      kind: Some(XsdName::new("xs:string", XsdType::SimpleType)),
      default: None,
      fixed: None,
      reference: None,
      required: Required::Optional,
      simple_type: None,
    };

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    let value = attribute
      .get_implementation(&mut context)
      .unwrap()
      .to_string()
      .unwrap();
    let implementation = quote!(#value).to_string();
    assert_eq!(implementation, "".to_string());
  }
}
