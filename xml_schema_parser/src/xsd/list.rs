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
    element.check_name("xs:list")?;

    let output = Self {
      item_type: element.get_attribute("itemType")?,
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  pub fn get_implementation(&self, name: XsdName, context: &mut XsdContext) -> XsdImpl {
    let list_type = RustTypesMapping::get(context, &self.item_type);

    let mut generated_struct = Struct::new(&name.local_name);
    generated_struct.push_field(Field::new("items", format!("Vec<{}>", list_type)));
    for derive in ["Clone", "Debug", "Default", "PartialEq"] {
      generated_struct.derive(derive);
    }

    let mut deserialize = Function::new("deserialize")
      .bound("R", "Read")
      .arg("reader", "&mut yaserde::de::Deserializer<R>")
      .ret("Result<Self, String>")
      .to_owned();
    deserialize.body = Some(vec![Body::String(
      r#"
loop {
  match reader.next_event()? {
    xml::reader::XmlEvent::StartElement{..} => {}
    xml::reader::XmlEvent::Characters(ref text_content) => {
      let items: Vec<#list_type> =
        text_content
          .split(' ')
          .map(|item| item.to_owned())
          .map(|item| item.parse().unwrap())
          .collect();

      return Ok(#struct_name {items});
    }
    _ => {break;}
  }
}

Err("Unable to parse attribute".to_string())
"#
      .to_string(),
    )]);

    let mut serialize = Function::new("serialize")
      .bound("R", "Read")
      .arg_ref_self()
      .arg("writer", "&mut yaserde::ser::Serializer<W>")
      .ret("Result<(), String>")
      .to_owned();
    serialize.body = Some(vec![Body::String(
      r#"
let content =
  self.items.iter().map(|item| item.to_string()).collect::<Vec<String>>().join(" ");

let data_event = xml::writer::XmlEvent::characters(&content);
writer.write(data_event).map_err(|e| e.to_string())?;

Ok(())
"#
      .to_string(),
    )]);

    let mut serialize_attributes = Function::new("serialize_attributes")
      .bound("R", "Read")
      .arg_ref_self()
      .arg(
        "mut source_attributes",
        "Vec<xml::attribute::OwnedAttribute>",
      )
      .arg("mut source_namespace", "xml::namespace::Namespace")
      .ret(
        r#"Result<
        (
          Vec<xml::attribute::OwnedAttribute>,
          xml::namespace::Namespace,
        ),
        String,
      >"#,
      )
      .to_owned();
    serialize_attributes.body = Some(vec![Body::String(
      "Ok((source_attributes, source_namespace))".to_string(),
    )]);

    XsdImpl {
      name: Some(name),
      element: XsdElement::Struct(generated_struct.clone()),
      inner: vec![],
      implementation: vec![
        Impl::new(generated_struct.ty())
          .impl_trait("YaDeserialize")
          .push_fn(deserialize)
          .to_owned(),
        Impl::new(generated_struct.ty())
          .impl_trait("YaSerialize")
          .push_fn(serialize)
          .push_fn(serialize_attributes)
          .to_owned(),
      ],
    }
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
      .to_string()
      .unwrap();
    let implementation = quote!(#value).to_string();

    assert_eq!(
      implementation,
      r#"# [ derive ( Clone , Debug , Default , PartialEq ) ] pub struct Parent { items : Vec < String > } impl YaDeserialize for Parent { fn deserialize < R : Read > ( reader : & mut yaserde :: de :: Deserializer < R > ) -> Result < Self , String > { loop { match reader . next_event ( ) ? { xml :: reader :: XmlEvent :: StartElement { .. } => { } xml :: reader :: XmlEvent :: Characters ( ref text_content ) => { let items : Vec < String > = text_content . split ( ' ' ) . map ( | item | item . to_owned ( ) ) . map ( | item | item . parse ( ) . unwrap ( ) ) . collect ( ) ; return Ok ( Parent { items } ) ; } _ => { break ; } } } Err ( "Unable to parse attribute" . to_string ( ) ) } } impl YaSerialize for Parent { fn serialize < W : Write > ( & self , writer : & mut yaserde :: ser :: Serializer < W > ) -> Result < ( ) , String > { let content = self . items . iter ( ) . map ( | item | item . to_string ( ) ) . collect :: < Vec < String >> ( ) . join ( " " ) ; let data_event = xml :: writer :: XmlEvent :: characters ( & content ) ; writer . write ( data_event ) . map_err ( | e | e . to_string ( ) ) ? ; Ok ( ( ) ) } fn serialize_attributes ( & self , mut source_attributes : Vec < xml :: attribute :: OwnedAttribute > , mut source_namespace : xml :: namespace :: Namespace ) -> Result < ( Vec < xml :: attribute :: OwnedAttribute > , xml :: namespace :: Namespace ) , String > { Ok ( ( source_attributes , source_namespace ) ) } }"#
    );
  }
}
