use crate::{
  codegen::Struct,
  xsd::{attribute::Attribute, sequence::Sequence, XsdContext},
};

use super::{
  annotation::Annotation,
  attribute_group::AttributeGroup,
  choice::Choice,
  group::Group,
  xsd_context::{MergeSettings, XsdElement, XsdImpl, XsdName},
  XMLElementWrapper, XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(
//   root = "extension",
//   prefix = "xs",
//   namespace = "xs: http://www.w3.org/2001/XMLSchema"
// )]
pub struct Extension {
  pub base: String,
  pub attributes: Vec<Attribute>,
  pub attribute_groups: Vec<AttributeGroup>,
  pub sequence: Option<Sequence>,
  pub group: Option<Group>,
  pub choice: Option<Choice>,
  pub annotation: Option<Annotation>,
}

impl Extension {
  pub fn parse(mut element: XMLElementWrapper) -> Result<Self, XsdError> {
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
      return Err(XsdError::XsdParseError(format!(
        "(group | choice | sequence) and (attribute | attributeGroup) cannot both present in {}",
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
      base: element.get_attribute("base")?,
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
    let mut generated_impl = match (&self.group, &self.sequence, &self.choice) {
      (None, None, Some(choice)) => choice.get_implementation(Some(parent_name), context),
      (None, Some(sequence), None) => sequence.get_implementation(Some(parent_name), context),
      (Some(group), None, None) => group.get_implementation(Some(parent_name), context),
      (None, None, None) => Ok(XsdImpl {
        name: parent_name.clone(),
        fieldname_hint: Some(parent_name.to_field_name()),
        element: XsdElement::Struct(
          Struct::new(&parent_name.to_struct_name())
            .vis("pub")
            .to_owned(),
        ),
        inner: vec![],
        implementation: vec![],
      }),
      _ => unreachable!("Error parsing {}, Invalid XSD!", &parent_name.local_name),
    }?;

    for attribute in &self.attributes {
      if let Some(attribute) = attribute.get_implementation(context)? {
        generated_impl.merge(attribute, MergeSettings::ATTRIBUTE);
      }
    }

    Ok(generated_impl)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extension() {
    let st = Extension {
      base: "xs:string".to_string(),
      ..Default::default()
    };

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    let value = st
      .get_implementation(XsdName::new("test"), &mut context)
      .unwrap()
      .to_string()
      .unwrap();
    let ts = quote!(#value).to_string();
    assert!(ts == "# [ yaserde ( text ) ] pub content : String ,");
  }

  #[test]
  fn extension_with_attributes() {
    use crate::xsd::attribute::Required;

    let st = Extension {
      base: "xs:string".to_string(),
      attributes: vec![
        Attribute {
          name: Some("attribute_1".to_string()),
          kind: Some("xs:string".to_string()),
          default: None,
          fixed: None,
          reference: None,
          required: Required::Required,
          simple_type: None,
        },
        Attribute {
          name: Some("attribute_2".to_string()),
          kind: Some("xs:boolean".to_string()),
          default: None,
          fixed: None,
          reference: None,
          required: Required::Optional,
          simple_type: None,
        },
      ],
      ..Default::default()
    };

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    let value = st
      .get_implementation(XsdName::new("test"), &mut context)
      .unwrap()
      .to_string()
      .unwrap();
    let ts = quote!(#value).to_string();
    assert!(ts == "struct Test { # [ yaserde ( text ) ] pub content : String , # [ yaserde ( attribute ) ] pub attribute_1 : String , # [ yaserde ( attribute ) ] pub attribute_2 : Option < bool > , }");
  }
}
