use xsd_codegen::{Struct, XMLElement};
use xsd_types::{XsdIoError, XsdName, XsdType};

use super::{
  annotation::Annotation,
  choice::Choice,
  general_xsdgen,
  group::Group,
  max_occurences::MaxOccurences,
  xsd_context::{infer_type_name, MergeSettings, XsdImpl, XsdImplType},
  XsdError,
};
use crate::xsd::{element::Element, XsdContext};

#[derive(Clone, Debug, PartialEq)]
pub enum SequenceOptions {
  Element(Element),
  Group(Group),
  Choice(Choice),
  Sequence(Sequence),
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Sequence {
  pub id: Option<String>,
  pub min_occurences: u64,
  pub max_occurences: MaxOccurences,
  pub annotation: Option<Annotation>,
  pub children: Vec<SequenceOptions>,
}

impl Sequence {
  pub fn parse(mut element: XMLElement) -> Result<Self, XsdIoError> {
    element.check_name("sequence")?;

    let mut children = vec![];
    for child in element.get_all_children() {
      children.push(match child.element.name.as_str() {
        "element" => SequenceOptions::Element(Element::parse(child, false)?),
        "group" => SequenceOptions::Group(Group::parse(child)?),
        "choice" => SequenceOptions::Choice(Choice::parse(child)?),
        "sequence" => SequenceOptions::Sequence(Sequence::parse(child)?),
        name => unreachable!("Unexpected child name {name}"),
      });
    }

    let output = Self {
      id: element.try_get_attribute("id")?,
      min_occurences: element.try_get_attribute("minOccurs")?.unwrap_or(1),
      max_occurences: element
        .try_get_attribute("maxOccurs")?
        .unwrap_or(MaxOccurences::Number { value: 1 }),
      annotation: element.try_get_child_with("annotation", Annotation::parse)?,
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
        SequenceOptions::Element(element) => {
          generated_impls.push(element.get_implementation(context)?)
        }
        SequenceOptions::Group(group) => {
          generated_impls.push(group.get_implementation(None, context)?)
        }
        SequenceOptions::Choice(choice) => {
          generated_impls.push(choice.get_implementation(None, context)?)
        }
        SequenceOptions::Sequence(sequence) => {
          generated_impls.push(sequence.get_implementation(None, context)?)
        }
      }
    }

    let mut xml_name = if let Some(parent_name) = parent_name.clone() {
      parent_name
    } else {
      let inferred_name = infer_type_name(&generated_impls);
      XsdName {
        namespace: None,
        local_name: inferred_name,
        ty: XsdType::Sequence,
      }
    };
    xml_name.ty = XsdType::Sequence;

    let mut generated_impl = XsdImpl {
      name: xml_name.clone(),
      fieldname_hint: Some(xml_name.to_field_name()),
      element: XsdImplType::Struct(
        Struct::new(Some(xml_name.clone()), &xml_name.to_struct_name())
          .vis("pub")
          .derives(&["Clone", "Debug", "PartialEq"]),
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
        flatten: generated_impl.flatten,
        inner: vec![generated_impl],
        implementation: vec![],
      }
    } else if option {
      let old_name = generated_impl.name.clone();
      generated_impl.name.local_name = format!("inner-{}", old_name.local_name);
      XsdImpl {
        name: old_name,
        fieldname_hint: Some(generated_impl.fieldname_hint.clone().unwrap()),
        element: XsdImplType::Type(generated_impl.element.get_type().wrap("Option")),
        flatten: generated_impl.flatten,
        inner: vec![generated_impl],
        implementation: vec![],
      }
    } else {
      generated_impl
    };

    generated_impl.name.ty = XsdType::Sequence;

    Ok(generated_impl)
  }
}
