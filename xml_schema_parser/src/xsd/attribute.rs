use std::str::FromStr;

use super::{
  xsd_context::{to_field_name, XsdElement, XsdImpl, XsdName},
  XMLElementWrapper, XsdError,
};
use crate::{
  codegen::{Field, Struct, Type},
  xsd::{simple_type::SimpleType, XsdContext},
};

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(
//   rename = "attribute",
//   prefix = "xs",
//   namespace = "xs: http://www.w3.org/2001/XMLSchema"
// )]
pub struct Attribute {
  pub name: Option<String>,
  pub kind: Option<String>,
  pub default: Option<String>,
  pub fixed: Option<String>,
  pub required: Required,
  pub reference: Option<String>,
  pub simple_type: Option<SimpleType>,
}

#[derive(Clone, Debug, PartialEq, YaDeserialize)]
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

    let name = element.try_get_attribute("name")?;
    let reference = element.try_get_attribute("ref")?;

    if name.is_some() && reference.is_some() {
      return Err(XsdError::XsdParseError(format!(
        "name and ref cannot both present in {}",
        element.name()
      )));
    }

    let kind = element.try_get_attribute("type")?;

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
  pub fn get_implementation(&self, context: &mut XsdContext) -> Result<Option<XsdImpl>, XsdError> {
    if self.name.is_none() {
      return Ok(None);
    }

    let rust_type = match (
      self.reference.as_ref(),
      self.kind.as_ref(),
      self.simple_type.as_ref(),
    ) {
      (Some(reference), None, None) => {
        let name = XsdName {
          namespace: None,
          local_name: reference.clone(),
        };
        if let Some(str) = context.structs.get(&name) {
          str.clone()
        } else {
          return Err(XsdError::XsdImplNotFound(name));
        }
      }
      (None, Some(kind), None) => {
        let name = XsdName {
          namespace: None,
          local_name: kind.clone(),
        };
        if let Some(str) = context.structs.get(&name) {
          str.clone()
        } else {
          return Err(XsdError::XsdImplNotFound(name));
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

    let generated_impl = XsdImpl {
      element: XsdElement::Type(rust_type),
      name: XsdName::new(self.name.as_ref().unwrap()),
      fieldname_hint: Some(to_field_name(self.name.as_ref().unwrap())),
      inner: vec![],
      implementation: vec![],
    };

    Ok(Some(generated_impl))
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
      name: Some("language".to_string()),
      kind: Some("xs:string".to_string()),
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
      name: Some("language".to_string()),
      kind: Some("xs:string".to_string()),
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
      name: Some("type".to_string()),
      kind: Some("xs:string".to_string()),
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
      name: Some("type".to_string()),
      kind: None,
      default: None,
      fixed: None,
      reference: Some("MyType".to_string()),
      required: Required::Optional,
      simple_type: None,
    };

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    let value = attribute
      .get_implementation(&mut context)
      .unwrap()
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
      name: Some("type".to_string()),
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
      name: None,
      kind: Some("xs:string".to_string()),
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
      .unwrap()
      .to_string()
      .unwrap();
    let implementation = quote!(#value).to_string();
    assert_eq!(implementation, "".to_string());
  }
}
