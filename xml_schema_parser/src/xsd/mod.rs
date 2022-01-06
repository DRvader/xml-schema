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
mod rust_types_mapping;
mod schema;
mod sequence;
mod simple_content;
mod simple_type;
mod union;
mod xsd_context;

use log::info;
use std::collections::BTreeMap;
use std::fs;
use std::str::FromStr;
use thiserror::Error;
use xmltree::{Element, XMLNode};
use xsd_context::XsdContext;

use self::xsd_context::XsdName;

#[derive(Error, Debug)]
pub enum XsdError {
  #[error("{0} not found")]
  XsdImplNotFound(XsdName),
  #[error("{0}")]
  XsdParseError(String),
  #[error(transparent)]
  XmlParseError(#[from] xmltree::ParseError),
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[error("Unknown Xsd error")]
  Unknown,
  #[error(transparent)]
  NetworkError(#[from] reqwest::Error),
  #[error(transparent)]
  Infalible(#[from] std::convert::Infallible),
}

pub struct XMLElementWrapper(Element);

impl XMLElementWrapper {
  fn name(&self) -> &str {
    &self.0.name
  }

  fn check_name(&self, name: &str) -> Result<(), XsdError> {
    if self.0.name != name {
      Err(XsdError::XsdParseError(format!(
        "Unexpected element name {} expected {}",
        name, self.0.name
      )))
    } else {
      Ok(())
    }
  }

  fn get_children(&mut self, name: &str) -> Vec<XMLElementWrapper> {
    let mut output = Vec::new();
    while let Some(child) = self.0.take_child(name) {
      output.push(XMLElementWrapper(child));
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

  fn get_children_with<T>(
    &mut self,
    name: &str,
    func: impl Fn(XMLElementWrapper) -> Result<T, XsdError>,
  ) -> Result<Vec<T>, XsdError> {
    self.get_children_with_filter(name, |child| func(child).map(|v| Some(v)))
  }

  fn get_child_with<T>(
    &mut self,
    name: &str,
    func: impl FnOnce(XMLElementWrapper) -> Result<T, XsdError>,
  ) -> Result<T, XsdError> {
    func(self.get_child(name)?)
  }

  fn try_get_child_with<T>(
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

  fn try_get_attribute<T: FromStr>(&mut self, name: &str) -> Result<Option<T>, XsdError>
  where
    <T as FromStr>::Err: ToString,
  {
    let value = self.0.attributes.remove(name);
    if let Some(value) = value {
      return Ok(Some(value.parse::<T>().map_err(|e| {
        XsdError::XsdParseError(format!(
          "Error parsing {} in {}: {}",
          name,
          self.0.name,
          e.to_string()
        ))
      })?));
    } else {
      Ok(None)
    }
  }

  fn get_attribute<T: FromStr>(&mut self, name: &str) -> Result<T, XsdError>
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
    self.0.attributes.drain().collect()
  }

  fn try_get_content<T: FromStr>(&mut self) -> Result<Option<T>, XsdError>
  where
    <T as FromStr>::Err: ToString,
  {
    let value = self.0.get_text();
    if let Some(value) = value {
      return Ok(Some(value.parse::<T>().map_err(|e| {
        XsdError::XsdParseError(format!(
          "Error parsing node text in {}: {}",
          self.0.name,
          e.to_string()
        ))
      })?));
    } else {
      Ok(None)
    }
  }

  fn get_content<T: FromStr>(&mut self) -> Result<T, XsdError>
  where
    <T as FromStr>::Err: ToString,
  {
    match self.try_get_content()? {
      Some(output) => Ok(output),
      None => Err(XsdError::XsdParseError(format!(
        "no text found in {}",
        self.0.name
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

  fn finalize(
    self,
    allow_extra_attributes: bool,
    allow_extra_children: bool,
  ) -> Result<(), XsdError> {
    let child_errs = self
      .0
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
      .0
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
      text.push_str(&format!("Unused nodes found in {}; ", self.0.name));

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
  pub fn new(
    content: &str,
    module_namespace_mappings: &BTreeMap<String, String>,
  ) -> Result<Self, XsdError> {
    let context = XsdContext::new(content)?;
    let context = context.with_module_namespace_mappings(module_namespace_mappings);
    let schema = schema::Schema::parse(XMLElementWrapper(xmltree::Element::parse(
      content.as_bytes(),
    )?))?;

    Ok(Xsd { context, schema })
  }

  pub fn new_from_file(
    source: &str,
    module_namespace_mappings: &BTreeMap<String, String>,
  ) -> Result<Self, XsdError> {
    let content = if source.starts_with("http://") || source.starts_with("https://") {
      info!("Load HTTP schema {}", source);
      reqwest::blocking::get(source)?.text()?
    } else {
      let path = std::env::current_dir().unwrap();
      info!("The current directory is {}", path.display());

      fs::read_to_string(source)?
    };

    // skip BOM header, can be present on some files
    let content = if content.as_bytes()[0..3] == [0xef, 0xbb, 0xbf] {
      content[3..].to_owned()
    } else {
      content
    };

    Xsd::new(&content, module_namespace_mappings)
  }

  pub fn generate(&mut self, target_prefix: &Option<String>) -> Result<String, XsdError> {
    self.schema.generate(&mut self.context)
  }
}

#[cfg(test)]
mod test {
  use std::collections::BTreeMap;

  use super::{Xsd, XsdError};

  #[test]
  fn musicxml() -> Result<(), XsdError> {
    let mut xsd = Xsd::new_from_file("../musicxml.xsd", &BTreeMap::new())?;
    let output = xsd.generate(&None)?;

    dbg!(output);

    Ok(())
  }
}
