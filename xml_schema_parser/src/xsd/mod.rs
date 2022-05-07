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
use xsd_codegen::{xsdgen_impl, Block, Field, TupleField, XMLElement};
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

  let mut name_used = false;
  match &generated_impl.element {
    xsd_context::XsdImplType::Struct(ty) => {
      name_used = true;
      block = match &ty.fields {
        xsd_codegen::Fields::Empty => block
          .push_block(
            Block::new("if let Some(name) = name").push_block(
              Block::new("match gen_state.state")
                .push_block(
                  Block::new("GenType::Attribute =>")
                    .line("element.get_attribute::<String>(name)?;".to_string()),
                )
                .push_block(
                  Block::new("GenType::Content =>").line("element.get_child(name)?;".to_string()),
                ),
            ),
          )
          .line("Ok(Self)"),
        xsd_codegen::Fields::Tuple(fields) => {
          name_used = true;
          let mut inner_name_used = false;
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
              if field.xml_name.is_none() {
                inner_name_used = true;
              }
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
          let mut self_gen = self_gen.line("))").after(";");

          if !inner_name_used {
            self_gen.before =
              Some("let gen_self = |element: &mut XMLElement, _name: Option<&str>|".to_string())
          }

          block
            .push_block(self_gen)
            .push_block(
              Block::new("if let (Some(name), GenType::Content) = (name, gen_state.state)").line(
                "element.get_next_child_with(name, |mut element| gen_self(&mut element, None))",
              ),
            )
            .push_block(Block::new("else").line("gen_self(element, name)"))
        }
        xsd_codegen::Fields::Named(fields) => {
          name_used = true;
          let mut inner_name_used = false;
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
              if field.xml_name.is_none() {
                inner_name_used = true;
              }
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
          let mut self_gen = self_gen.push_block(inner_block.after(")")).after(";");

          if !inner_name_used {
            self_gen.before =
              Some("let gen_self = |element: &mut XMLElement, _name: Option<&str>|".to_string())
          }

          block
            .push_block(self_gen)
            .push_block(
              Block::new("if let (Some(name), GenType::Content) = (name, gen_state.state)").line(
                "element.get_next_child_with(name, |mut element| gen_self(&mut element, None))",
              ),
            )
            .push_block(Block::new("else").line("gen_self(element, name)"))
        }
      }
    }
    xsd_context::XsdImplType::Enum(r#enum) => {
      for (variant_index, variant) in r#enum.variants.iter().enumerate() {
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
            let mut current_block =
              Block::new("").line("let mut variant_element = element.clone();");

            let mut field_blocks = vec![];
            for (
              field_index,
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
              } else if (field_index == (fields.len() - 1))
                && (variant_index == (r#enum.variants.len() - 1))
              {
                "gen_state"
              } else {
                "gen_state.clone()"
              };

              let next_xml_name = if *flatten {
                "None".to_string()
              } else {
                if field.xml_name.is_none() {
                  name_used = true;
                }
                field
                  .xml_name
                  .as_ref()
                  .map(|v| format!("Some(\"{}\")", v))
                  .unwrap_or_else(|| "name".to_string())
              };

              current_block = current_block.line(format!(
                "let attempt_{field_index} = <{} as XsdGen>::gen(&mut variant_element, {new_gen_state}, {next_xml_name});",
                field.to_string(),
              ));

              field_blocks.push(current_block);

              current_block = Block::new(&format!(
                "if let Ok(attempt_{field_index}) = attempt_{field_index}"
              ));
            }

            let all_fields = (0..fields.len())
              .map(|v| format!("attempt_{v}"))
              .collect::<Vec<_>>()
              .join(", ");
            field_blocks.push(
              current_block
                .line("*element = variant_element;")
                .line(&format!("return Ok(Self::{}({all_fields}));", variant.name)),
            );

            block.push_block(
              field_blocks
                .into_iter()
                .reduce(|current, v| current.push_block(v))
                .unwrap(),
            )
          }
          xsd_codegen::Fields::Named(fields) => {
            let mut current_block =
              Block::new("").line("let mut variant_element = element.clone();");

            let mut field_blocks = vec![];
            for (
              field_index,
              Field {
                name,
                ty,
                xml_name,
                attribute,
                flatten,
                ..
              },
            ) in fields.iter().enumerate()
            {
              let new_gen_state = if *attribute {
                "gen_state.to_attr()"
              } else if (field_index == (fields.len() - 1))
                && (variant_index == (r#enum.variants.len() - 1))
              {
                "gen_state"
              } else {
                "gen_state.clone()"
              };

              let next_xml_name = if *flatten {
                "None".to_string()
              } else {
                if xml_name.is_none() {
                  name_used = true;
                }
                xml_name
                  .as_ref()
                  .map(|v| format!("Some(\"{}\")", v))
                  .unwrap_or_else(|| "name".to_string())
              };

              current_block = current_block.line(format!(
                "let attempt_{name} = <{} as XsdGen>::gen(&mut variant_element, {new_gen_state}, {next_xml_name});",
                ty.to_string(),
              ));

              field_blocks.push((name, current_block));

              current_block = Block::new(&format!("if let Ok(attempt_{name}) = attempt_{name}"));
            }

            let all_fields = field_blocks
              .iter()
              .map(|v| format!("{0}: attempt_{0}", v.0))
              .fold(
                Block::new(&format!("return Ok(Self::{}", variant.name)),
                |current, v| current.line(format!("{v},")),
              )
              .after(");");
            let mut field_blocks = field_blocks.into_iter().map(|v| v.1).collect::<Vec<_>>();
            field_blocks.push(
              current_block
                .line("*element = variant_element;")
                .push_block(all_fields),
            );

            block.push_block(
              field_blocks
                .into_iter()
                .reduce(|current, v| current.push_block(v))
                .unwrap(),
            )
          }
        }
      }
      block = block.line("Err(XsdGenError { ty: XsdType::Unknown, node_name: element.name().to_string(), msg: \"No valid values could be parsed.\".to_string() }.into())")
    }
    _ => {
      generated_new_impl = false;
    }
  };

  if generated_new_impl {
    generated_impl.implementation.push(xsdgen_impl(
      generated_impl.element.get_type(),
      block,
      false,
      name_used,
    ));
  }

  generated_impl
}
