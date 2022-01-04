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
    xsd_context::{MergeSettings, XsdElement, XsdImpl, XsdName},
    XsdContext,
  },
};
use heck::CamelCase;

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
    let simple_content =
      element.try_get_child_with("simpleContent", |child| SimpleContent::parse(child))?;
    let complex_content =
      element.try_get_child_with("complexContent", |child| ComplexContent::parse(child))?;

    let choice = element.try_get_child_with("choice", |child| Choice::parse(child))?;
    let group = element.try_get_child_with("group", |child| Group::parse(child))?;
    let sequence = element.try_get_child_with("sequence", |child| Sequence::parse(child))?;

    let attributes = element.get_children_with("attribute", |child| Attribute::parse(child))?;

    let attribute_groups =
      element.get_children_with("attributeGroup", |child| AttributeGroup::parse(child))?;

    if simple_content.is_some() && complex_content.is_some() {
      return Err(XsdError::XsdParseError(format!(
        "simpleContent | complexContent cannot both present in {}",
        element.name()
      )));
    }

    if (simple_content.is_some() || complex_content.is_some())
      && (!attributes.is_empty()
        || attribute_groups.is_empty()
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
      annotation: element.try_get_child_with("annotation", |child| Annotation::parse(child))?,
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  pub fn get_implementation(&self, context: &mut XsdContext) -> XsdImpl {
    let name = self.name.clone().unwrap_or("temp".to_string());

    let struct_id = XsdName {
      namespace: None,
      local_name: name.clone(),
    };

    assert!(
      !context.structs.contains_key(&struct_id),
      "Struct {:?} has already been declared.",
      &struct_id
    );

    let struct_name = name.replace(".", "_").to_camel_case();

    let fields = match (
      &self.complex_content,
      &self.simple_content,
      &self.group,
      &self.sequence,
    ) {
      (Some(complex_content), None, None, None) => complex_content.get_implementation(
        XsdName {
          namespace: None,
          local_name: name,
        },
        context,
      ),
      (None, Some(simple_content), None, None) => simple_content.get_implementation(
        XsdName {
          namespace: None,
          local_name: name,
        },
        context,
      ),
      (None, None, Some(group), None) => group.get_implementation(
        Some(XsdName {
          namespace: None,
          local_name: name,
        }),
        context,
      ),
      (None, None, None, Some(sequence)) => sequence.get_implementation(
        XsdName {
          namespace: None,
          local_name: name,
        },
        context,
      ),
      _ => unreachable!("Xsd is invalid."),
    };

    let docs = self
      .annotation
      .as_ref()
      .map(|annotation| annotation.get_doc())
      .unwrap_or_default();

    let mut generated_impl = XsdImpl {
      element: XsdElement::Struct(
        Struct::new(&struct_name)
          .doc(&docs.join("\n"))
          .derive("#[derive(Clone, Debug, Default, PartialEq, YaDeserialize, YaSerialize)]")
          .to_owned(),
      ),
      ..Default::default()
    };

    generated_impl.merge(fields, MergeSettings::default());
    for attribute in &self.attributes {
      if let Some(generated) = attribute.get_implementation(context) {
        generated_impl.merge(
          generated,
          MergeSettings {
            conflict_prefix: Some("attr_"),
          },
        );
      }
    }

    generated_impl
  }
}
