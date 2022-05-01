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
use xsd_codegen::{xsdgen_impl, Block, TupleField, XMLElement};
use xsd_context::XsdContext;
use xsd_types::{XsdIoError, XsdName};

use self::xsd_context::XsdImpl;

#[derive(Error, Debug)]
pub enum XsdError {
  #[error("{0} not found")]
  XsdImplNotFound(XsdName),
  #[error(transparent)]
  XsdIoError(#[from] XsdIoError),
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

fn general_xsdgen(mut generated_impl: XsdImpl) -> XsdImpl {
  let mut block = Block::new("");
  let mut generated_new_impl = true;
  match &generated_impl.element {
    xsd_context::XsdImplType::Struct(ty) => {
      block = match &ty.fields {
        xsd_codegen::Fields::Empty => block
          .push_block(
            Block::new("match gen_state.state")
              .push_block(Block::new("GenType::Attribute =>").line(format!(
                "assert!(element.element.attributes.remove(\"{}\").is_some());",
                ty.ty().xml_name.clone().unwrap()
              )))
              .push_block(Block::new("GenType::Content =>").line(format!(
                "assert!(element.try_get_child(\"{}\")?.is_some());",
                ty.ty().xml_name.clone().unwrap()
              ))),
          )
          .line("Ok(Self)"),
        xsd_codegen::Fields::Tuple(fields) => {
          let mut self_gen =
            Block::new("let gen_self = |element: &mut XMLElement, name: Option<&str>|");
          self_gen = self_gen.line("Ok(Self (");
          for TupleField {
            ty: field,
            attribute,
            flatten,
            ..
          } in fields
          {
            let new_gen_state = if *attribute {
              "gen_state.to_attr()"
            } else {
              "gen_state.clone()"
            };

            let next_xml_name = if *flatten {
              "None".to_string()
            } else {
              field
                .xml_name
                .as_ref()
                .map(|v| format!("Some(\"{}\")", v))
                .unwrap_or_else(|| "name".to_string())
            };

            self_gen = self_gen.line(format!(
              "<{} as XsdGen>::gen(element, {new_gen_state}, {next_xml_name})?,",
              field.to_string(),
            ));
          }
          let self_gen = self_gen.line("))").after(";");

          block
            .push_block(self_gen)
            .push_block(
              Block::new("if let (Some(name), GenType::Content) = (name, gen_state.state)")
                .line("element.get_child_with(name, |mut element| gen_self(&mut element, None))"),
            )
            .push_block(Block::new("else").line("gen_self(element, name)"))
        }
        xsd_codegen::Fields::Named(fields) => {
          let self_gen =
            Block::new("let gen_self = |element: &mut XMLElement, name: Option<&str>|");
          let mut inner_block = Block::new("Ok(Self");
          for field in fields {
            let new_gen_state = if field.attribute {
              "gen_state.to_attr()"
            } else {
              "gen_state.clone()"
            };

            let next_xml_name = if field.flatten {
              "None".to_string()
            } else {
              field
                .xml_name
                .as_ref()
                .map(|v| format!("Some(\"{}\")", v))
                .unwrap_or_else(|| "name".to_string())
            };

            inner_block = inner_block.line(format!(
              "{}: <{} as XsdGen>::gen(element, {new_gen_state}, {next_xml_name})?,",
              field.name,
              field.ty.to_string()
            ));
          }
          let self_gen = self_gen.push_block(inner_block.after(")")).after(";");

          block
            .push_block(self_gen)
            .push_block(
              Block::new("if let (Some(name), GenType::Content) = (name, gen_state.state)")
                .line("element.get_child_with(name, |mut element| gen_self(&mut element, None))"),
            )
            .push_block(Block::new("else").line("gen_self(element, name)"))
        }
      }
    }
    xsd_context::XsdImplType::Enum(ty) => {
      let mut variant_resolution_results = vec![];
      for (variant_index, variant) in ty.variants.iter().enumerate() {
        block = match &variant.fields {
          xsd_codegen::Fields::Empty => block
            .push_block(
              Block::new("match gen_state.state")
                .push_block(Block::new("GenType::Attribute").line(format!(
                  "assert!(element.element.attributes.remove(\"{}\").is_some());",
                  variant.xml_name.clone().unwrap()
                )))
                .push_block(Block::new("GenType::Content").line(format!(
                  "assert!(element.try_get_child(\"{}\")?.is_some());",
                  variant.xml_name.clone().unwrap()
                ))),
            )
            .line(format!("Ok(Self::{})", &variant.name)),
          xsd_codegen::Fields::Tuple(fields) => {
            for (
              index,
              TupleField {
                ty: field,
                attribute,
                flatten,
                ..
              },
            ) in fields.iter().enumerate()
            {
              let new_gen_state = if *attribute {
                "gen_state.to_attr()"
              } else {
                "gen_state.clone()"
              };

              let next_xml_name = if *flatten {
                "None".to_string()
              } else {
                field
                  .xml_name
                  .as_ref()
                  .map(|v| format!("Some(\"{}\")", v))
                  .unwrap_or_else(|| "name".to_string())
              };

              let variant_res_name = format!("attempt_{}_{}", variant_index, index);
              block = block.line(format!(
                "let mut {variant_res_name}_element = element.clone();",
              ));
              block = block.line(format!(
                "let {} = <{} as XsdGen>::gen(&mut {variant_res_name}_element, {new_gen_state}, {next_xml_name});",
                variant_res_name,
                field.to_string(),
              ));

              variant_resolution_results.push((variant_res_name, variant.name.clone()));
            }
            block
          }
          xsd_codegen::Fields::Named(fields) => {
            let mut inner_block = Block::new(&format!("Ok(Self::{}", variant.name));
            for field in fields {
              let new_gen_state = if field.attribute {
                "gen_state.to_attr()"
              } else {
                "gen_state.clone()"
              };

              let next_xml_name = if field.flatten {
                "None".to_string()
              } else {
                field
                  .xml_name
                  .as_ref()
                  .map(|v| format!("Some(\"{}\")", v))
                  .unwrap_or_else(|| "name".to_string())
              };

              block = block.line(format!("let mut {}_element = element.clone();", field.name));
              inner_block = inner_block.line(format!(
                "{}: <{} as XsdGen>::gen({0}_element, {new_gen_state}, {next_xml_name})?,",
                field.name,
                field.ty.to_string(),
              ));
              variant_resolution_results.push((field.name.clone(), variant.name.clone()));
            }
            block.push_block(inner_block.after(")"))
          }
        }
      }
      let mut match_block = Block::new(&format!(
        "match ({})",
        variant_resolution_results
          .iter()
          .map(|v| v.0.as_str())
          .collect::<Vec<_>>()
          .join(", ")
      ));

      for (index, (attempt_name, variant_name)) in variant_resolution_results.iter().enumerate() {
        match_block = match_block.push_block(
          Block::new(&format!(
            "({}) =>",
            (0..variant_resolution_results.len())
              .map(|i| if i == index { "Ok(value)" } else { "Err(_)" })
              .collect::<Vec<_>>()
              .join(", ")
          ))
          .line(format!("*element = {attempt_name}_element;"))
          .line(format!("Ok(Self::{variant_name}(value))")),
        );
      }
      block = block.push_block(match_block.push_block(Block::new(&format!(
            "({}) =>",
            (0..variant_resolution_results.len())
              .map(|_| "Err(_)")
              .collect::<Vec<_>>()
              .join(", ")))
            .line("Err(XsdGenError { ty: XsdType::Unknown, node_name: element.name().to_string(), msg: format!(\"No valid values could be parsed.\") })?")
          ).line("_ => { Err(XsdGenError { ty: XsdType::Unknown, node_name: element.name().to_string(), msg: format!(\"Multiple values were able to be parsed.\") })? }")
        );
    }
    _ => {
      generated_new_impl = false;
    }
  }

  if generated_new_impl {
    generated_impl
      .implementation
      .push(xsdgen_impl(generated_impl.element.get_type(), block));
  }

  generated_impl
}
