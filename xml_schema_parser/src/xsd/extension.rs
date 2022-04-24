use xsd_codegen::{Struct, XMLElement};
use xsd_types::{to_field_name, XsdIoError, XsdName, XsdParseError, XsdType};

use crate::xsd::{attribute::Attribute, sequence::Sequence, XsdContext};

use super::{
  annotation::Annotation,
  attribute_group::AttributeGroup,
  choice::Choice,
  group::Group,
  xsd_context::{MergeSettings, XsdElement, XsdImpl},
  XsdError,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Extension {
  pub base: XsdName,
  pub attributes: Vec<Attribute>,
  pub attribute_groups: Vec<AttributeGroup>,
  pub sequence: Option<Sequence>,
  pub group: Option<Group>,
  pub choice: Option<Choice>,
  pub annotation: Option<Annotation>,
}

impl Extension {
  pub fn parse(mut element: XMLElement) -> Result<Self, XsdIoError> {
    element.check_name("extension")?;

    let attributes = element.get_children_with("attribute", Attribute::parse)?;

    let attribute_groups = element.get_children_with("attributeGroup", AttributeGroup::parse)?;

    // group|all|choice|sequence
    let group = element.try_get_child_with("group", Group::parse)?;
    let choice = element.try_get_child_with("choice", Choice::parse)?;
    let sequence = element.try_get_child_with("sequence", Sequence::parse)?;

    if (!attributes.is_empty() || !attribute_groups.is_empty())
      && (group.is_some() || choice.is_some() || sequence.is_some())
    {
      return Err(XsdIoError::XsdParseError(XsdParseError {
        node_name: element.node_name(),
        msg: format!(
          "(group | choice | sequence) and (attribute | attributeGroup) cannot both present",
        ),
      }));
    }

    if group.is_some() as u8 + choice.is_some() as u8 + sequence.is_some() as u8 > 1 {
      return Err(XsdIoError::XsdParseError(XsdParseError {
        node_name: element.node_name(),
        msg: format!("group | choice | sequence cannot all be present",),
      }));
    }

    let output = Self {
      base: XsdName::new(
        &element.get_attribute::<String>("base")?,
        XsdType::SimpleType,
      ),
      sequence: element.try_get_child_with("sequence", Sequence::parse)?,
      group,
      choice,
      attributes,
      attribute_groups,
      annotation: element.try_get_child_with("annotation", Annotation::parse)?,
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  #[tracing::instrument(skip_all)]
  pub fn get_implementation(
    &self,
    parent_name: XsdName,
    context: &mut XsdContext,
  ) -> Result<XsdImpl, XsdError> {
    let generated_impl = context.multi_search(
      self.base.namespace.clone(),
      self.base.local_name.clone(),
      &[XsdType::SimpleType, XsdType::ComplexType],
    );
    let base_impl = match generated_impl {
      super::xsd_context::SearchResult::SingleMatch(imp) => imp,
      super::xsd_context::SearchResult::MultipleMatches => {
        return Err(XsdError::ContextSearchError {
          name: self.base.clone(),
          msg: format!("found both a simple and complex type"),
        });
      }
      super::xsd_context::SearchResult::NoMatches => {
        return Err(XsdError::XsdImplNotFound(self.base.clone()));
      }
    };

    let mut generated_impl = XsdImpl {
      name: parent_name.clone(),
      fieldname_hint: Some(parent_name.to_field_name()),
      element: XsdElement::Struct(Struct::new(None, &parent_name.to_struct_name()).vis("pub")),
      inner: vec![],
      implementation: vec![],
    };

    generated_impl.merge(base_impl.to_field(), MergeSettings::default());

    let to_merge_impl = match (&self.group, &self.sequence, &self.choice) {
      (None, None, Some(choice)) => Some(choice.get_implementation(Some(parent_name), context)),
      (None, Some(sequence), None) => Some(sequence.get_implementation(Some(parent_name), context)),
      (Some(group), None, None) => Some(group.get_implementation(Some(parent_name), context)),
      (None, None, None) => None,
      _ => unreachable!("Error parsing {}, Invalid XSD!", &parent_name.local_name),
    };

    if let Some(to_merge_impl) = to_merge_impl {
      generated_impl.merge(to_merge_impl?, MergeSettings::default());
    }

    for attribute in &self.attributes {
      generated_impl.merge(
        attribute.get_implementation(context, false)?,
        MergeSettings::ATTRIBUTE,
      );
    }

    for attribute in &self.attribute_groups {
      generated_impl.merge(
        attribute.get_implementation(None, context)?,
        MergeSettings::default(),
      );
    }

    generated_impl.name.ty = XsdType::Extension;

    Ok(generated_impl)
  }
}
