use heck::{CamelCase, SnakeCase};
use thiserror::Error;

#[derive(Error, Debug)]
#[error("Error parsing xml node[{node_name}]: {msg}")]
pub struct XsdParseError {
  pub node_name: String,
  pub msg: String,
}

#[derive(Error, Debug)]
#[error("Error generating xsd node [{node_name}; {ty:?}]: {msg}")]
pub struct XsdGenError {
  pub node_name: String,
  pub ty: XsdType,
  pub msg: String,
}

#[derive(Error, Debug)]
pub enum XsdIoError {
  #[error(transparent)]
  XsdParseError(#[from] XsdParseError),
  #[error(transparent)]
  XsdGenError(#[from] XsdGenError),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum XsdType {
  Annotation,
  AttributeGroup,
  Attribute,
  Choice,
  ComplexContent,
  ComplexType,
  Element,
  Extension,
  Group,
  Import,
  List,
  Restriction,
  Sequence,
  SimpleContent,
  SimpleType,
  Union,
  Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct XsdName {
  pub namespace: Option<String>,
  pub local_name: String,
  pub ty: XsdType,
}

impl std::fmt::Display for XsdName {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if let Some(namespace) = &self.namespace {
      write!(f, "{}:{}", namespace, self.local_name)
    } else {
      write!(f, "{}", self.local_name)
    }
  }
}

impl XsdName {
  pub fn new(name: &str, ty: XsdType) -> Self {
    if let Some((lhs, rhs)) = name.split_once(':') {
      Self {
        namespace: Some(lhs.to_string()),
        local_name: rhs.to_string(),
        ty,
      }
    } else {
      Self {
        namespace: None,
        local_name: name.to_string(),
        ty,
      }
    }
  }

  pub fn new_namespace(name: &str, ty: XsdType, namespace: Option<&str>) -> Self {
    if let Some((lhs, rhs)) = name.split_once(':') {
      Self {
        namespace: Some(lhs.to_string()),
        local_name: rhs.to_string(),
        ty,
      }
    } else {
      Self {
        namespace: namespace.map(|s| s.to_string()),
        local_name: name.to_string(),
        ty,
      }
    }
  }

  pub fn to_struct_name(&self) -> String {
    to_struct_name(&self.local_name)
  }

  pub fn to_field_name(&self) -> String {
    to_field_name(&self.local_name)
  }
}

pub fn to_struct_name(name: &str) -> String {
  let output = name.replace(".", "_").to_camel_case();
  if let Some(char) = output.chars().next() {
    if char.is_numeric() {
      return format!("_{output}");
    }
  }

  output
}

pub fn to_field_name(name: &str) -> String {
  let name = name.to_snake_case();

  if name == "type" {
    "r#type".to_string()
  } else {
    name
  }
}
