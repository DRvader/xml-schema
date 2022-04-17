use xsd_codegen::XMLElement;
use xsd_types::{XsdName, XsdParseError, XsdType};

use crate::xsd::{list::List, restriction::Restriction, union::Union, XsdContext};

use super::{
  annotation::Annotation, restriction::RestrictionParentType, xsd_context::XsdImpl, XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
pub struct SimpleType {
  pub name: Option<XsdName>,
  pub annotation: Option<Annotation>,
  pub restriction: Option<Restriction>,
  pub list: Option<List>,
  pub union: Option<Union>,
}

impl SimpleType {
  pub fn parse(mut element: XMLElement, parent_is_schema: bool) -> Result<Self, XsdParseError> {
    element.check_name("simpleType")?;

    let restriction = element.try_get_child_with("restriction", |child| {
      Restriction::parse(RestrictionParentType::SimpleType, child)
    })?;
    let list = element.try_get_child_with("list", List::parse)?;
    let union = element.try_get_child_with("union", Union::parse)?;

    if restriction.is_some() as u8 + list.is_some() as u8 + union.is_some() as u8 > 1 {
      return Err(XsdParseError {
        node_name: element.node_name(),
        msg: format!("Two of (extension | restriction | union) cannot be present"),
      });
    }

    let name = element
      .try_get_attribute("name")?
      .map(|v: String| element.new_name(&v, XsdType::SimpleType));

    if parent_is_schema && name.is_none() {
      return Err(XsdParseError {
        node_name: element.node_name(),
        msg: format!("The name attribute is required if the parent node is a schema.",),
      });
    } else if !parent_is_schema && name.is_some() {
      return Err(XsdParseError {
        node_name: element.node_name(),
        msg: format!("The name attribute is not allowed if the parent of node is not a schema.",),
      });
    }

    let output = Self {
      name,
      annotation: element.try_get_child_with("annotation", Annotation::parse)?,
      restriction,
      list,
      union,
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
    let name = self.name.clone().unwrap_or_else(|| {
      let mut parent = parent_name.unwrap();
      parent.ty = XsdType::SimpleType;
      parent
    });

    let mut generated_impl = match (&self.list, &self.union, &self.restriction) {
      (None, None, Some(restriction)) => {
        restriction.get_implementation(name.clone(), RestrictionParentType::SimpleType, context)
      }
      (None, Some(union), None) => union.get_implementation(name.clone(), context),
      (Some(list), None, None) => list.get_implementation(name.clone(), context),
      _ => unreachable!("Invalid Xsd!"),
    }?;

    if let Some(doc) = &self.annotation {
      generated_impl.element.add_doc(&doc.get_doc().join(""));
    }

    generated_impl.name = name;
    generated_impl.name.ty = XsdType::SimpleType;

    Ok(generated_impl)
  }
}
