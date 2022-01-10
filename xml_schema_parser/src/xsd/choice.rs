use crate::codegen::{Block, Enum, Field, Impl, Struct, Type};

use super::{
  element::Element,
  group::Group,
  max_occurences::MaxOccurences,
  sequence::Sequence,
  xsd_context::{MergeSettings, XsdContext, XsdElement, XsdImpl, XsdName},
  XMLElementWrapper, XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct Choice {
  pub id: Option<String>,
  pub min_occurences: u64,
  pub max_occurences: MaxOccurences,
  pub elements: Vec<Element>,
  pub groups: Vec<Group>,
  pub choices: Vec<Choice>,
  pub sequences: Vec<Sequence>,
}

impl Choice {
  pub fn parse(mut element: XMLElementWrapper) -> Result<Self, XsdError> {
    element.check_name("choice")?;

    let output = Self {
      id: element.try_get_attribute("id")?,
      min_occurences: element.try_get_attribute("minOccurs")?.unwrap_or(1),
      max_occurences: element.get_attribute_default("maxOccurs")?,
      elements: element.get_children_with("element", |child| Element::parse(child, false))?,
      groups: element.get_children_with("group", |child| Group::parse(child))?,
      choices: element.get_children_with("choice", |child| Choice::parse(child))?,
      sequences: element.get_children_with("sequence", |child| Sequence::parse(child))?,
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  pub fn get_implementation(
    &self,
    parent_name: XsdName,
    context: &mut XsdContext,
  ) -> Result<XsdImpl, XsdError> {
    let mut outer_enum = XsdImpl {
      name: Some(parent_name.clone()),
      fieldname_hint: Some(parent_name.to_field_name()),
      element: XsdElement::Enum(Enum::new(&parent_name.to_struct_name())),
      inner: vec![],
      implementation: vec![],
    };
    for group in &self.groups {
      outer_enum.merge(
        group.get_implementation(Some(parent_name.clone()), context)?,
        MergeSettings::default(),
      );
    }

    for sequence in &self.sequences {
      outer_enum.merge(
        sequence.get_implementation(
          XsdName {
            namespace: None,
            local_name: "temp".to_string(),
          },
          context,
        )?,
        MergeSettings::default(),
      );
    }

    for choice in &self.choices {
      outer_enum.merge(
        choice.get_implementation(
          XsdName {
            namespace: None,
            local_name: "temp".to_string(),
          },
          context,
        )?,
        MergeSettings::default(),
      );
    }

    for element in &self.elements {
      outer_enum.merge(
        element.get_implementation(context)?,
        MergeSettings::default(),
      );
    }

    let multiple = match &self.max_occurences {
      MaxOccurences::Unbounded => true,
      MaxOccurences::Number { value } => *value > 1,
    };

    let option = match &self.max_occurences {
      MaxOccurences::Unbounded => false,
      MaxOccurences::Number { value } => *value == 1 && self.min_occurences == 0,
    };

    if multiple {
      let mut inner_impl = Impl::new(&parent_name.to_struct_name());

      inner_impl
        .new_fn("parse")
        .vis("pub")
        .arg("mut element", "XMLElementWrapper")
        .ret("Result<Self, XsdError>")
        .push_block(
          Block::new("")
            .line(format!(
              "element.check_name(\"{}\")?;",
              parent_name.local_name
            ))
            .push_block(
              Block::new(&format!(
                "element.get_children_with(\"{}\", |child| ",
                parent_name.local_name
              ))
              .line("")
              .after(")?;")
              .to_owned(),
            )
            .line("element.finalize(false, false)?;")
            .line("Ok(output);")
            .to_owned(),
        );

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

      Ok(XsdImpl {
        name: Some(parent_name.clone()),
        fieldname_hint: Some(parent_name.to_field_name()),
        element: XsdElement::Struct(
          Struct::new(&parent_name.to_struct_name())
            .push_field(Field::new(
              "inner",
              inner_enum.element.get_type().wrap("Vec").to_owned(),
            ))
            .to_owned(),
        ),
        inner: vec![Box::from(inner_enum)],
        implementation: vec![inner_impl],
      })
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

      Ok(XsdImpl {
        name: Some(parent_name.clone()),
        fieldname_hint: Some(parent_name.to_field_name()),
        element: XsdElement::Struct(
          Struct::new(&parent_name.to_struct_name())
            .push_field(Field::new(
              "inner",
              Type::new(&inner_enum.element.get_type().wrap("Option").to_string()),
            ))
            .to_owned(),
        ),
        inner: vec![Box::from(inner_enum)],
        implementation: vec![],
      })
    } else {
      Ok(outer_enum)
    }
  }
}
