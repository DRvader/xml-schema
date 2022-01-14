use super::{
  attribute_group::AttributeGroup, choice::Choice, group::Group, XMLElementWrapper, XsdError,
};
use crate::{
  codegen::Struct,
  xsd::{
    annotation::Annotation,
    attribute::Attribute,
    complex_content::ComplexContent,
    sequence::Sequence,
    simple_content::SimpleContent,
    xsd_context::{to_struct_name, MergeSettings, XsdElement, XsdImpl, XsdName},
    XsdContext,
  },
};

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(
//   rename = "complexType"
//   prefix = "xs",
//   namespace = "xs: http://www.w3.org/2001/XMLSchema"
// )]
pub struct ComplexType {
  pub name: Option<String>,
  pub attributes: Vec<Attribute>,
  pub attribute_groups: Vec<AttributeGroup>,
  pub choice: Option<Choice>,
  pub group: Option<Group>,
  pub sequence: Option<Sequence>,
  pub simple_content: Option<SimpleContent>,
  pub complex_content: Option<ComplexContent>,
  pub annotation: Option<Annotation>,
}

impl ComplexType {
  pub fn parse(mut element: XMLElementWrapper) -> Result<Self, XsdError> {
    element.check_name("complexType")?;

    // (annotation?,(simpleContent|complexContent|((group|all|choice|sequence)?,((attribute|attributeGroup)*,anyAttribute?))))
    let simple_content = element.try_get_child_with("simpleContent", SimpleContent::parse)?;
    let complex_content = element.try_get_child_with("complexContent", ComplexContent::parse)?;

    let choice = element.try_get_child_with("choice", Choice::parse)?;
    let group = element.try_get_child_with("group", Group::parse)?;
    let sequence = element.try_get_child_with("sequence", Sequence::parse)?;

    let attributes = element.get_children_with("attribute", Attribute::parse)?;

    let attribute_groups = element.get_children_with("attributeGroup", AttributeGroup::parse)?;

    if simple_content.is_some() && complex_content.is_some() {
      return Err(XsdError::XsdParseError(format!(
        "simpleContent | complexContent cannot both present in {}",
        element.name()
      )));
    }

    if (simple_content.is_some() || complex_content.is_some())
      && (!attributes.is_empty()
        || !attribute_groups.is_empty()
        || group.is_some()
        || choice.is_some()
        || sequence.is_some())
    {
      return Err(XsdError::XsdParseError(format!(
        "(simpleContent | complexContent) and (group | choice | sequence | attribute | attributeGroup) cannot both present in {}",
        element.name()
      )));
    }

    if group.is_some() as u8 + choice.is_some() as u8 + sequence.is_some() as u8 > 1 {
      return Err(XsdError::XsdParseError(format!(
        "group | choice | sequence cannot all be present in {}",
        element.name()
      )));
    }

    let output = Self {
      name: element.try_get_attribute("name")?,
      choice,
      group,
      sequence,
      simple_content,
      complex_content,
      attribute_groups,
      attributes,
      annotation: element.try_get_child_with("annotation", Annotation::parse)?,
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  pub fn get_implementation(&self, context: &mut XsdContext) -> Result<XsdImpl, XsdError> {
    let name = self.name.clone().unwrap_or_else(|| "temp".to_string());

    log::debug!("Entered Complex Type: {:?}", &name);

    let struct_id = XsdName {
      namespace: None,
      local_name: name.clone(),
    };

    if context.structs.contains_key(&struct_id) {
      return Ok(context.structs.get(&struct_id).unwrap().clone());
    }

    let fields = match (
      &self.complex_content,
      &self.simple_content,
      &self.group,
      &self.sequence,
    ) {
      (Some(complex_content), None, None, None) => {
        complex_content.get_implementation(struct_id, context)
      }
      (None, Some(simple_content), None, None) => {
        simple_content.get_implementation(struct_id, context)
      }
      (None, None, Some(group), None) => group.get_implementation(Some(struct_id), context),
      (None, None, None, Some(sequence)) => sequence.get_implementation(struct_id, context),
      (None, None, None, None) => Ok(XsdImpl {
        name: Some(struct_id),
        element: XsdElement::Struct(Struct::new(&to_struct_name(&name))),
        ..Default::default()
      }),
      _ => unreachable!("Xsd is invalid."),
    };

    let docs = self
      .annotation
      .as_ref()
      .map(|annotation| annotation.get_doc())
      .unwrap_or_default();

    let mut generated_impl = XsdImpl {
      name: self.name.as_ref().map(|n| XsdName::new(n)),
      element: XsdElement::Struct(
        Struct::new(&to_struct_name(&name))
          .doc(&docs.join("\n"))
          .derive("#[derive(Clone, Debug, Default, PartialEq, YaDeserialize, YaSerialize)]")
          .to_owned(),
      ),
      ..Default::default()
    };

    generated_impl.merge(fields?, MergeSettings::default());
    for attribute in &self.attributes {
      if let Some(generated) = attribute.get_implementation(context)? {
        generated_impl.merge(
          generated,
          MergeSettings {
            conflict_prefix: Some("attr_"),
            merge_type: crate::xsd::xsd_context::MergeType::Structs,
          },
        );
      }
    }

    log::debug!("Exited Complex Type: {:?}", &name);

    Ok(generated_impl)
  }
}
