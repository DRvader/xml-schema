use crate::{codegen::Struct, xsd::attribute::Attribute};

use super::{
  annotation::Annotation,
  xsd_context::{to_struct_name, MergeSettings, XsdContext, XsdElement, XsdImpl, XsdName},
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
  pub annotation: Option<Annotation>,
  pub attributes: Vec<Attribute>,
  pub attribute_groups: Vec<AttributeGroup>,
}

impl AttributeGroup {
  pub fn parse(mut element: XMLElementWrapper) -> Result<Self, XsdError> {
    element.check_name("attributeGroup")?;

    let name = element.try_get_attribute("name")?;
    let reference = element.try_get_attribute("ref")?;

    if name.is_some() && reference.is_some() {
      return Err(XsdError::XsdParseError(format!(
        "name and ref both present in {}",
        element.name()
      )));
    }

    let attributes = element.get_children_with("attribute", |child| Attribute::parse(child))?;
    let attribute_groups =
      element.get_children_with("attributeGroup", |child| AttributeGroup::parse(child))?;

    let output = Ok(Self {
      name,
      reference,
      annotation: element.try_get_child_with("annotation", |child| Annotation::parse(child))?,
      attributes,
      attribute_groups,
    });

    element.finalize(false, false)?;

    output
  }

  pub fn get_implementation(
    &self,
    parent_name: Option<XsdName>,
    context: &mut XsdContext,
  ) -> Result<XsdImpl, XsdError> {
    // TODO(drosen): We know that both name and reference cannot be some,
    //               but we have no handler for what happens if the parent
    //               name is None.
    let xml_name = self
      .name
      .as_ref()
      .or_else(|| self.reference.as_ref())
      .unwrap_or_else(|| &parent_name.as_ref().unwrap().local_name);

    let mut generated_struct = XsdImpl {
      name: Some(XsdName::new(xml_name)), // Could be a child of schema so set the name.
      element: XsdElement::Struct(Struct::new(&to_struct_name(xml_name))),
      ..Default::default()
    };

    if let Some(reference) = &self.reference {
      let name = XsdName::new(reference);
      // We are using a reference as a base so load the reference
      if let Some(imp) = context.structs.get(&name) {
        generated_struct.merge(imp.clone(), MergeSettings::default());
      } else {
        return Err(XsdError::XsdImplNotFound(name));
      }
    }

    for attr in &self.attributes {
      if let Some(attr) = attr.get_implementation(context)? {
        generated_struct.merge(attr, MergeSettings::default());
      }
    }

    for attr in &self.attribute_groups {
      generated_struct.merge(
        attr.get_implementation(parent_name.clone(), context)?,
        MergeSettings::default(),
      );
    }

    Ok(generated_struct)
  }
}
