use xsd_codegen::{Struct, XMLElement};
use xsd_types::{XsdGenError, XsdIoError, XsdName, XsdParseError, XsdType};

use crate::xsd::{
  annotation::Annotation,
  complex_type::ComplexType,
  max_occurences::MaxOccurences,
  simple_type::SimpleType,
  xsd_context::{XsdImpl, XsdImplType},
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
  pub fn parse(mut element: XMLElement, parent_is_schema: bool) -> Result<Self, XsdIoError> {
    element.check_name("element")?;

    let name = element
      .try_get_attribute("name")?
      .map(|v: String| element.new_name(&v, XsdType::Element));
    let refers = element
      .try_get_attribute("ref")?
      .map(|v: String| XsdName::new(&v, XsdType::Element));

    if parent_is_schema && name.is_none() {
      return Err(XsdIoError::XsdParseError(XsdParseError {
        node_name: element.node_name(),
        msg: "name attribute cannot be absent when parent is the schema tag.".to_string(),
      }));
    } else if parent_is_schema && refers.is_some() {
      return Err(XsdIoError::XsdParseError(XsdParseError {
        node_name: element.node_name(),
        msg: format!(
          "ref attribute ({}) cannot be present when parent is the schema tag.",
          refers.unwrap()
        ),
      }));
    }

    let complex_type = element.try_get_child_with("complexType", ComplexType::parse)?;
    let simple_type =
      element.try_get_child_with("simpleType", |child| SimpleType::parse(child, false))?;

    if simple_type.is_some() && complex_type.is_some() {
      return Err(XsdIoError::XsdParseError(XsdParseError {
        node_name: element.node_name(),
        msg: "simpleType | complexType cannot both present".to_string(),
      }));
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
    let xml_name = self.name.clone().unwrap();

    let mut generated_struct = match (&self.simple_type, &self.complex_type, &self.kind) {
      (None, Some(complex_type), None) => {
        complex_type.get_implementation(false, Some(xml_name.clone()), context)?
      }
      (Some(simple_type), None, None) => {
        simple_type.get_implementation(Some(xml_name.clone()), context)?
      }
      (None, None, Some(kind)) => {
        let imp = context.multi_search(
          kind.namespace.clone(),
          kind.local_name.clone(),
          &[XsdType::SimpleType, XsdType::ComplexType],
        );
        match imp {
          super::xsd_context::SearchResult::SingleMatch(imp) => {
            let mut ty = imp.element.get_type();
            ty.xml_name = Some(xml_name.clone());
            XsdImpl {
              name: xml_name.clone(),
              fieldname_hint: Some(xml_name.to_field_name()),
              element: XsdImplType::Type(ty.xml_name(Some(xml_name.clone()))),
              inner: vec![],
              implementation: vec![],
              flatten: false,
            }
          }
          super::xsd_context::SearchResult::MultipleMatches => {
            return Err(XsdError::XsdIoError(XsdIoError::XsdGenError(XsdGenError {
              node_name: xml_name.to_string(),
              ty: XsdType::Element,
              msg: format!(
                "Found both a simple and complex type named {}",
                self.kind.as_ref().unwrap()
              ),
            })));
          }
          super::xsd_context::SearchResult::NoMatches => {
            return Err(XsdError::XsdImplNotFound(xml_name));
          }
        }
      }
      (None, None, None) => {
        return Ok(XsdImpl {
          name: xml_name.clone(),
          fieldname_hint: Some(xml_name.to_field_name()),
          element: XsdImplType::Struct(
            Struct::new(Some(xml_name.clone()), &xml_name.to_struct_name()).vis("pub"),
          ),
          inner: vec![],
          implementation: vec![],
          flatten: false,
        });
      }
      _ => {
        return Err(XsdError::XsdIoError(XsdIoError::XsdGenError(XsdGenError {
          node_name: xml_name.to_string(),
          ty: XsdType::Element,
          msg: "Found both simple and complex type in element.".to_string(),
        })))
      }
    };

    if let Some(annotation) = &self.annotation {
      generated_struct
        .element
        .add_doc(&annotation.get_doc().join("\n"));
    }

    let mut generated_struct = if self.is_multiple() || self.could_be_none() {
      let field_name = xml_name.to_field_name();
      let field_type = generated_struct.element.get_type();

      let field_type = if self.is_multiple() {
        field_type.wrap("Vec")
      } else if self.could_be_none() {
        field_type.wrap("Option")
      } else {
        field_type
      };

      let inner = if let XsdImplType::Struct(_) | XsdImplType::Enum(_) = generated_struct.element {
        vec![generated_struct]
      } else {
        vec![]
      };

      XsdImpl {
        name: xml_name,
        fieldname_hint: Some(field_name),
        element: XsdImplType::Type(field_type),
        inner,
        implementation: vec![],
        flatten: false,
      }
    } else {
      generated_struct
    };

    generated_struct.name.ty = XsdType::Element;

    Ok(generated_struct)
  }
}
