use xsd_codegen::{fromxml_impl, Block, Struct, XMLElement};
use xsd_types::{XsdIoError, XsdName, XsdType};

use crate::xsd::XsdContext;

use super::{
  xsd_context::{XsdElement, XsdImpl},
  XsdError,
};

#[derive(Clone, Debug, PartialEq)]
pub struct List {
  pub item_type: XsdName,
}

impl List {
  pub fn parse(mut element: XMLElement) -> Result<Self, XsdIoError> {
    element.check_name("list")?;

    let item_type: String = element.get_attribute("itemType")?;

    let output = Self {
      item_type: element.new_name(&item_type, XsdType::SimpleType),
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  #[tracing::instrument(skip_all)]
  pub fn get_implementation(
    &self,
    name: XsdName,
    context: &mut XsdContext,
  ) -> Result<XsdImpl, XsdError> {
    let struct_name = name.to_struct_name();
    let inner = if let Some(imp) = context.search(&self.item_type) {
      imp
    } else {
      return Err(XsdError::XsdImplNotFound(self.item_type.clone()));
    };

    let list_type = inner.element.get_type().to_string();

    let generated_struct = Struct::new(Some(name.clone()), &struct_name)
      .vis("pub")
      .tuple_field(format!("Vec<{}>", list_type))
      .derives(&["Clone", "Debug", "Default", "PartialEq"]);

    let from_xml = fromxml_impl(
      generated_struct.ty().clone(),
      Block::new("")
        .line("let output = element.get_content()?.split(' ').map(|item| item.from_xml(item)).collect();")
        .line(format!("Ok({struct_name}(output))")),
    );

    Ok(XsdImpl {
      name: XsdName {
        ty: XsdType::List,
        ..name.clone()
      },
      fieldname_hint: Some(name.to_field_name()),
      element: XsdElement::Struct(generated_struct.clone()),
      inner: vec![],
      implementation: vec![from_xml],
    })
  }
}
