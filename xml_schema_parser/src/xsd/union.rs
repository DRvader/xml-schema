use xsd_codegen::{Enum, XMLElement};
use xsd_types::{XsdIoError, XsdName, XsdType};

use super::{
  general_xsdgen,
  simple_type::SimpleType,
  xsd_context::{MergeSettings, XsdContext, XsdElement, XsdImpl},
  XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Union {
  pub member_types: Vec<XsdName>,
  pub simple_types: Vec<SimpleType>,
}

impl Union {
  pub fn parse(mut element: XMLElement) -> Result<Self, XsdIoError> {
    element.check_name("union")?;

    let member_types: Option<String> = element.try_get_attribute("memberTypes")?;
    let mut members = vec![];

    if let Some(member_types) = member_types {
      for member in member_types.split_whitespace() {
        members.push(element.new_name(member, XsdType::SimpleType));
      }
    }

    let output = Self {
      member_types: members,
      simple_types: element
        .get_children_with("simpleType", |child| SimpleType::parse(child, false))?,
    };

    element.finalize(false, false)?;

    Ok(output)
  }

  #[tracing::instrument(skip_all)]
  pub fn get_implementation(
    &self,
    parent_name: XsdName,
    context: &mut XsdContext,
  ) -> Result<XsdImpl, XsdError> {
    let mut xml_name = parent_name.clone();
    xml_name.ty = XsdType::Union;

    let mut generated_impl = XsdImpl {
      fieldname_hint: Some(xml_name.to_field_name()),
      name: xml_name.clone(),
      element: XsdElement::Enum(
        Enum::new(Some(xml_name.clone()), &xml_name.to_struct_name())
          .vis("pub")
          .derives(&["Clone", "Debug", "PartialEq"]),
      ),
      implementation: vec![],
      inner: vec![],
      flatten: false,
    };

    for member in &self.member_types {
      if let Some(imp) = context.search(member) {
        generated_impl.merge(imp.to_field(), MergeSettings::default());
      } else {
        return Err(XsdError::XsdImplNotFound(parent_name));
      }
    }

    for member in &self.simple_types {
      generated_impl.merge(
        member.get_implementation(Some(parent_name.clone()), context)?,
        MergeSettings::default(),
      );
    }

    Ok(general_xsdgen(generated_impl))
  }
}

//   #[tracing::instrument(skip_all)]
//   pub fn get_implementation(
//     &self,
//     mut parent_name: XsdName,
//     context: &mut XsdContext,
//   ) -> Result<XsdImpl, XsdError> {
//     let mut generated_enum = Enum::new(&parent_name.to_struct_name())
//       .vis("pub")
//       .to_owned();
//     for derive in ["Clone", "Debug", "PartialEq"] {
//       generated_enum.derive(derive);
//     }

//     let mut output = Block::new("let output = ").after(";");

//     let mut names = Vec::new();
//     for (index, member) in self.member_types.iter().enumerate() {
//       if let Some(imp) = context.search(&member) {
//         let st_name = to_struct_name(&imp.element.get_type().name);
//         generated_enum
//           .new_variant(&st_name)
//           .tuple(imp.element.get_type());
//         output = output.line(format!(
//           "let gen_{}: Option<{}> = element.try_get_content()?;",
//           index,
//           imp.element.get_type().name
//         ));
//         names.push(st_name);
//       } else {
//         return Err(XsdError::XsdImplNotFound(parent_name));
//       }
//     }

//     let mut inner_impl = vec![];
//     for (index, member) in self.simple_types.iter().enumerate() {
//       let index = index + self.member_types.len();

//       let inner = member.get_implementation(Some(parent_name.clone()), context)?;

//       generated_enum
//         .new_variant(&to_struct_name(
//           &inner
//             .fieldname_hint
//             .clone()
//             .unwrap_or_else(|| inner.name.to_struct_name()),
//         ))
//         .tuple(inner.element.get_type().path(&inner.name.to_field_name()));
//       output = output.line(format!(
//         "let gen_{}: Option<{}> = element.try_get_content()?;",
//         index,
//         inner
//           .element
//           .get_type()
//           .path(&inner.name.to_field_name())
//           .to_string()
//       ));
//       names.push(inner.element.get_type().to_string());

//       inner_impl.push(inner);
//     }

//     let mut match_block = Block::new(&format!(
//       "match ({})",
//       (0..names.len())
//         .map(|i| format!("gen_{}", i))
//         .collect::<Vec<_>>()
//         .join(", ")
//     ));
//     for index in 0..names.len() {
//       match_block = match_block.line(&format!(
//         "({}) => Self::{}(value),",
//         (0..names.len())
//           .map(|i| if i == index { "Some(value)" } else { "None" })
//           .collect::<Vec<_>>()
//           .join(", "),
//         names[index]
//       ));
//     }
//     match_block = match_block.push_block(Block::new(&format!(
//       "({}) => ",
//       (0..names.len())
//         .map(|_| "None")
//         .collect::<Vec<_>>()
//         .join(", "))).line("return Err(XsdError::XsdGenError { node_name: element.name().to_string(), msg: format!(\"No valid values could be parsed.\") });").to_owned()
//     );
//     match_block = match_block.line("_ => { return Err(XsdError::XsdGenError { node_name: element.name().to_string(), msg: format!(\"Multiple values were able to be parsed.\") }); }");

//     output = output.push_block(match_block);

//     let r#impl = Impl::new(generated_enum.ty())
//       .push_fn(
//         Function::new("parse_content")
//           .arg("mut element", "XMLElementWrapper")
//           .ret("Result<Self, XsdError>")
//           .push_block(output)
//           .line("element.finalize(false, false)?;")
//           .line("Ok(output)"),
//       )
//       .push_fn(
//         Function::new("parse_attribute")
//           .arg("mut element", "XMLElementWrapper")
//           .ret("Result<Self, XsdError>")
//           .push_block(output)
//           .line("element.finalize(false, false)?;")
//           .line("Ok(output)"),
//       );

//     parent_name.ty = XsdType::Union;

//     Ok(XsdImpl {
//       fieldname_hint: Some(parent_name.to_field_name()),
//       name: parent_name,
//       element: XsdElement::Enum(generated_enum),
//       implementation: vec![r#impl],
//       inner: inner_impl,
//     })
//   }
// }
