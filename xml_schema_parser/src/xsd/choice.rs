use xsd_codegen::{Enum, XMLElement};
use xsd_types::{to_struct_name, XsdIoError, XsdName, XsdType};

use super::{
  element::Element,
  general_xsdgen,
  group::Group,
  max_occurences::MaxOccurences,
  sequence::Sequence,
  xsd_context::{infer_type_name, MergeSettings, XsdContext, XsdImpl, XsdImplType},
  XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Choice {
  pub id: Option<String>,
  pub min_occurences: u64,
  pub max_occurences: MaxOccurences,
  pub children: Vec<ChoiceOptions>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChoiceOptions {
  Element(Element),
  Group(Group),
  Choice(Choice),
  Sequence(Sequence),
}

impl Choice {
  pub fn parse(mut element: XMLElement) -> Result<Self, XsdIoError> {
    element.check_name("choice")?;

    let mut children = vec![];
    for child in element.get_all_children() {
      children.push(match child.element.name.as_str() {
        "element" => ChoiceOptions::Element(Element::parse(child, false)?),
        "group" => ChoiceOptions::Group(Group::parse(child)?),
        "choice" => ChoiceOptions::Choice(Choice::parse(child)?),
        "sequence" => ChoiceOptions::Sequence(Sequence::parse(child)?),
        name => unreachable!("Unexpected child name {name}"),
      });
    }

    let output = Self {
      id: element.try_get_attribute("id")?,
      min_occurences: element.try_get_attribute("minOccurs")?.unwrap_or(1),
      max_occurences: element.get_attribute_default("maxOccurs")?,
      children,
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

    for child in &self.children {
      match child {
        ChoiceOptions::Element(element) => {
          generated_impls.push(element.get_implementation(context)?)
        }
        ChoiceOptions::Group(group) => {
          generated_impls.push(group.get_implementation(None, context)?)
        }
        ChoiceOptions::Choice(choice) => {
          generated_impls.push(choice.get_implementation(None, context)?)
        }
        ChoiceOptions::Sequence(sequence) => {
          generated_impls.push(sequence.get_implementation(None, context)?)
        }
      }
    }

    let inferred_name = infer_type_name(&generated_impls);

    let xml_name = if let Some(parent_name) = parent_name.clone() {
      parent_name
    } else {
      XsdName {
        namespace: None,
        local_name: inferred_name,
        ty: XsdType::Choice,
      }
    };

    let struct_name = xml_name.local_name.clone();
    let struct_name = to_struct_name(&struct_name);

    let mut generated_impl = XsdImpl {
      fieldname_hint: Some(xml_name.to_field_name()),
      name: xml_name.clone(),
      element: XsdImplType::Enum(
        Enum::new(Some(xml_name), &struct_name)
          .derives(&["Clone", "Debug", "PartialEq"])
          .vis("pub"),
      ),
      inner: vec![],
      implementation: vec![],
      flatten: parent_name.is_none(),
    };

    for imp in generated_impls {
      generated_impl.merge(imp, MergeSettings::default());
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
      XsdImpl {
        name: old_name,
        fieldname_hint: Some(generated_impl.fieldname_hint.clone().unwrap()),
        element: XsdImplType::Type(generated_impl.element.get_type().wrap("Vec")),
        inner: vec![generated_impl],
        implementation: vec![],
        flatten: parent_name.is_none(),
      }
    } else if option {
      let old_name = generated_impl.name.clone();
      generated_impl.name.local_name = format!("inner-{}", old_name.local_name);
      XsdImpl {
        name: old_name,
        fieldname_hint: Some(generated_impl.fieldname_hint.clone().unwrap()),
        element: XsdImplType::Type(generated_impl.element.get_type().wrap("Option")),
        inner: vec![generated_impl],
        implementation: vec![],
        flatten: parent_name.is_none(),
      }
    } else {
      generated_impl
    };

    generated_impl.name.ty = XsdType::Choice;

    Ok(generated_impl)
  }
}
