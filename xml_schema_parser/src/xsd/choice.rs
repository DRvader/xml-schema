use std::io::prelude::*;
use yaserde::YaDeserialize;

use crate::codegen::{Enum, Field, Struct, Type};

use super::{
  element::Element,
  group::Group,
  max_occurences::MaxOccurences,
  sequence::Sequence,
  xsd_context::{XsdContext, XsdElement, XsdImpl, XsdName},
};

#[derive(Clone, Default, Debug, PartialEq, YaDeserialize)]
#[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct Choice {
  #[yaserde(attribute)]
  pub id: Option<String>,
  #[yaserde(rename = "minOccurs", attribute)]
  pub min_occurences: Option<u64>,
  #[yaserde(rename = "maxOccurs", attribute)]
  pub max_occurences: Option<MaxOccurences>,
  #[yaserde(rename = "element")]
  pub elements: Vec<Element>,
  #[yaserde(rename = "group")]
  pub groups: Vec<Group>,
  #[yaserde(rename = "choice")]
  pub choices: Vec<Choice>,
  #[yaserde(rename = "sequence")]
  pub sequences: Vec<Sequence>,
}

impl Choice {
  pub fn get_implementation(&self, parent_name: XsdName, context: &mut XsdContext) -> XsdImpl {
    let mut outer_enum = XsdImpl {
      name: Some(parent_name.clone()),
      element: XsdElement::Enum(Enum::new(&parent_name.to_struct_name())),
      inner: vec![],
      implementation: vec![],
    };
    for group in &self.groups {
      outer_enum.merge_into_enum(
        group.get_implementation(Some(parent_name.clone()), context),
        true,
      );
    }

    for sequence in &self.sequences {
      outer_enum.merge_into_enum(
        sequence.get_implementation(
          XsdName {
            namespace: None,
            local_name: "temp".to_string(),
          },
          context,
        ),
        false,
      );
    }

    for choice in &self.choices {
      outer_enum.merge_into_enum(
        choice.get_implementation(
          XsdName {
            namespace: None,
            local_name: "temp".to_string(),
          },
          context,
        ),
        false,
      );
    }

    for element in &self.elements {
      outer_enum.merge_into_enum(element.get_implementation(context), true);
    }

    let min_occurances = self.min_occurences.unwrap_or(1);
    let max_occurances = self
      .max_occurences
      .as_ref()
      .unwrap_or(&MaxOccurences::Number { value: 1 });
    let multiple = match max_occurances {
      MaxOccurences::Unbounded => true,
      MaxOccurences::Number { value } => *value > 1,
    };

    let option = match max_occurances {
      MaxOccurences::Unbounded => false,
      MaxOccurences::Number { value } => *value == 1 && min_occurances == 0,
    };

    if multiple {
      let mut inner_enum = outer_enum;
      match &mut inner_enum.element {
        XsdElement::Struct(str) => {
          str.type_def.ty.prefix("Inner");
        }
        XsdElement::Enum(en) => {
          en.type_def.ty.prefix("Inner");
        }
        XsdElement::Type(ty) => {
          ty.prefix("Inner");
        }
        _ => {}
      }

      XsdImpl {
        name: Some(parent_name.clone()),
        element: XsdElement::Struct(
          Struct::new(&parent_name.to_struct_name())
            .push_field(
              Field::new(
                "inner",
                inner_enum.element.get_type().wrap("Vec").to_owned(),
              )
              .annotation(vec!["yaserde(flatten)"])
              .to_owned(),
            )
            .to_owned(),
        ),
        inner: vec![Box::from(inner_enum)],
        implementation: vec![],
      }
    } else if option {
      let mut inner_enum = outer_enum;
      match &mut inner_enum.element {
        XsdElement::Struct(str) => {
          str.type_def.ty.prefix("Inner");
        }
        XsdElement::Enum(en) => {
          en.type_def.ty.prefix("Inner");
        }
        XsdElement::Type(ty) => {
          ty.prefix("Inner");
        }
        _ => {}
      }

      XsdImpl {
        name: Some(parent_name.clone()),
        element: XsdElement::Struct(
          Struct::new(&parent_name.to_struct_name())
            .push_field(
              Field::new(
                "inner",
                Type::new(&inner_enum.element.get_type().wrap("Option").to_string()),
              )
              .annotation(vec!["yaserde(flatten)"])
              .to_owned(),
            )
            .to_owned(),
        ),
        inner: vec![Box::from(inner_enum)],
        implementation: vec![],
      }
    } else {
      outer_enum
    }
  }
}
