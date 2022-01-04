use crate::codegen::Enum;

use super::{
  simple_type::SimpleType,
  xsd_context::{XsdContext, XsdElement, XsdImpl, XsdName},
  XMLElementWrapper, XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct Union {
  pub member_types: Vec<String>,
  pub simple_types: Vec<SimpleType>,
}

impl Union {
  pub fn parse(mut element: XMLElementWrapper) -> Result<Self, XsdError> {
    element.check_name("union")?;

    let member_types: Option<String> = element.try_get_attribute("memberTypes")?;
    let mut members = vec![];

    if let Some(member_types) = member_types {
      for member in member_types.split_whitespace() {
        members.push(member.to_string());
      }
    }

    let output = Self {
      member_types: members,
      simple_types: element
        .get_children_with("simpleType", |child| SimpleType::parse(child, false))?,
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  pub fn get_implementation(&self, parent_name: XsdName, context: &mut XsdContext) -> XsdImpl {
    let mut generated_enum = Enum::new(&parent_name.local_name);

    for member in &self.member_types {
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
