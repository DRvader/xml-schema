use crate::{codegen::Struct, xsd::attribute::Attribute};

use super::{
  xsd_context::{MergeSettings, XsdContext, XsdElement, XsdImpl, XsdName},
  XMLElementWrapper, XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(
//   rename = "attributeGroup",
//   prefix = "xs",
//   namespace = "xs: http://www.w3.org/2001/XMLSchema"
// )]
pub struct AttributeGroup {
  pub name: Option<String>,
  pub reference: Option<String>,
  pub attributes: Vec<Attribute>,
  pub attribute_groups: Vec<AttributeGroup>,
}

impl AttributeGroup {
  pub fn parse(mut element: XMLElementWrapper) -> Result<Self, XsdError> {
    element.check_name("xs:attributeGroup");

    let name = element.try_get_attribute("name")?;
    let reference = element.try_get_attribute("ref")?;

    if name.is_some() && reference.is_some() {
      return Err(XsdError::XsdParseError(format!(
        "name and ref both present in {}",
        element.name()
      )));
    }

    let attributes = element.get_children_with("xs:attribute", |child| Attribute::parse(child))?;
    let attribute_groups =
      element.get_children_with("xs:attributeGroup", |child| AttributeGroup::parse(child))?;

    let output = Ok(Self {
      name,
      reference,
      attributes,
      attribute_groups,
    });

    element.finalize(false, false);

    output
  }

  pub fn get_implementation(
    &self,
    parent_name: Option<XsdName>,
    context: &mut XsdContext,
  ) -> XsdImpl {
    let mut generated_struct = XsdImpl {
      element: XsdElement::Struct(Struct::new(
        &self
          .name
          .as_ref()
          .unwrap_or(&parent_name.as_ref().unwrap().to_struct_name()),
      )),
      ..Default::default()
    };

    for attr in &self.attributes {
      if let Some(attr) = attr.get_implementation(context) {
        generated_struct.merge(attr, MergeSettings::default());
      }
    }

    for attr in &self.attribute_groups {
      generated_struct.merge(
        attr.get_implementation(parent_name.clone(), context),
        MergeSettings::default(),
      );
    }

    generated_struct
  }
}