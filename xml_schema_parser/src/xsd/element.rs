use crate::{
  codegen::{Field, Struct},
  xsd::{
    annotation::Annotation,
    complex_type::ComplexType,
    max_occurences::MaxOccurences,
    simple_type::SimpleType,
    xsd_context::{XsdElement, XsdImpl, XsdName},
    XsdContext, XsdError,
  },
};

use super::{
  xsd_context::{to_field_name, to_struct_name},
  XMLElementWrapper,
};

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct Element {
  pub name: Option<String>,
  pub kind: Option<String>,
  pub refers: Option<String>,
  pub min_occurences: u64,
  pub r#final: Option<String>,
  pub block: Option<String>,

  pub max_occurences: MaxOccurences,
  pub complex_type: Option<ComplexType>,
  pub simple_type: Option<SimpleType>,
  pub annotation: Option<Annotation>,
  // #[yaserde(rename = "unique")]
  // pub uniques: Vec<String>,
  // #[yaserde(rename = "key")]
  // pub keys: Vec<String>,
  // #[yaserde(rename = "keyref")]
  // pub keyrefs: Vec<String>,
}

impl Element {
  pub fn parse(mut element: XMLElementWrapper, parent_is_schema: bool) -> Result<Self, XsdError> {
    element.check_name("element")?;

    let name = element.try_get_attribute("name")?;
    let refers = element.try_get_attribute("ref")?;

    if parent_is_schema && name.is_none() {
      return Err(XsdError::XsdParseError(
        "name attribute cannot be absent when parent is the schema tag.".to_string(),
      ));
    } else if parent_is_schema && refers.is_some() {
      return Err(XsdError::XsdParseError(format!(
        "ref attribute ({}) cannot be present when parent is the schema tag.",
        refers.unwrap()
      )));
    }

    let complex_type = element.try_get_child_with("complexType", ComplexType::parse)?;
    let simple_type =
      element.try_get_child_with("simpleType", |child| SimpleType::parse(child, false))?;

    if simple_type.is_some() && complex_type.is_some() {
      return Err(XsdError::XsdParseError(format!(
        "simpleType | complexType cannot both present in {}",
        element.name()
      )));
    }

    let annotation = element.try_get_child_with("annotation", Annotation::parse)?;

    let output = Ok(Self {
      name,
      kind: element.try_get_attribute("type")?,
      refers,
      r#final: element.try_get_attribute("final")?,
      block: element.try_get_attribute("block")?,
      min_occurences: element.try_get_attribute("minOccurs")?.unwrap_or(1),
      max_occurences: element
        .try_get_attribute("maxOccurs")?
        .unwrap_or(MaxOccurences::Number { value: 1 }),
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
      MaxOccurences::Number { value } => *value > 0,
    }) || self.min_occurences > 0
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
    let xml_name = self.name.clone().unwrap_or_else(|| "anon".to_string());
    let type_name = to_struct_name(&xml_name);

    // Now we will generate and return a struct which contains the data declared in the element.
    // TODO(drosen): Simplify output if element is trivial (e.g. simpleType).

    let mut ty_impl = match (&self.simple_type, &self.complex_type) {
      (None, Some(complex_type)) => complex_type.get_implementation(context)?,
      (Some(simple_type), None) => simple_type.get_implementation(context)?,
      (None, None) => {
        if self.kind.is_none() {
          return Ok(XsdImpl {
            name: XsdName::new(&xml_name),
            fieldname_hint: Some(to_field_name(&xml_name)),
            element: XsdElement::Struct(Struct::new(&to_struct_name(&xml_name))),
            inner: vec![],
            implementation: vec![],
          });
        } else if let Some(imp) = context
          .structs
          .get(&XsdName::new(self.kind.as_ref().unwrap()))
        {
          imp.clone()
        } else {
          return Err(XsdError::XsdImplNotFound(XsdName::new(&xml_name)));
        }
      }
      _ => {
        return Err(XsdError::XsdGenError {
          node_name: xml_name,
          msg: "Found both simple and complex type in element.".to_string(),
        })
      }
    };

    let initial_field_name = to_field_name(&xml_name);

    let generated_struct = if self.is_multiple() || self.could_be_none() {
      let field_name = if self.is_multiple() {
        format!("{}s", initial_field_name)
      } else {
        initial_field_name.clone()
      };

      let mut field_type = ty_impl.element.get_type();

      if self.is_multiple() {
        field_type.wrap("Vec");
      } else if self.could_be_none() {
        field_type.wrap("Option");
      }

      if self.could_be_none() {
        field_type.wrap("Option");
      }

      // TODO(drosen): Gen parse function for this case!

      XsdImpl {
        name: XsdName::new(&xml_name),
        fieldname_hint: Some(initial_field_name),
        element: XsdElement::Struct(
          Struct::new(&type_name)
            .push_field(Field::new(&field_name, field_type).vis("pub").to_owned())
            .to_owned(),
        ),
        inner: vec![],
        implementation: vec![],
      }
    } else {
      let docs = self
        .annotation
        .as_ref()
        .map(|annotation| annotation.get_doc());

      if let Some(docs) = docs {
        match &mut ty_impl.element {
          XsdElement::Struct(str) => {
            str.doc(&docs.as_slice().join(""));
          }
          XsdElement::Enum(en) => {
            en.doc(&docs.as_slice().join(""));
          }
          XsdElement::Field(field) => {
            field.doc(vec![&docs.as_slice().join("")]);
          }
          XsdElement::Type(_) => {
            unreachable!()
          }
        }
      }

      ty_impl
    };

    Ok(generated_struct)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  static DERIVES: &str =
    "# [ derive ( Clone , Debug , Default , PartialEq , YaDeserialize , YaSerialize ) ] ";

  static DOCS: &str = r#"# [ doc = "Loudness measured in Decibels" ] "#;

  #[test]
  fn extern_type() {
    let element = Element {
      name: Some("volume".to_string()),
      kind: Some("books:volume-type".to_string()),
      refers: None,
      min_occurences: 1,
      max_occurences: MaxOccurences::Number { value: 1 },
      complex_type: None,
      simple_type: None,
      annotation: Some(Annotation {
        id: None,
        documentation: vec!["Loudness measured in Decibels".to_string()],
      }),
      ..Default::default()
    };

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    let value = element
      .get_implementation(&mut context)
      .unwrap()
      .to_string()
      .unwrap();
    let ts = quote!(#value).to_string();

    assert_eq!(
      ts,
      format!(
        "{}{}pub struct Volume {{ # [ yaserde ( flatten ) ] pub content : VolumeType , }}",
        DOCS, DERIVES
      )
    );
  }

  #[test]
  fn xs_string_element() {
    let element = Element {
      name: Some("volume".to_string()),
      kind: Some("xs:string".to_string()),
      refers: None,
      min_occurences: 1,
      max_occurences: MaxOccurences::Number { value: 1 },
      complex_type: None,
      simple_type: None,
      annotation: Some(Annotation {
        id: None,
        documentation: vec!["Loudness measured in Decibels".to_string()],
      }),
      ..Default::default()
    };

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    let value = element
      .get_implementation(&mut context)
      .unwrap()
      .to_string()
      .unwrap();
    let ts = quote!(#value).to_string();

    assert_eq!(
      ts,
      format!(
        "{}{}pub struct Volume {{ # [ yaserde ( text ) ] pub content : String , }}",
        DOCS, DERIVES
      )
    );
  }
}
