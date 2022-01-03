use std::str::FromStr;

use super::{
  xsd_context::{XsdElement, XsdImpl, XsdName},
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
  // #[yaserde(attribute)]
  // pub default: Option<String>,
  // #[yaserde(attribute)]
  // pub fixed: Option<String>,
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
      "required" => Ok(Required::Optional),
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
    element.check_name("xs:attribute");

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
      element.try_get_child_with("xs:simpleType", |child| SimpleType::parse(child))?;

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
      reference,
      kind,
      required,
      simple_type,
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  pub fn get_implementation(&self, context: &mut XsdContext) -> Option<XsdImpl> {
    if self.name.is_none() {
      return None;
    }

    let rust_type = match (
      self.reference.as_ref(),
      self.kind.as_ref(),
      self.simple_type.as_ref(),
    ) {
      (None, Some(kind), None) => context
        .structs
        .get(&XsdName {
          namespace: None,
          local_name: kind.clone(),
        })
        .unwrap()
        .clone(),
      (Some(reference), None, None) => context
        .structs
        .get(&XsdName {
          namespace: None,
          local_name: reference.clone(),
        })
        .unwrap()
        .clone(),
      (None, None, Some(simple_type)) => simple_type.get_implementation(context),
      (_, _, _) => panic!("Not implemented Rust type for: {:?}", self),
    };

    let rust_type = if self.required == Required::Optional {
      Type::new(&format!("Option<{}>", rust_type.element.get_type().name))
    } else {
      rust_type.element.get_type()
    };

    let generated_impl = XsdImpl {
      element: XsdElement::Struct(
        Struct::new("attribute")
          .push_field(
            Field::new(
              &XsdName {
                namespace: None,
                local_name: self.name.clone().unwrap(),
              }
              .to_field_name(),
              rust_type,
            )
            .annotation(vec![&format!(
              "yaserde(attribute, rename={})",
              self.name.clone().unwrap()
            )])
            .to_owned(),
          )
          .to_owned(),
      ),
      ..Default::default()
    };

    Some(generated_impl)
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
      name: Some("language".to_string()),
      kind: Some("xs:string".to_string()),
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
      name: Some("type".to_string()),
      kind: Some("xs:string".to_string()),
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
      name: Some("type".to_string()),
      kind: None,
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
      kind: None,
      reference: None,
      required: Required::Optional,
      simple_type: None,
    };

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    attribute.get_implementation(&mut context);
  }

  #[test]
  fn attribute_without_name() {
    let attribute = Attribute {
      name: None,
      kind: Some("xs:string".to_string()),
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