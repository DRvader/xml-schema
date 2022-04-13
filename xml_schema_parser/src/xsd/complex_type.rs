use super::{
  annotation::Annotation,
  attribute::Attribute,
  attribute_group::AttributeGroup,
  choice::Choice,
  complex_content::ComplexContent,
  group::Group,
  sequence::Sequence,
  simple_content::SimpleContent,
  xsd_context::{infer_type_name, XsdType},
  xsd_context::{MergeSettings, XsdElement, XsdImpl, XsdName},
  XMLElementWrapper, XsdContext, XsdError,
};
use crate::codegen::Struct;

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(
//   rename = "complexType"
//   prefix = "xs",
//   namespace = "xs: http://www.w3.org/2001/XMLSchema"
// )]
pub struct ComplexType {
  pub name: Option<XsdName>,
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
      name: element
        .try_get_attribute("name")?
        .map(|v: String| element.new_name(&v, XsdType::ComplexType)),
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

  #[tracing::instrument(skip_all)]
  pub fn get_implementation(
    &self,
    parent_is_schema: bool,
    parent_name: Option<XsdName>,
    context: &mut XsdContext,
  ) -> Result<XsdImpl, XsdError> {
    let struct_id = self
      .name
      .as_ref()
      .map(|v| XsdName {
        namespace: v.namespace.clone(),
        local_name: v.local_name.clone(),
        ty: XsdType::ComplexType,
      })
      .or_else(|| parent_name);

    let xml_name = struct_id.clone();

    let mut generated_impl = XsdImpl {
      name: struct_id.clone().unwrap(),
      element: XsdElement::Struct(
        Struct::new(&struct_id.unwrap().to_struct_name())
          .vis("pub")
          .to_owned(),
      ),
      fieldname_hint: None,
      implementation: vec![],
      inner: vec![],
    };

    let inner_impl = match (
      &self.complex_content,
      &self.simple_content,
      &self.group,
      &self.sequence,
    ) {
      (Some(complex_content), None, None, None) => {
        Some(complex_content.get_implementation(xml_name.unwrap(), context)?)
      }
      (None, Some(simple_content), None, None) => {
        Some(simple_content.get_implementation(xml_name.unwrap(), context)?)
      }
      (None, None, Some(group), None) => Some(group.get_implementation(xml_name, context)?),
      (None, None, None, Some(sequence)) => Some(sequence.get_implementation(xml_name, context)?),
      (None, None, None, None) => None,
      _ => unreachable!("Xsd is invalid."),
    };

    let mut generated_impls = vec![];

    for attribute in &self.attributes {
      generated_impls.push(attribute.get_implementation(context)?);
    }

    for g in &self.attribute_groups {
      generated_impls.push(g.get_implementation(None, context)?);
    }

    if let Some(inner_impl) = inner_impl {
      generated_impl.merge(inner_impl, MergeSettings::default());
    }

    for i in generated_impls {
      generated_impl.merge(
        i,
        MergeSettings {
          conflict_prefix: Some("attr_"),
          merge_type: crate::xsd::xsd_context::MergeType::Structs,
        },
      );
    }

    let docs = self
      .annotation
      .as_ref()
      .map(|annotation| annotation.get_doc())
      .unwrap_or_default();
    generated_impl.element.add_doc(&docs.join(""));

    generated_impl.name.ty = XsdType::ComplexType;

    Ok(generated_impl)
  }
}
