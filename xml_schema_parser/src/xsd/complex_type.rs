use xsd_codegen::{Struct, XMLElement};
use xsd_types::{XsdIoError, XsdName, XsdParseError, XsdType};

use super::{
  annotation::Annotation,
  attribute::Attribute,
  attribute_group::AttributeGroup,
  choice::Choice,
  complex_content::ComplexContent,
  general_xsdgen,
  group::Group,
  sequence::Sequence,
  simple_content::SimpleContent,
  xsd_context::{MergeSettings, XsdImpl, XsdImplType},
  XsdContext, XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
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
  pub fn parse(mut element: XMLElement) -> Result<Self, XsdIoError> {
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
      return Err(XsdIoError::XsdParseError(XsdParseError {
        node_name: element.node_name(),
        msg: "simpleContent | complexContent cannot both present".to_string(),
      }));
    }

    if (simple_content.is_some() || complex_content.is_some())
      && (!attributes.is_empty()
        || !attribute_groups.is_empty()
        || group.is_some()
        || choice.is_some()
        || sequence.is_some())
    {
      return Err(XsdIoError::XsdParseError(XsdParseError {node_name: element.node_name(), msg: "(simpleContent | complexContent) and (group | choice | sequence | attribute | attributeGroup) cannot both present".to_string()}));
    }

    if group.is_some() as u8 + choice.is_some() as u8 + sequence.is_some() as u8 > 1 {
      return Err(XsdIoError::XsdParseError(XsdParseError {
        node_name: element.node_name(),
        msg: "group | choice | sequence cannot all be present".to_string(),
      }));
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
      .or(parent_name);

    let xml_name = struct_id.clone();

    let mut generated_impl = XsdImpl {
      name: struct_id.clone().unwrap(),
      element: XsdImplType::Struct(
        Struct::new(xml_name.clone(), &struct_id.unwrap().to_struct_name())
          .vis("pub")
          .derives(&["Clone", "Debug", "PartialEq"]),
      ),
      fieldname_hint: None,
      implementation: vec![],
      inner: vec![],
      flatten: false,
    };

    let inner_impl = match (
      &self.complex_content,
      &self.simple_content,
      &self.group,
      &self.sequence,
      &self.choice,
    ) {
      (Some(complex_content), None, None, None, None) => {
        Some(complex_content.get_implementation(xml_name.unwrap(), context)?)
      }
      (None, Some(simple_content), None, None, None) => {
        Some(simple_content.get_implementation(xml_name.unwrap(), context)?)
      }
      (None, None, Some(group), None, None) => Some(group.get_implementation(xml_name, context)?),
      (None, None, None, Some(sequence), None) => {
        Some(sequence.get_implementation(xml_name, context)?)
      }
      (None, None, None, None, Some(choice)) => Some(choice.get_implementation(xml_name, context)?),
      (None, None, None, None, None) => None,
      _ => unreachable!("Xsd is invalid."),
    };

    let mut generated_impls = vec![];

    for attribute in &self.attributes {
      generated_impls.push(attribute.get_implementation(context, false)?);
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
          merge_type: super::xsd_context::MergeType::Field,
        },
      );
    }

    if let Some(docs) = &self.annotation {
      generated_impl.element.add_doc(&docs.get_doc().join(""));
    }

    generated_impl.name.ty = XsdType::ComplexType;

    Ok(general_xsdgen(generated_impl))
  }
}
