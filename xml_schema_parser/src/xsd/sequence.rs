use super::{
  annotation::Annotation,
  choice::Choice,
  group::Group,
  max_occurences::MaxOccurences,
  xsd_context::{MergeSettings, XsdElement, XsdImpl, XsdName},
  XMLElementWrapper, XsdError,
};
use crate::{
  codegen::Struct,
  xsd::{element::Element, XsdContext},
};

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct Sequence {
  pub id: Option<String>,
  pub min_occurences: u64,
  pub max_occurences: MaxOccurences,
  pub annotation: Option<Annotation>,
  pub elements: Vec<Element>,
  pub groups: Vec<Group>,
  pub choices: Vec<Choice>,
  pub sequences: Vec<Sequence>,
}

enum PureType {
  None,
  Element,
  Group,
  Choice,
  Sequence,
}

impl Sequence {
  pub fn parse(mut element: XMLElementWrapper) -> Result<Self, XsdError> {
    element.check_name("sequence")?;

    let output = Self {
      id: element.try_get_attribute("id")?,
      min_occurences: element.try_get_attribute("minOccurs")?.unwrap_or(1),
      max_occurences: element
        .try_get_attribute("maxOccurs")?
        .unwrap_or(MaxOccurences::Number { value: 1 }),
      annotation: element.try_get_child_with("annotation", Annotation::parse)?,
      elements: element.get_children_with("element", |child| Element::parse(child, false))?,
      groups: element.get_children_with("group", Group::parse)?,
      choices: element.get_children_with("choice", Choice::parse)?,
      sequences: element.get_children_with("sequence", Sequence::parse)?,
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  fn pure_type(&self) -> PureType {
    let has_elements = !self.elements.is_empty();
    let has_choices = !self.choices.is_empty();
    let has_groups = !self.groups.is_empty();
    let has_sequences = !self.sequences.is_empty();

    if has_elements as u8 + has_choices as u8 + has_groups as u8 + has_sequences as u8 == 1 {
      if has_elements {
        return PureType::Element;
      } else if has_choices {
        return PureType::Choice;
      } else if has_groups {
        return PureType::Group;
      } else if has_sequences {
        return PureType::Sequence;
      }
    }

    PureType::None
  }

  #[tracing::instrument(skip_all)]
  pub fn get_implementation(
    &self,
    parent_name: Option<XsdName>,
    context: &mut XsdContext,
  ) -> Result<XsdImpl, XsdError> {
    let mut generated_impl = XsdImpl {
      name: XsdName::new("temp"),
      fieldname_hint: None,
      element: XsdElement::Struct(Struct::new(&"Temp")),
      inner: vec![],
      implementation: vec![],
    };

    for element in &self.elements {
      generated_impl.merge(
        element.get_implementation(context)?,
        MergeSettings::default(),
      );
    }

    for choice in &self.choices {
      generated_impl.merge(
        choice.get_implementation(None, context)?,
        MergeSettings::default(),
      );
    }

    for sequence in &self.sequences {
      generated_impl.merge(
        sequence.get_implementation(None, context)?,
        MergeSettings::default(),
      );
    }

    for group in &self.groups {
      generated_impl.merge(
        group.get_implementation(None, context)?,
        MergeSettings::default(),
      );
    }

    Ok(generated_impl)
  }
}
