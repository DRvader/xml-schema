use crate::{
  codegen::{Block, Field, Function, Impl, Struct},
  xsd::attribute::Attribute,
};

use super::{
  annotation::Annotation,
  xsd_context::{to_field_name, MergeSettings, XsdContext, XsdElement, XsdImpl, XsdName, XsdType},
  XMLElementWrapper, XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(
//   rename = "attributeGroup",
//   prefix = "xs",
//   namespace = "xs: http://www.w3.org/2001/XMLSchema"
// )]
pub struct AttributeGroup {
  pub name: Option<XsdName>,
  pub reference: Option<XsdName>,
  pub annotation: Option<Annotation>,
  pub attributes: Vec<Attribute>,
  pub attribute_groups: Vec<AttributeGroup>,
}

impl AttributeGroup {
  pub fn parse(mut element: XMLElementWrapper) -> Result<Self, XsdError> {
    element.check_name("attributeGroup")?;

    let name = element
      .try_get_attribute("name")?
      .map(|v: String| element.new_name(&v, XsdType::AttributeGroup));
    let reference = element
      .try_get_attribute("ref")?
      .map(|v: String| element.new_name(&v, XsdType::AttributeGroup));

    if name.is_some() && reference.is_some() {
      return Err(XsdError::XsdParseError(format!(
        "name and ref both present in {}",
        element.name()
      )));
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

  #[tracing::instrument(skip_all)]
  pub fn get_implementation(
    &self,
    parent_name: Option<XsdName>,
    context: &mut XsdContext,
  ) -> Result<XsdImpl, XsdError> {
    // TODO(drosen): We know that both name and reference cannot be some,
    //               but we have no handler for what happens if the parent
    //               name is None.
    let generated_impl = match (&self.name, &self.reference) {
      (None, Some(refers)) => {
        let inner = if let Some(imp) = context.search(refers) {
          imp
        } else {
          return Err(XsdError::XsdImplNotFound(refers.clone()));
        };

        let field_name = if let Some(parent_name) = &parent_name {
          to_field_name(&parent_name.local_name)
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
            Field::new(&field_name, inner.element.get_type())
              .vis("pub")
              .to_owned(),
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
            Struct::new(&xml_name.to_struct_name())
              .vis("pub")
              .to_owned(),
          ),
          inner: vec![],
          implementation: vec![],
        };

        let mut fields = vec![];

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
            if let Some(field) = generated_struct.element.get_last_added_field() {
              fields.push(field);
            }
          } else {
            return Err(XsdError::XsdImplNotFound(reference.clone()));
          }
        }

        for attr in &self.attributes {
          generated_struct.merge(attr.get_implementation(context)?, MergeSettings::ATTRIBUTE);
          if let Some(field) = generated_struct.element.get_last_added_field() {
            fields.push(field);
          }
        }

        for attr in &self.attribute_groups {
          generated_struct.merge(
            attr.get_implementation(parent_name.clone(), context)?,
            MergeSettings::default(),
          );
          if let Some(field) = generated_struct.element.get_last_added_field() {
            fields.push(field);
          }
        }

        if let Some(doc) = &self.annotation {
          generated_struct.element.add_doc(&doc.get_doc().join(""));
        }

        let mut r#impl = Impl::new(generated_struct.element.get_type());

        let mut parse = Function::new("parse")
          .arg("mut element", "XMLElementWrapper")
          .ret("Result<Self, XsdError>");

        let mut block = Block::new("let output = Self").after(";").to_owned();
        for (field, ty) in fields {
          block = block.line(&format!("{}: XsdParse::parse(element)?,", field));
        }

        r#impl = r#impl.impl_trait("XsdParse").push_fn(
          parse
            .push_block(block)
            .line("element.finalize(false, false)?;")
            .line("Ok(output)"),
        );

        generated_struct.implementation.push(r#impl);

        Ok(generated_struct)
      }
      _ => unreachable!("The Xsd is invalid!"),
    };

    if let Ok(mut gen) = generated_impl {
      gen.name.ty = XsdType::AttributeGroup;
      Ok(gen)
    } else {
      generated_impl
    }
  }
}
