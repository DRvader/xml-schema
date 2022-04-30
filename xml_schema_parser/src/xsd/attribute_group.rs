use xsd_codegen::{Field, Struct, XMLElement};
use xsd_types::{XsdIoError, XsdName, XsdParseError, XsdType};

use crate::xsd::attribute::Attribute;

use super::{
  annotation::Annotation,
  general_xsdgen,
  xsd_context::{MergeSettings, XsdContext, XsdElement, XsdImpl},
  XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
pub struct AttributeGroup {
  pub name: Option<XsdName>,
  pub reference: Option<XsdName>,
  pub annotation: Option<Annotation>,
  pub attributes: Vec<Attribute>,
  pub attribute_groups: Vec<AttributeGroup>,
}

impl AttributeGroup {
  pub fn parse(mut element: XMLElement) -> Result<Self, XsdIoError> {
    element.check_name("attributeGroup")?;

    let name = element
      .try_get_attribute("name")?
      .map(|v: String| element.new_name(&v, XsdType::AttributeGroup));
    let reference = element
      .try_get_attribute("ref")?
      .map(|v: String| element.new_name(&v, XsdType::AttributeGroup));

    if name.is_some() && reference.is_some() {
      return Err(XsdIoError::XsdParseError(XsdParseError {
        node_name: element.node_name(),
        msg: format!("name and ref both present"),
      }));
    }

    let attributes = element.get_children_with("attribute", Attribute::parse)?;
    let attribute_groups = element.get_children_with("attributeGroup", AttributeGroup::parse)?;

    let output = Ok(Self {
      name,
      reference,
      annotation: element.try_get_child_with("annotation", Annotation::parse)?,
      attributes,
      attribute_groups,
    });

    element.finalize(false, false)?;

    output
  }

  fn create_type(
    &self,
    parent_name: Option<XsdName>,
    context: &mut XsdContext,
  ) -> Result<XsdImpl, XsdError> {
    // TODO(drosen): We know that both name and reference cannot be some,
    //               but we have no handler for what happens if the parent
    //               name is None.
    match (&self.name, &self.reference) {
      (None, Some(refers)) => {
        let inner = if let Some(imp) = context.search(refers) {
          imp
        } else {
          return Err(XsdError::XsdImplNotFound(refers.clone()));
        };

        let field_name = if let Some(parent_name) = &parent_name {
          parent_name.to_field_name()
        } else if let Some(field_hint) = &inner.fieldname_hint {
          field_hint.clone()
        } else {
          refers.to_field_name()
        };

        let name = if let Some(parent_name) = parent_name {
          parent_name
        } else {
          XsdName {
            namespace: None,
            local_name: inner.infer_type_name(),
            ty: XsdType::AttributeGroup,
          }
        };

        Ok(XsdImpl {
          name,
          element: XsdElement::Field(
            Field::new(None, &field_name, inner.element.get_type(), true).vis("pub"),
          ),
          fieldname_hint: Some(field_name.to_string()),
          inner: vec![],
          implementation: vec![],
        })
      }
      (_, None) => {
        let xml_name = self
          .name
          .clone()
          .unwrap_or_else(|| parent_name.as_ref().unwrap().clone())
          .clone();

        let mut generated_struct = XsdImpl {
          name: xml_name.clone(),
          fieldname_hint: Some(xml_name.to_field_name()),
          element: XsdElement::Struct(
            Struct::new(Some(xml_name.clone()), &xml_name.to_struct_name())
              .vis("pub")
              .derives(&["Clone", "Debug", "PartialEq"]),
          ),
          inner: vec![],
          implementation: vec![],
        };

        if let Some(reference) = &self.reference {
          // We are using a reference as a base so load the reference
          if let Some(imp) = context.search(&reference) {
            let value = XsdImpl {
              name: reference.clone(),
              fieldname_hint: Some(reference.to_field_name()),
              element: XsdElement::Type(imp.element.get_type()),
              inner: vec![],
              implementation: vec![],
            };
            generated_struct.merge(value, MergeSettings::default());
          } else {
            return Err(XsdError::XsdImplNotFound(reference.clone()));
          }
        }

        for attr in &self.attributes {
          generated_struct.merge(
            attr.get_implementation(context, false)?,
            MergeSettings::ATTRIBUTE,
          );
        }

        for attr in &self.attribute_groups {
          generated_struct.merge(
            attr.get_implementation(parent_name.clone(), context)?,
            MergeSettings::default(),
          );
        }

        if let Some(doc) = &self.annotation {
          generated_struct.element.add_doc(&doc.get_doc().join(""));
        }

        Ok(generated_struct)
      }
      _ => {
        unreachable!("Should have already checked that name and ref are not set together.");
      }
    }
  }

  #[tracing::instrument(skip_all)]
  pub fn get_implementation(
    &self,
    parent_name: Option<XsdName>,
    context: &mut XsdContext,
  ) -> Result<XsdImpl, XsdError> {
    let generated_impl = self.create_type(parent_name, context)?;

    let mut gen = general_xsdgen(generated_impl);

    gen.name.ty = XsdType::AttributeGroup;

    Ok(gen)
  }
}
