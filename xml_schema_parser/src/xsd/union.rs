use crate::codegen::{Block, Enum, Impl};

use super::{
  simple_type::SimpleType,
  xsd_context::{to_struct_name, XsdContext, XsdElement, XsdImpl, XsdName},
  XMLElementWrapper, XsdError,
};

#[derive(Clone, Default, Debug, PartialEq)]
// #[yaserde(prefix = "xs", namespace = "xs: http://www.w3.org/2001/XMLSchema")]
pub struct Union {
  pub member_types: Vec<String>,
  pub simple_types: Vec<SimpleType>,
}

impl Union {
  pub fn parse(mut element: XMLElementWrapper) -> Result<Self, XsdError> {
    element.check_name("union")?;

    let member_types: Option<String> = element.try_get_attribute("memberTypes")?;
    let mut members = vec![];

    if let Some(member_types) = member_types {
      for member in member_types.split_whitespace() {
        members.push(member.to_string());
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
    let mut generated_enum = Enum::new(&parent_name.to_struct_name())
      .vis("pub")
      .to_owned();
    for derive in ["Clone", "Debug", "PartialEq"] {
      generated_enum.derive(derive);
    }

    let mut output = Block::new("let output = ").after(";").to_owned();

    let mut names = Vec::new();
    for (index, member) in self.member_types.iter().enumerate() {
      if let Some(imp) = context.structs.get(&XsdName::new(member)) {
        let st_name = to_struct_name(&imp.element.get_type().name);
        generated_enum
          .new_variant(&st_name)
          .tuple(imp.element.get_type());
        output.line(format!(
          "let gen_{}: Option<{}> = element.try_get_content();",
          index,
          imp.element.get_type().name
        ));
        names.push(st_name);
      } else {
        return Err(XsdError::XsdImplNotFound(parent_name));
      }
    }

    for index in 0..names.len() {
      output.line(&format!(
        "if gen_{}.is_some() {{ oks.push({}) }}",
        index, index
      ));
    }

    output.line("if oks.len() > 1 { return Err(XsdError::XsdGenError { name: element.name, msg: format!(\"{} were able to be parsed.\", oks.join(\", \")) }); }");

    let mut match_block = Block::new(&format!(
      "match ({})",
      (0..self.member_types.len())
        .map(|i| format!("gen_{}", i))
        .collect::<Vec<_>>()
        .join(", ")
    ));
    for index in 0..self.member_types.len() {
      match_block.line(&format!(
        "({}) => Self::{}(value),",
        (0..self.member_types.len())
          .map(|i| if i == index { "Some(value)" } else { "None" })
          .collect::<Vec<_>>()
          .join(", "),
        names[index]
      ));
    }
    match_block.push_block(Block::new(&format!(
      "({}) => ",
      (0..self.member_types.len())
        .map(|_| "None")
        .collect::<Vec<_>>()
        .join(", "))).line("return Err(XsdError::XsdGenError { name: element.name, msg: format!(\"No valid values could be parsed.\") });").to_owned()
    );
    match_block.line("_ => unreachable!()");

    output.push_block(match_block);

    let mut r#impl = Impl::new(generated_enum.ty())
      .impl_trait("ParseXsd")
      .to_owned();

    let parse = r#impl.new_fn("parse");
    parse.arg("mut element", "XMLElementWrapper");
    parse.ret("Result<Self, XsdError>");

    parse.push_block(
      Block::new("")
        .push_block(output)
        .to_owned()
        .line("element.finalize(false, false)?;")
        .line("Ok(output)")
        .to_owned(),
    );

    Ok(XsdImpl {
      fieldname_hint: Some(parent_name.to_field_name()),
      name: parent_name,
      element: XsdElement::Enum(generated_enum),
      implementation: vec![r#impl],
      inner: vec![],
    })
  }
}
