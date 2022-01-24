use crate::{
  codegen::{Body, Field, Function, Impl, Struct},
  xsd::{rust_types_mapping::RustTypesMapping, XsdContext},
};

use super::{
  xsd_context::{XsdElement, XsdImpl, XsdName},
  XMLElementWrapper, XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct List {
  pub item_type: String,
}

impl List {
  pub fn parse(mut element: XMLElementWrapper) -> Result<Self, XsdError> {
    element.check_name("list")?;

    let output = Self {
      item_type: element.get_attribute("itemType")?,
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
    let list_type = RustTypesMapping::get(context, &self.item_type);

    let struct_name = name.to_struct_name();

    let mut generated_struct = Struct::new(&struct_name).vis("pub").to_owned();
    generated_struct.tuple_field(format!("Vec<{}>", list_type));
    for derive in ["Clone", "Debug", "Default", "PartialEq"] {
      generated_struct.derive(derive);
    }

    let mut parse_fn = Function::new("parse")
      .arg("mut element", "XsdElementWrapper")
      .ret("Result<Self, XsdError>")
      .vis("pub")
      .to_owned();

    parse_fn.line(format!("let output: Vec<{list_type}> = element.get_content()?.split(' ').map(|item| item.to_owned()).map(|item| item.parse().unwrap()).collect();"));

    parse_fn.line("element.finalize(false, false)?;");
    parse_fn.line(format!("Ok({struct_name}(output))"));

    Ok(XsdImpl {
      name: name.clone(),
      fieldname_hint: Some(name.to_field_name()),
      element: XsdElement::Struct(generated_struct.clone()),
      inner: vec![],
      implementation: vec![Impl::new(generated_struct.ty())
        .push_fn(parse_fn)
        .to_owned()],
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn basic_list() {
    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    let list_type = List {
      item_type: "xs:string".to_string(),
    };

    let value = list_type
      .get_implementation(XsdName::new("parent"), &mut context)
      .unwrap()
      .to_string()
      .unwrap();
    let implementation = quote!(#value).to_string();

    assert_eq!(
      implementation,
      r#"# [ derive ( Clone , Debug , Default , PartialEq ) ] pub struct Parent { items : Vec < String > } impl YaDeserialize for Parent { fn deserialize < R : Read > ( reader : & mut yaserde :: de :: Deserializer < R > ) -> Result < Self , String > { loop { match reader . next_event ( ) ? { xml :: reader :: XmlEvent :: StartElement { .. } => { } xml :: reader :: XmlEvent :: Characters ( ref text_content ) => { let items : Vec < String > = text_content . split ( ' ' ) . map ( | item | item . to_owned ( ) ) . map ( | item | item . parse ( ) . unwrap ( ) ) . collect ( ) ; return Ok ( Parent { items } ) ; } _ => { break ; } } } Err ( "Unable to parse attribute" . to_string ( ) ) } } impl YaSerialize for Parent { fn serialize < W : Write > ( & self , writer : & mut yaserde :: ser :: Serializer < W > ) -> Result < ( ) , String > { let content = self . items . iter ( ) . map ( | item | item . to_string ( ) ) . collect :: < Vec < String >> ( ) . join ( " " ) ; let data_event = xml :: writer :: XmlEvent :: characters ( & content ) ; writer . write ( data_event ) . map_err ( | e | e . to_string ( ) ) ? ; Ok ( ( ) ) } fn serialize_attributes ( & self , mut source_attributes : Vec < xml :: attribute :: OwnedAttribute > , mut source_namespace : xml :: namespace :: Namespace ) -> Result < ( Vec < xml :: attribute :: OwnedAttribute > , xml :: namespace :: Namespace ) , String > { Ok ( ( source_attributes , source_namespace ) ) } }"#
    );
  }
}
