mod annotation;
mod attribute;
mod attribute_group;
mod choice;
mod complex_content;
mod complex_type;
mod element;
mod enumeration;
mod extension;
mod group;
mod import;
mod list;
mod max_occurences;
mod qualification;
mod restriction;
mod schema;
mod sequence;
mod simple_content;
mod simple_type;
mod union;
mod xsd_context;

use std::fs;
use std::str::FromStr;
use thiserror::Error;
use xml::namespace::{NS_XML_PREFIX, NS_XML_URI};
use xmltree::{Element, XMLNode};
use xsd_context::XsdContext;

use self::xsd_context::{XsdName, XsdType};

#[derive(Error, Debug)]
pub enum XsdError {
  #[error("{0} not found")]
  XsdImplNotFound(XsdName),
  #[error("{0}")]
  XsdParseError(String),
  #[error(transparent)]
  XmlParseError(#[from] xmltree::ParseError),
  #[error("Error parsing node[{node_name}]: {msg}")]
  XsdGenError { node_name: String, msg: String },
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[error("Unknown Xsd error")]
  Unknown,
  #[error(transparent)]
  NetworkError(#[from] reqwest::Error),
  #[error(transparent)]
  Infalible(#[from] std::convert::Infallible),
}

pub struct XMLElementWrapper {
  element: Element,
  default_namespace: Option<String>,
}

impl XMLElementWrapper {
  pub fn name(&self) -> &str {
    &self.element.name
  }

  pub fn new_name(&self, name: &str, ty: XsdType) -> XsdName {
    XsdName::new_namespace(
      name,
      ty,
      self.default_namespace.as_ref().map(|s| s.as_str()),
    )
  }

  fn check_name(&self, name: &str) -> Result<(), XsdError> {
    if self.element.name != name {
      Err(XsdError::XsdParseError(format!(
        "Unexpected element name {} expected {}",
        name, self.element.name
      )))
    } else {
      Ok(())
    }
  }

  fn get_children(&mut self, name: &str) -> Vec<XMLElementWrapper> {
    let mut output = Vec::new();
    while let Some(child) = self.element.take_child(name) {
      output.push(XMLElementWrapper {
        element: child,
        default_namespace: self.default_namespace.clone(),
      });
    }

    output
  }

  fn get_child(&mut self, name: &str) -> Result<XMLElementWrapper, XsdError> {
    let mut output = self.get_children(name);
    if output.len() != 1 {
      return Err(XsdError::XsdParseError(format!(
        "Expected 1 child named {} found {}",
        name,
        output.len(),
      )));
    }

    Ok(output.remove(0))
  }

  fn try_get_child(&mut self, name: &str) -> Result<Option<XMLElementWrapper>, XsdError> {
    let mut output = self.get_children(name);
    if output.len() > 1 {
      return Err(XsdError::XsdParseError(format!(
        "Expected 0 or 1 children named {} found {}",
        name,
        output.len(),
      )));
    }

    if output.is_empty() {
      Ok(None)
    } else {
      Ok(Some(output.remove(0)))
    }
  }

  fn get_children_with_filter<T>(
    &mut self,
    name: &str,
    func: impl Fn(XMLElementWrapper) -> Result<Option<T>, XsdError>,
  ) -> Result<Vec<T>, XsdError> {
    let mut output = Vec::new();
    for child in self.get_children(name) {
      if let Some(child) = func(child)? {
        output.push(child);
      }
    }

    Ok(output)
  }

  fn has_child(&self, name: &str) -> bool {
    self.element.get_child(name).is_some()
  }

  fn has_attr(&self, name: &str) -> bool {
    self.element.attributes.contains_key(name)
  }

  fn get_children_with<T>(
    &mut self,
    name: &str,
    func: impl Fn(XMLElementWrapper) -> Result<T, XsdError>,
  ) -> Result<Vec<T>, XsdError> {
    self.get_children_with_filter(name, |child| func(child).map(Some))
  }

  pub fn get_child_with<T>(
    &mut self,
    name: &str,
    func: impl FnOnce(XMLElementWrapper) -> Result<T, XsdError>,
  ) -> Result<T, XsdError> {
    func(self.get_child(name)?)
  }

  pub fn try_get_child_with<T>(
    &mut self,
    name: &str,
    func: impl FnOnce(XMLElementWrapper) -> Result<T, XsdError>,
  ) -> Result<Option<T>, XsdError> {
    if let Some(child) = self.try_get_child(name)? {
      Ok(Some(func(child)?))
    } else {
      Ok(None)
    }
  }

  pub fn try_get_attribute<T: FromStr>(&mut self, name: &str) -> Result<Option<T>, XsdError>
  where
    <T as FromStr>::Err: ToString,
  {
    let value = self.element.attributes.remove(name);
    if let Some(value) = value {
      Ok(Some(value.parse::<T>().map_err(|e| {
        XsdError::XsdParseError(format!(
          "Error parsing {} in {}: {}",
          name,
          self.element.name,
          e.to_string()
        ))
      })?))
    } else {
      Ok(None)
    }
  }

  pub fn get_attribute<T: FromStr>(&mut self, name: &str) -> Result<T, XsdError>
  where
    <T as FromStr>::Err: ToString,
  {
    match self.try_get_attribute(name)? {
      Some(output) => Ok(output),
      None => Err(XsdError::XsdParseError(format!("{} not found", name))),
    }
  }

  fn get_attribute_default<T: Default + FromStr>(&mut self, name: &str) -> Result<T, XsdError>
  where
    <T as FromStr>::Err: ToString,
  {
    match self.try_get_attribute(name)? {
      Some(output) => Ok(output),
      None => Ok(T::default()),
    }
  }

  fn get_remaining_attributes(&mut self) -> Vec<(String, String)> {
    self.element.attributes.drain().collect()
  }

  pub fn try_get_content<T: FromStr>(&mut self) -> Result<Option<T>, XsdError>
  where
    <T as FromStr>::Err: ToString,
  {
    let value = self.element.get_text();
    if let Some(value) = value {
      Ok(Some(value.parse::<T>().map_err(|e| {
        XsdError::XsdParseError(format!(
          "Error parsing node text in {}: {}",
          self.element.name,
          e.to_string()
        ))
      })?))
    } else {
      Ok(None)
    }
  }

  pub fn get_content<T: FromStr>(&mut self) -> Result<T, XsdError>
  where
    <T as FromStr>::Err: ToString,
  {
    match self.try_get_content()? {
      Some(output) => Ok(output),
      None => Err(XsdError::XsdParseError(format!(
        "no text found in {}",
        self.element.name
      ))),
    }
  }

  fn get_content_default<T: Default + FromStr>(&mut self) -> Result<T, XsdError>
  where
    <T as FromStr>::Err: ToString,
  {
    match self.try_get_content()? {
      Some(output) => Ok(output),
      None => Ok(T::default()),
    }
  }

  pub fn finalize(
    self,
    allow_extra_attributes: bool,
    allow_extra_children: bool,
  ) -> Result<(), XsdError> {
    let child_errs = self
      .element
      .children
      .into_iter()
      .filter_map(|v| {
        if let XMLNode::Element(node) = v {
          Some(node)
        } else {
          None
        }
      })
      .map(|e| e.name)
      .collect::<Vec<_>>()
      .join(", ");
    let attr_errs = self
      .element
      .attributes
      .into_iter()
      .map(|v| v.0)
      .collect::<Vec<_>>()
      .join(", ");

    if (child_errs.is_empty() || allow_extra_children)
      && (attr_errs.is_empty() || allow_extra_attributes)
    {
      Ok(())
    } else {
      let mut text = String::new();
      text.push_str(&format!("Unused nodes found in {}; ", self.element.name));

      let mut include_space = false;
      if !child_errs.is_empty() && !allow_extra_children {
        text.push_str(&format!("[extra children] {}", child_errs));
        include_space = true;
      }

      if !attr_errs.is_empty() && !allow_extra_attributes {
        if include_space {
          text.push_str("; ")
        }
        text.push_str(&format!("[extra attributes] {}", attr_errs));
      }
      Err(XsdError::XsdParseError(text))
    }
  }
}

#[derive(Clone, Debug)]
pub struct Xsd {
  context: XsdContext,
  schema: schema::Schema,
}

impl Xsd {
  pub fn new(content: &str) -> Result<Self, XsdError> {
    let mut context = XsdContext::new(content)?;
    let schema = schema::Schema::parse(XMLElementWrapper {
      element: xmltree::Element::parse(content.as_bytes())?,
      default_namespace: None,
    })?;

    context.namespace.put(NS_XML_PREFIX, NS_XML_URI);

    for (key, value) in &schema.extra {
      if let Some((lhs, rhs)) = key.split_once(':') {
        if lhs == "xmlns" {
          context.namespace.put(value.to_string(), rhs.to_string());
        }
      }
    }

    Ok(Xsd { context, schema })
  }

  pub fn new_from_file(source: &str) -> Result<Self, XsdError> {
    let content = if source.starts_with("http://") || source.starts_with("https://") {
      tracing::info!("Load HTTP schema {}", source);
      reqwest::blocking::get(source)?.text()?
    } else {
      let path = std::env::current_dir().unwrap();
      tracing::info!("The current directory is {}", path.display());

      fs::read_to_string(source)?
    };

    // skip BOM header, can be present on some files
    let content = if content.as_bytes()[0..3] == [0xef, 0xbb, 0xbf] {
      content[3..].to_owned()
    } else {
      content
    };

    Xsd::new(&content)
  }

  pub fn generate(&mut self, _target_prefix: &Option<String>) -> Result<String, XsdError> {
    self.schema.generate(&mut self.context)
  }
}
