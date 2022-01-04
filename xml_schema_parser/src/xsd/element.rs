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
use heck::CamelCase;

use super::XMLElementWrapper;

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
  pub fn parse(mut element: XMLElementWrapper) -> Result<Self, XsdError> {
    element.check_name("element")?;

    let complex_type =
      element.try_get_child_with("complexType", |child| ComplexType::parse(child))?;
    let simple_type =
      element.try_get_child_with("simpleType", |child| SimpleType::parse(child, false))?;
    let annotation = element.try_get_child_with("annotation", |child| Annotation::parse(child))?;

    let output = Ok(Self {
      name: element.try_get_attribute("name")?,
      kind: element.try_get_attribute("type")?,
      refers: element.try_get_attribute("ref")?,
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

  pub fn get_implementation(&self, context: &mut XsdContext) -> XsdImpl {
    let name = self.name.clone().unwrap_or("temp".to_string());
    let type_name = name.replace(".", "_").to_camel_case();

    let generated_impl = if self.is_multiple() || self.could_be_none() {
      let mut generated_field = self.get_field(context);
      let docs = generated_field.documentation.join("\n");
      generated_field.documentation = vec![];

      let generated_struct = Struct::new(&type_name)
        .push_field(generated_field)
        .doc(&docs)
        .to_owned();

      XsdImpl {
        element: XsdElement::Struct(generated_struct),
        ..Default::default()
      }
    } else {
      let docs = self
        .annotation
        .as_ref()
        .map(|annotation| annotation.get_doc());

      let generated_impl = match (&self.simple_type, &self.complex_type) {
        (None, Some(complex_type)) => complex_type.get_implementation(context),
        (Some(simple_type), None) => simple_type.get_implementation(context),
        _ => unreachable!("Invalid Xsd."),
      };

      generated_impl
    };

    generated_impl
  }

  pub fn get_field(&self, context: &mut XsdContext) -> Field {
    let mut field_type = match (&self.simple_type, &self.complex_type) {
      (None, Some(complex_type)) => complex_type.get_implementation(context).element.get_type(),
      (Some(simple_type), None) => simple_type.get_implementation(context).element.get_type(),
      _ => unreachable!("Invalid Xsd."),
    };

    let name = self.name.clone().unwrap_or("temp".to_string());

    let field_name = XsdName::new(&name).to_field_name();

    let multiple = self.is_multiple();

    let field_name = if multiple {
      format!("{}s", field_name)
    } else {
      field_name
    };

    let yaserde_rename = self.name.clone().unwrap_or("temp".to_string());

    if multiple {
      field_type.wrap("Vec");
    }

    if self.could_be_none() {
      field_type.wrap("Option");
    }

    let mut generated_field = Field::new(&field_name, field_type)
      .vis("pub")
      .annotation(vec![&format!("yaserde(rename={})", yaserde_rename)])
      .to_owned();

    let docs = self
      .annotation
      .as_ref()
      .map(|annotation| annotation.get_doc());

    if let Some(docs) = docs {
      generated_field.doc(docs.iter().map(|f| f.as_str()).collect());
    }

    generated_field
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
      .to_string()
      .unwrap();
    let ts = quote!(#value).to_string();

    assert_eq!(
      ts.to_string(),
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
      .to_string()
      .unwrap();
    let ts = quote!(#value).to_string();

    assert_eq!(
      ts.to_string(),
      format!(
        "{}{}pub struct Volume {{ # [ yaserde ( text ) ] pub content : String , }}",
        DOCS, DERIVES
      )
    );
  }
}
