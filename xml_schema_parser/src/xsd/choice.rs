use crate::codegen::{Block, Enum, Field, Impl, Struct, Type};

use super::{
  element::Element,
  group::Group,
  max_occurences::MaxOccurences,
  sequence::Sequence,
  xsd_context::{to_struct_name, MergeSettings, XsdContext, XsdElement, XsdImpl, XsdName},
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
      groups: element.get_children_with("group", Group::parse)?,
      choices: element.get_children_with("choice", Choice::parse)?,
      sequences: element.get_children_with("sequence", Sequence::parse)?,
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  #[tracing::instrument(skip_all)]
  pub fn get_implementation(
    &self,
    parent_name: Option<XsdName>,
    context: &mut XsdContext,
  ) -> Result<XsdImpl, XsdError> {
    let mut outer_enum = XsdImpl {
      name: XsdName::new("temp"),
      fieldname_hint: "temp".to_string(),
      element: XsdElement::Enum(Enum::new(&"Temp")),
      inner: vec![],
      implementation: vec![],
    };

    let mut possible_enums = vec![];
    for group in &self.groups {
      outer_enum.merge_structs(
        group.get_implementation(None, context)?,
        MergeSettings::default(),
      );

      if let XsdElement::Enum(en) = &outer_enum.element {
        possible_enums.push(en.variants.last().unwrap().name.clone());
      } else {
        unreachable!();
      }
    }

    for sequence in &self.sequences {
      outer_enum.merge(
        sequence.get_implementation(None, context)?,
        MergeSettings::default(),
      );

      if let XsdElement::Enum(en) = &outer_enum.element {
        possible_enums.push(en.variants.last().unwrap().name.clone());
      } else {
        unreachable!();
      }
    }

    for choice in &self.choices {
      outer_enum.merge(
        choice.get_implementation(None, context)?,
        MergeSettings::default(),
      );

      if let XsdElement::Enum(en) = &outer_enum.element {
        possible_enums.push(en.variants.last().unwrap().name.clone());
      } else {
        unreachable!();
      }
    }

    for element in &self.elements {
      outer_enum.merge(
        element.get_implementation(context)?,
        MergeSettings::default(),
      );

      if let XsdElement::Enum(en) = &outer_enum.element {
        possible_enums.push(en.variants.last().unwrap().name.clone());
      } else {
        unreachable!();
      }
    }

    if let Some(parent_name) = parent_name {
      parent_name
    } else {
      outer_enum.set_type(outer_enum.infer_type_name());
    }
    imp.element.set_type(&new_type);
    for implementation in &mut imp.implementation {
      implementation.target = crate::codegen::Type::new(&new_type);
    }

    imp.name = XsdName::new(&new_type);

    let multiple = match &self.max_occurences {
      MaxOccurences::Unbounded => true,
      MaxOccurences::Number { value } => *value > 1,
    };

    let option = match &self.max_occurences {
      MaxOccurences::Unbounded => false,
      MaxOccurences::Number { value } => *value == 1 && self.min_occurences == 0,
    };

    let generated_impl = if multiple {
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
        inner: vec![inner_enum],
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
        inner: vec![inner_enum],
        implementation: vec![],
      })
    } else {
      Ok(outer_enum)
    };

    generated_impl
  }
}
