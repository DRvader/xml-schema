use xmltree::{Element, XMLNode};
use xsd_types::{XsdIoError, XsdName, XsdParseError, XsdType};

use crate::FromXmlString;

#[derive(Clone)]
pub struct XMLElement {
  pub element: Element,
  pub default_namespace: Option<String>,
}

impl XMLElement {
  pub fn parse(buffer: &[u8]) -> Result<Self, xmltree::ParseError> {
    Ok(Self {
      element: xmltree::Element::parse(buffer)?,
      default_namespace: None,
    })
  }

  pub fn parse_hack(buffer: &[u8]) -> Result<Self, xmltree::ParseError> {
    let mut element = Self::parse(buffer)?;

    let mut root_element = Element::new("root");
    root_element
      .children
      .push(XMLNode::Element(element.element));
    element.element = root_element;

    Ok(element)
  }

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

  pub fn node_name(&self) -> String {
    self.element.name.to_string()
  }

  pub fn check_name(&self, name: &str) -> Result<(), XsdIoError> {
    if self.element.name != name {
      Err(XsdIoError::XsdParseError(XsdParseError {
        node_name: self.node_name(),
        msg: format!(
          "Unexpected element name {} expected {}",
          name, self.element.name
        ),
      }))
    } else {
      Ok(())
    }
  }

  fn get_children(&mut self, name: &str) -> Vec<XMLElement> {
    let mut output = Vec::new();
    while let Some(child) = self.element.take_child(name) {
      output.push(XMLElement {
        element: child,
        default_namespace: self.default_namespace.clone(),
      });
    }

    output
  }

  fn get_child(&mut self, name: &str) -> Result<XMLElement, XsdIoError> {
    let mut output = self.get_children(name);
    if output.len() != 1 {
      return Err(XsdIoError::XsdParseError(XsdParseError {
        node_name: self.node_name(),
        msg: format!("Expected 1 child named {} found {}", name, output.len(),),
      }));
    }

    Ok(output.remove(0))
  }

  pub fn try_get_child(&mut self, name: &str) -> Result<Option<XMLElement>, XsdIoError> {
    let mut output = self.get_children(name);
    if output.len() > 1 {
      return Err(XsdIoError::XsdParseError(XsdParseError {
        node_name: self.node_name(),
        msg: format!(
          "Expected 0 or 1 children named {} found {}",
          name,
          output.len(),
        ),
      }));
    }

    if output.is_empty() {
      Ok(None)
    } else {
      Ok(Some(output.remove(0)))
    }
  }

  pub fn get_children_with_filter<T>(
    &mut self,
    name: &str,
    func: impl Fn(XMLElement) -> Result<Option<T>, XsdIoError>,
  ) -> Result<Vec<T>, XsdIoError> {
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

  pub fn get_children_with<T>(
    &mut self,
    name: &str,
    func: impl Fn(XMLElement) -> Result<T, XsdIoError>,
  ) -> Result<Vec<T>, XsdIoError> {
    self.get_children_with_filter(name, |child| func(child).map(Some))
  }

  pub fn get_child_with<T>(
    &mut self,
    name: &str,
    func: impl FnOnce(XMLElement) -> Result<T, XsdIoError>,
  ) -> Result<T, XsdIoError> {
    func(self.get_child(name)?)
  }

  pub fn get_all_children(&mut self) -> Vec<XMLElement> {
    let mut output = Vec::new();

    let mut to_remove = Vec::new();
    for (index, child) in self.element.children.iter().enumerate() {
      if let XMLNode::Element(_) = child {
        to_remove.push(index);
      }
    }
    to_remove.reverse();

    for index in to_remove {
      if let XMLNode::Element(element) = self.element.children.remove(index) {
        output.push(XMLElement {
          element,
          default_namespace: self.default_namespace.clone(),
        });
      }
    }

    output.reverse();

    output
  }

  pub fn try_get_child_with<T>(
    &mut self,
    name: &str,
    func: impl FnOnce(XMLElement) -> Result<T, XsdIoError>,
  ) -> Result<Option<T>, XsdIoError> {
    if let Some(child) = self.try_get_child(name)? {
      Ok(Some(func(child)?))
    } else {
      Ok(None)
    }
  }

  pub fn try_get_attribute<T: FromXmlString>(
    &mut self,
    name: &str,
  ) -> Result<Option<T>, XsdIoError> {
    let value = self.element.attributes.remove(name);
    if let Some(value) = value {
      Ok(Some(T::from_xml(&value).map_err(|e| XsdParseError {
        node_name: self.node_name(),
        msg: format!("error converting {} from text: {}", name, e.to_string()),
      })?))
    } else {
      Ok(None)
    }
  }

  pub fn get_attribute<T: FromXmlString>(&mut self, name: &str) -> Result<T, XsdIoError> {
    match self.try_get_attribute(name)? {
      Some(output) => Ok(output),
      None => Err(XsdIoError::XsdParseError(XsdParseError {
        node_name: self.node_name(),
        msg: format!("{} not found", name),
      })),
    }
  }

  pub fn get_attribute_default<T: Default + FromXmlString>(
    &mut self,
    name: &str,
  ) -> Result<T, XsdIoError> {
    match self.try_get_attribute(name)? {
      Some(output) => Ok(output),
      None => Ok(T::default()),
    }
  }

  pub fn get_remaining_attributes(&mut self) -> Vec<(String, String)> {
    self.element.attributes.drain().collect()
  }

  pub fn try_get_content<T: FromXmlString>(&mut self) -> Result<Option<T>, XsdIoError> {
    let value = self.element.get_text();
    if let Some(value) = value {
      Ok(Some(T::from_xml(&value).map_err(|e| XsdParseError {
        node_name: self.node_name(),
        msg: format!("could not parse node content from text: {}", e.to_string()),
      })?))
    } else {
      Ok(None)
    }
  }

  pub fn get_content<T: FromXmlString>(&mut self) -> Result<T, XsdIoError> {
    match self.try_get_content()? {
      Some(output) => Ok(output),
      None => Err(XsdIoError::XsdParseError(XsdParseError {
        node_name: self.node_name(),
        msg: format!("no text found"),
      })),
    }
  }

  fn get_content_default<T: Default + FromXmlString>(&mut self) -> Result<T, XsdIoError> {
    match self.try_get_content()? {
      Some(output) => Ok(output),
      None => Ok(T::default()),
    }
  }

  pub fn finalize(
    self,
    allow_extra_attributes: bool,
    allow_extra_children: bool,
  ) -> Result<(), XsdIoError> {
    let child_errs = self
      .element
      .children
      .iter()
      .filter_map(|v| {
        if let XMLNode::Element(node) = v {
          Some(node)
        } else {
          None
        }
      })
      .map(|e| e.name.as_str())
      .collect::<Vec<_>>()
      .join(", ");
    let attr_errs = self
      .element
      .attributes
      .iter()
      .map(|v| v.0.as_str())
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
      Err(XsdIoError::XsdParseError(XsdParseError {
        node_name: self.node_name(),
        msg: text,
      }))
    }
  }
}
