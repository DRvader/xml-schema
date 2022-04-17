use xsd_codegen::{Function, Impl, Struct, XMLElement};
use xsd_types::{XsdName, XsdParseError, XsdType};

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
  pub fn parse(mut element: XMLElement) -> Result<Self, XsdParseError> {
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

    let mut generated_struct = Struct::new(&struct_name).vis("pub").to_owned();
    generated_struct.tuple_field(format!("Vec<{}>", list_type));
    for derive in ["Clone", "Debug", "Default", "PartialEq"] {
      generated_struct.derive(derive);
    }

    let parse_fn = Function::new("parse")
      .arg("mut element", "XsdElementWrapper")
      .ret("Result<Self, XsdError>")
      .vis("pub").line(format!("let output: Vec<{list_type}> = element.get_content()?.split(' ').map(|item| item.to_owned()).map(|item| item.parse().unwrap()).collect();")).line("element.finalize(false, false)?;").line(format!("Ok({struct_name}(output))"));

    Ok(XsdImpl {
      name: XsdName {
        ty: XsdType::List,
        ..name.clone()
      },
      fieldname_hint: Some(name.to_field_name()),
      element: XsdElement::Struct(generated_struct.clone()),
      inner: vec![],
      implementation: vec![Impl::new(generated_struct.ty())
        .push_fn(parse_fn)
        .to_owned()],
    })
  }
}
