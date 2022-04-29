use xsd_codegen::{Enum, XMLElement};
use xsd_types::{to_struct_name, XsdIoError, XsdName, XsdType};

use super::{
  element::Element,
  general_xsdgen,
  group::Group,
  max_occurences::MaxOccurences,
  sequence::Sequence,
  xsd_context::{infer_type_name, MergeSettings, XsdContext, XsdElement, XsdImpl},
  XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
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
  pub fn parse(mut element: XMLElement) -> Result<Self, XsdIoError> {
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
    let mut generated_impls = vec![];

    // let mut possible_enums = vec![];
    for group in &self.groups {
      generated_impls.push(group.get_implementation(None, context)?);
    }

    for sequence in &self.sequences {
      generated_impls.push(sequence.get_implementation(None, context)?);
    }

    for element in &self.elements {
      generated_impls.push(element.get_implementation(context)?);
    }

    for choice in &self.choices {
      generated_impls.push(choice.get_implementation(None, context)?);
    }

    let inferred_name = infer_type_name(&generated_impls);

    let xml_name = if let Some(parent_name) = parent_name.clone() {
      parent_name
    } else {
      XsdName {
        namespace: None,
        local_name: inferred_name.clone(),
        ty: XsdType::Choice,
      }
    };

    let struct_name = if let Some(parent_name) = parent_name {
      parent_name.local_name
    } else {
      inferred_name
    };
    let struct_name = to_struct_name(&struct_name);

    let mut generated_impl = XsdImpl {
      fieldname_hint: Some(xml_name.to_field_name()),
      name: xml_name.clone(),
      element: XsdElement::Enum(
        Enum::new(Some(xml_name.clone()), &struct_name)
          .derives(&["Clone", "Debug", "PartialEq"])
          .vis("pub")
          .to_owned(),
      ),
      inner: vec![],
      implementation: vec![],
    };

    let mut variants = vec![];
    for imp in generated_impls {
      generated_impl.merge(imp, MergeSettings::default());
      if let Some(field) = generated_impl.element.get_last_added_field() {
        variants.push(field);
      }
    }

    let multiple = match &self.max_occurences {
      MaxOccurences::Unbounded => true,
      MaxOccurences::Number { value } => *value > 1,
    } || self.min_occurences > 1;

    let option = match &self.max_occurences {
      MaxOccurences::Unbounded => false,
      MaxOccurences::Number { value } => *value == 1 && self.min_occurences == 0,
    };

    let mut generated_impl = general_xsdgen(generated_impl);

    let mut generated_impl = if multiple {
      let old_name = generated_impl.name.clone();
      generated_impl.name.local_name = format!("inner-{}", old_name.local_name);
      let output = XsdImpl {
        name: old_name.clone(),
        fieldname_hint: Some(generated_impl.fieldname_hint.clone().unwrap()),
        element: XsdElement::Type(generated_impl.element.get_type().wrap("Vec").to_owned()),
        inner: vec![generated_impl],
        implementation: vec![],
      };
      output
    } else if option {
      let old_name = generated_impl.name.clone();
      generated_impl.name.local_name = format!("inner-{}", old_name.local_name);
      let output = XsdImpl {
        name: old_name.clone(),
        fieldname_hint: Some(generated_impl.fieldname_hint.clone().unwrap()),
        element: XsdElement::Type(generated_impl.element.get_type().wrap("Option").to_owned()),
        inner: vec![generated_impl],
        implementation: vec![],
      };
      output
    } else {
      generated_impl
    };

    generated_impl.name.ty = XsdType::Choice;

    Ok(generated_impl)
  }
}
