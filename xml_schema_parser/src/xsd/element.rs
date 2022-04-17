use xsd_codegen::{Block, Function, Impl, Struct, XMLElement};
use xsd_types::{XsdGenError, XsdName, XsdParseError, XsdType};

use crate::xsd::{
  annotation::Annotation,
  complex_type::ComplexType,
  max_occurences::MaxOccurences,
  simple_type::SimpleType,
  xsd_context::{XsdElement, XsdImpl},
  XsdContext, XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Element {
  pub name: Option<XsdName>,
  pub kind: Option<XsdName>,
  pub refers: Option<XsdName>,
  pub min_occurences: u64,
  pub r#final: Option<String>,
  pub block: Option<String>,

  pub max_occurences: MaxOccurences,
  pub complex_type: Option<ComplexType>,
  pub simple_type: Option<SimpleType>,
  pub annotation: Option<Annotation>,
  // pub uniques: Vec<String>,
  // pub keys: Vec<String>,
  // pub keyrefs: Vec<String>,
}

impl Element {
  pub fn parse(mut element: XMLElement, parent_is_schema: bool) -> Result<Self, XsdParseError> {
    element.check_name("element")?;

    let name = element
      .try_get_attribute("name")?
      .map(|v: String| element.new_name(&v, XsdType::Element));
    let refers = element
      .try_get_attribute("ref")?
      .map(|v: String| XsdName::new(&v, XsdType::Element));

    if parent_is_schema && name.is_none() {
      return Err(XsdParseError {
        node_name: element.node_name(),
        msg: "name attribute cannot be absent when parent is the schema tag.".to_string(),
      });
    } else if parent_is_schema && refers.is_some() {
      return Err(XsdParseError {
        node_name: element.node_name(),
        msg: format!(
          "ref attribute ({}) cannot be present when parent is the schema tag.",
          refers.unwrap()
        ),
      });
    }

    let complex_type = element.try_get_child_with("complexType", ComplexType::parse)?;
    let simple_type =
      element.try_get_child_with("simpleType", |child| SimpleType::parse(child, false))?;

    if simple_type.is_some() && complex_type.is_some() {
      return Err(XsdParseError {
        node_name: element.node_name(),
        msg: format!("simpleType | complexType cannot both present",),
      });
    }

    let annotation = element.try_get_child_with("annotation", Annotation::parse)?;

    let output = Ok(Self {
      name,
      kind: element
        .try_get_attribute("type")?
        .map(|v: String| XsdName::new(&v, XsdType::SimpleType)),
      refers,
      r#final: element.try_get_attribute("final")?,
      block: element.try_get_attribute("block")?,
      min_occurences: element.try_get_attribute("minOccurs")?.unwrap_or(1),
      max_occurences: element.get_attribute_default("maxOccurs")?,
      complex_type,
      simple_type,
      annotation,
    });

    element.finalize(false, false)?;

    output
  }

  fn is_multiple(&self) -> bool {
    (match &self.max_occurences {
      MaxOccurences::Unbounded => true,
      MaxOccurences::Number { value } => *value > 1,
    }) || self.min_occurences > 1
  }

  fn could_be_none(&self) -> bool {
    (match &self.max_occurences {
      MaxOccurences::Unbounded => false,
      MaxOccurences::Number { value } => *value == 1,
    }) && self.min_occurences == 0
  }

  #[tracing::instrument(skip_all)]
  pub fn get_implementation(&self, context: &mut XsdContext) -> Result<XsdImpl, XsdError> {
    // We either have a named (such as a schema decl) or an anonymous element.
    let xml_name = self.name.clone().unwrap();

    // Now we will generate and return a struct which contains the data declared in the element.
    // TODO(drosen): Simplify output if element is trivial (e.g. simpleType).

    let mut generated_struct = match (&self.simple_type, &self.complex_type) {
      (None, Some(complex_type)) => {
        complex_type.get_implementation(false, Some(xml_name.clone()), context)?
      }
      (Some(simple_type), None) => {
        simple_type.get_implementation(Some(xml_name.clone()), context)?
      }
      (None, None) => {
        if self.kind.is_none() {
          return Ok(XsdImpl {
            name: xml_name.clone(),
            fieldname_hint: Some(xml_name.to_field_name()),
            element: XsdElement::Struct(
              Struct::new(&xml_name.to_struct_name())
                .vis("pub")
                .to_owned(),
            ),
            inner: vec![],
            implementation: vec![],
          });
        } else {
          let imp = context.multi_search(
            self.kind.as_ref().unwrap().namespace.clone(),
            self.kind.as_ref().unwrap().local_name.clone(),
            &[XsdType::SimpleType, XsdType::ComplexType],
          );
          match imp {
            super::xsd_context::SearchResult::SingleMatch(imp) => XsdImpl {
              name: xml_name.clone(),
              fieldname_hint: Some(xml_name.to_field_name()),
              element: XsdElement::Type(imp.element.get_type()),
              inner: vec![],
              implementation: vec![],
            },
            super::xsd_context::SearchResult::MultipleMatches => {
              return Err(XsdError::XsdGenError(XsdGenError {
                node_name: xml_name.to_string(),
                ty: XsdType::Element,
                msg: format!(
                  "Found both a simple and complex type named {}",
                  self.kind.as_ref().unwrap()
                ),
              }));
            }
            super::xsd_context::SearchResult::NoMatches => {
              return Err(XsdError::XsdImplNotFound(xml_name.clone()));
            }
          }
        }
      }
      _ => {
        return Err(XsdError::XsdGenError(XsdGenError {
          node_name: xml_name.to_string(),
          ty: XsdType::Element,
          msg: "Found both simple and complex type in element.".to_string(),
        }))
      }
    };

    let field_name = xml_name.to_field_name();
    let field_type = generated_struct.element.get_type();

    let docs = self
      .annotation
      .as_ref()
      .map(|annotation| annotation.get_doc());
    if let Some(docs) = docs {
      generated_struct.element.add_doc(&docs.join(""));
    }

    let mut generated_struct = if self.is_multiple() || self.could_be_none() {
      let field_type = if self.is_multiple() {
        field_type.wrap("Vec")
      } else if self.could_be_none() {
        field_type.wrap("Option")
      } else {
        field_type
      };

      let mut output_struct = XsdImpl {
        name: xml_name.clone(),
        fieldname_hint: Some(field_name.clone()),
        element: XsdElement::Type(field_type).to_owned(),
        inner: vec![],
        implementation: vec![],
      };

      let mut r#impl = Impl::new(output_struct.element.get_type())
        .impl_trait("XsdParse")
        .to_owned();

      let mut parse = Function::new("parse")
        .arg("element", "&mut XMLElementWrapper")
        .ret("Result<Self, XsdError>");

      let output = Block::new("let output = Self").after(";").to_owned();

      let output = if self.is_multiple() {
        output.line(&format!(
          "{field_name}: element.try_get_children_with({xml_name}, |v| XsdParse::parse(v))?,"
        ))
      } else if self.could_be_none() {
        output.line(&format!(
          "{field_name}: element.try_get_child_with({xml_name}, |v| XsdParse::parse(v))?,"
        ))
      } else {
        output.line(&format!("{field_name}: XsdParse::parse(element)?,"))
      };

      parse = parse.push_block(output).line("Ok(output)");
      r#impl = r#impl.push_fn(parse);

      match generated_struct.element {
        XsdElement::Struct(_) | XsdElement::Enum(_) => output_struct.inner.push(generated_struct),
        _ => {}
      }

      output_struct.implementation.push(r#impl);

      output_struct
    } else {
      generated_struct
    };

    generated_struct.name.ty = XsdType::Element;

    Ok(generated_struct)
  }
}
