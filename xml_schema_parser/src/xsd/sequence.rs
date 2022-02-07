use super::{
  annotation::Annotation,
  choice::Choice,
  group::Group,
  max_occurences::MaxOccurences,
  xsd_context::{
    infer_type_name, to_struct_name, MergeSettings, XsdElement, XsdImpl, XsdName, XsdType,
  },
  XMLElementWrapper, XsdError,
};
use crate::{
  codegen::{Block, Function, Impl, Struct},
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

    let inferred_name = infer_type_name(&generated_impls);

    let mut xml_name = if let Some(parent_name) = parent_name.clone() {
      parent_name
    } else {
      XsdName {
        namespace: None,
        local_name: inferred_name.clone(),
        ty: XsdType::Sequence,
      }
    };
    xml_name.ty = XsdType::Sequence;

    let struct_name = if let Some(parent_name) = parent_name {
      parent_name.local_name
    } else {
      inferred_name
    };
    let struct_name = to_struct_name(&struct_name);

    let mut generated_impl = XsdImpl {
      name: xml_name,
      fieldname_hint: None,
      element: XsdElement::Struct(
        Struct::new(&struct_name)
          .vis("pub")
          .derives(&["Clone", "Debug", "Default", "PartialEq"])
          .to_owned(),
      ),
      inner: vec![],
      implementation: vec![],
    };

    let mut parsable_fields = vec![];
    for imp in generated_impls {
      generated_impl.merge(imp, MergeSettings::default());
      if let Some(field) = generated_impl.element.get_last_added_field() {
        parsable_fields.push(field);
      }
    }

    let mut value = Function::new("parse")
      .vis("pub")
      .arg("mut element", "XMLElementWrapper")
      .ret("Result<Self, XsdError>")
      .to_owned();

    for (field, ty) in &parsable_fields {
      value.line(format!("let {field} = XsdParse::parse(element)?;"));
    }

    let mut block = Block::new("let output = Self").after(";").to_owned();
    for (field, _) in parsable_fields {
      block.line(format!("{field},"));
    }
    value.push_block(block.to_owned());

    value.line("element.finalize(false, false)?;");
    value.line("Ok(output)");

    let struct_impl = Impl::new(generated_impl.element.get_type())
      .push_fn(value)
      .to_owned();

    // generated_impl.implementation.push(struct_impl);

    Ok(generated_impl)
  }
}
