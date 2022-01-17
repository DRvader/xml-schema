use crate::xsd::{
  list::List, restriction::Restriction, union::Union, xsd_context::XsdName, XsdContext,
};

use super::{
  annotation::Annotation, restriction::RestrictionParentType, xsd_context::XsdImpl,
  XMLElementWrapper, XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct SimpleType {
  pub name: Option<String>,
  pub annotation: Option<Annotation>,
  pub restriction: Option<Restriction>,
  pub list: Option<List>,
  pub union: Option<Union>,
}

impl SimpleType {
  pub fn parse(mut element: XMLElementWrapper, parent_is_schema: bool) -> Result<Self, XsdError> {
    element.check_name("simpleType")?;

    let restriction = element.try_get_child_with("restriction", |child| {
      Restriction::parse(RestrictionParentType::SimpleType, child)
    })?;
    let list = element.try_get_child_with("list", List::parse)?;
    let union = element.try_get_child_with("union", Union::parse)?;

    if restriction.is_some() as u8 + list.is_some() as u8 + union.is_some() as u8 > 1 {
      return Err(XsdError::XsdParseError(format!(
        "Two of (extension | restriction | union) cannot be present in {}",
        element.name()
      )));
    }

    let name = element.try_get_attribute("name")?;

    if parent_is_schema && name.is_none() {
      return Err(XsdError::XsdParseError(format!(
        "The name attribute is required if the parent of {} is a schema.",
        element.name()
      )));
    } else if !parent_is_schema && name.is_some() {
      return Err(XsdError::XsdParseError(format!(
        "The name attribute is not allowed if the parent of {} is not a schema.",
        element.name()
      )));
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
  pub fn get_implementation(&self, context: &mut XsdContext) -> Result<XsdImpl, XsdError> {
    let name = XsdName {
      namespace: None,
      local_name: self.name.clone().unwrap_or_else(|| "temp".to_string()),
    };

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

    generated_impl.name = Some(name);

    Ok(generated_impl)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  static DERIVES: &str =
    "# [ derive ( Clone , Debug , Default , PartialEq , YaDeserialize , YaSerialize ) ] ";

  #[test]
  fn simple_type() {
    let st = SimpleType {
      name: Some("test".to_string()),
      annotation: None,
      restriction: None,
      list: None,
      union: None,
    };

    let mut context =
      XsdContext::new(r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#)
        .unwrap();

    let value = st
      .get_implementation(&mut context)
      .unwrap()
      .to_string()
      .unwrap();
    let ts = quote!(#value).to_string();

    assert_eq!(
      format!(
        "{}pub struct Test {{ # [ yaserde ( text ) ] pub content : std :: string :: String , }}",
        DERIVES
      ),
      ts
    );
  }

  // <!-- Whitespace-separated list of strings -->
  // <xs:simpleType name="StringVectorType">
  //   <xs:list itemType="xs:string"/>
  // </xs:simpleType>

  // <!-- Whitespace-separated list of unsigned integers -->
  // <xs:simpleType name="UIntVectorType">
  //   <xs:list itemType="xs:unsignedInt"/>
  // </xs:simpleType>

  // #[test]
  // fn list_type() {
  //   let st = SimpleType {
  //     name: "string-list".to_string(),
  //     restriction: None,
  //     list: Some(List{
  //       item_type: "xs:string".to_string()
  //     }),
  //     union: None,
  //   };

  //   let context = XsdContext {
  //     xml_schema_prefix: Some("xs".to_string()),
  //   };

  //   let ts = st
  //     .get_implementation(&quote!(), &None, &context)
  //     .to_string();
  //   println!("{}", ts);
  //   assert!(ts == format!("{}pub struct StringList {{ # [ yaserde ( text ) ] pub content : String , }}", DERIVES));
  // }
}
