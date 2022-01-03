use std::io::prelude::*;
use yaserde::YaDeserialize;

use super::{
  choice::Choice,
  max_occurences::MaxOccurences,
  sequence::Sequence,
  xsd_context::{XsdContext, XsdElement, XsdImpl, XsdName},
};

#[derive(Clone, Default, Debug, PartialEq, YaDeserialize)]
#[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct Group {
  #[yaserde(attribute)]
  pub id: Option<String>,
  #[yaserde(attribute)]
  pub name: Option<String>,
  #[yaserde(rename = "ref", attribute)]
  pub refers: Option<String>,
  #[yaserde(rename = "minOccurs", attribute)]
  pub min_occurences: Option<u64>,
  #[yaserde(rename = "maxOccurs", attribute)]
  pub max_occurences: Option<MaxOccurences>,
  pub sequence: Option<Sequence>,
  pub choice: Option<Choice>,
}

impl Group {
  pub fn get_implementation(
    &self,
    parent_name: Option<XsdName>,
    context: &mut XsdContext,
  ) -> XsdImpl {
    match (&self.name, parent_name, &self.refers) {
      (Some(name), _, None) => match (&self.choice, &self.sequence) {
        (None, Some(sequence)) => sequence.get_implementation(
          XsdName {
            namespace: None,
            local_name: name.clone(),
          },
          context,
        ),
        (Some(choice), None) => choice.get_implementation(
          XsdName {
            namespace: None,
            local_name: name.clone(),
          },
          context,
        ),
        _ => unreachable!("The Xsd is invalid!"),
      },
      (None, Some(name), Some(refers)) => {
        let mut inner = context
          .structs
          .get(&XsdName {
            namespace: None,
            local_name: refers.to_string(),
          })
          .unwrap()
          .clone();

        inner.element.set_type(&name.local_name);

        inner
      }
      _ => unreachable!("The Xsd is invalid!"),
    }
  }
}
