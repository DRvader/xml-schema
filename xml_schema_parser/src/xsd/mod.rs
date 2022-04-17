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
use thiserror::Error;
use xml::namespace::{NS_XML_PREFIX, NS_XML_URI};
use xsd_codegen::XMLElement;
use xsd_context::XsdContext;
use xsd_types::{XsdGenError, XsdName, XsdParseError};

#[derive(Error, Debug)]
pub enum XsdError {
  #[error("{0} not found")]
  XsdImplNotFound(XsdName),
  #[error(transparent)]
  XsdParseError(#[from] XsdParseError),
  #[error(transparent)]
  XsdGenError(#[from] XsdGenError),
  #[error(transparent)]
  XmlParseError(#[from] xmltree::ParseError),
  #[error("{0}")]
  XsdMissing(String),
  #[error("When searching for {name}: {msg}")]
  ContextSearchError { name: XsdName, msg: String },
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[error("Unknown Xsd error")]
  Unknown,
  #[error(transparent)]
  NetworkError(#[from] reqwest::Error),
  #[error(transparent)]
  Infalible(#[from] std::convert::Infallible),
}

#[derive(Clone, Debug)]
pub struct Xsd {
  context: XsdContext,
  schema: schema::Schema,
}

impl Xsd {
  pub fn new(content: &str) -> Result<Self, XsdError> {
    let mut context = XsdContext::new(content)?;
    let schema = schema::Schema::parse(XMLElement {
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
