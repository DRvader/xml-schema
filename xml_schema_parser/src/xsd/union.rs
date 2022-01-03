use log::debug;
use std::io::prelude::*;
use yaserde::YaDeserialize;

use crate::codegen::Enum;

use super::xsd_context::{XsdContext, XsdElement, XsdImpl, XsdName};

#[derive(Clone, Default, Debug, PartialEq, YaDeserialize)]
#[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct Union {
  #[yaserde(rename = "memberTypes", attribute)]
  pub member_types: String,
}

impl Union {
  pub fn get_implementation(&self, parent_name: XsdName, context: &mut XsdContext) -> XsdImpl {
    let mut generated_enum = Enum::new(&parent_name.local_name);

    for member in self.member_types.split_whitespace() {
      let str = context
        .structs
        .get(&XsdName {
          namespace: None,
          local_name: member.to_string(),
        })
        .unwrap();
      generated_enum
        .new_variant(&str.element.get_type().name)
        .tuple(str.element.get_type());
    }

    XsdImpl {
      element: XsdElement::Enum(generated_enum),
      ..Default::default()
    }
  }
}
