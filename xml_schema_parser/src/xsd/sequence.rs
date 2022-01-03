use crate::{
  codegen::Struct,
  xsd::{element::Element, XsdContext},
};
use log::debug;
use std::io::prelude::*;
use yaserde::YaDeserialize;

use super::{
  choice::Choice,
  group::Group,
  xsd_context::{MergeSettings, XsdElement, XsdImpl, XsdName},
};

#[derive(Clone, Default, Debug, PartialEq, YaDeserialize)]
#[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct Sequence {
  #[yaserde(rename = "element")]
  pub elements: Vec<Element>,
  #[yaserde(rename = "group")]
  pub groups: Vec<Group>,
  #[yaserde(rename = "choice")]
  pub choices: Vec<Choice>,
  #[yaserde(rename = "sequence")]
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

  pub fn get_implementation(&self, parent_name: XsdName, context: &mut XsdContext) -> XsdImpl {
    let pure_type = self.pure_type();

    match pure_type {
      PureType::None | PureType::Choice | PureType::Element => {
        let mut generated_impl = XsdImpl {
          element: XsdElement::Struct(Struct::new(&parent_name.local_name)),
          ..XsdImpl::default()
        };

        for element in &self.elements {
          generated_impl.merge(
            element.get_implementation(context),
            MergeSettings::default(),
          );
        }

        for choice in &self.choices {
          generated_impl.merge(
            choice.get_implementation(
              XsdName {
                namespace: None,
                local_name: "temp".to_string(),
              },
              context,
            ),
            MergeSettings::default(),
          );
        }

        for sequence in &self.sequences {
          generated_impl.merge(
            sequence.get_implementation(
              XsdName {
                namespace: None,
                local_name: "temp".to_string(),
              },
              context,
            ),
            MergeSettings::default(),
          );
        }

        generated_impl
      }
      PureType::Group => {
        let mut generated_impl = XsdImpl::default();
        for group in &self.groups {
          generated_impl.merge(
            group.get_implementation(
              Some(XsdName {
                namespace: None,
                local_name: "temp".to_string(),
              }),
              context,
            ),
            MergeSettings::default(),
          );
        }

        generated_impl
      }
      PureType::Sequence => todo!(),
    }
  }
}
