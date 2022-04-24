use xsd_codegen::{Block, Function, Impl, Struct, XMLElement};
use xsd_types::{XsdIoError, XsdName, XsdType};

use super::{
  annotation::Annotation,
  choice::Choice,
  general_xsdgen,
  group::Group,
  max_occurences::MaxOccurences,
  xsd_context::{infer_type_name, MergeSettings, XsdElement, XsdImpl},
  XsdError,
};
use crate::xsd::{element::Element, XsdContext};

#[derive(Clone, Default, Debug, PartialEq)]
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
  pub fn parse(mut element: XMLElement) -> Result<Self, XsdIoError> {
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
    let mut generated_impls = vec![];

    for element in &self.elements {
      generated_impls.push(element.get_implementation(context)?);
    }

    for choice in &self.choices {
      generated_impls.push(choice.get_implementation(None, context)?);
    }

    for sequence in &self.sequences {
      generated_impls.push(sequence.get_implementation(None, context)?);
    }

    for group in &self.groups {
      generated_impls.push(group.get_implementation(None, context)?);
    }

    let mut xml_name = if let Some(parent_name) = parent_name.clone() {
      parent_name
    } else {
      let inferred_name = infer_type_name(&generated_impls);
      XsdName {
        namespace: None,
        local_name: inferred_name.clone(),
        ty: XsdType::Sequence,
      }
    };
    xml_name.ty = XsdType::Sequence;

    let mut generated_impl = XsdImpl {
      name: xml_name.clone(),
      fieldname_hint: Some(xml_name.to_field_name()),
      element: XsdElement::Struct(
        Struct::new(Some(xml_name.clone()), &xml_name.to_struct_name())
          .vis("pub")
          .derives(&["Clone", "Debug", "Default", "PartialEq"]),
      ),
      inner: vec![],
      implementation: vec![],
    };

    for imp in generated_impls {
      generated_impl.merge(imp, MergeSettings::default());
    }

    generated_impl.name.ty = XsdType::Sequence;

    Ok(general_xsdgen(generated_impl))
  }
}
