use crate::{rust_codegen::Body, Block, Function, Impl, Type};

pub fn xsdgen_impl(r#type: Type, block: Block) -> Impl {
  let mut function = Function::new("gen")
    .arg("element", Type::new(None, "&mut XMLElement"))
    .arg("mut gen_state", Type::new(None, "GenState"))
    .arg("name", Type::new(None, "Option<&str>"))
    .ret(Type::new(None, "Result<Self, XsdIoError>"));
  let mut skip_b = false;
  if let Some(b) = &block.before {
    skip_b = b.is_empty();
  } else if block.before.is_none() {
    skip_b = true;
  }

  let mut skip_a = false;
  if let Some(a) = &block.after {
    skip_a = a.is_empty();
  } else if block.after.is_none() {
    skip_a = true;
  }

  let body = if skip_a && skip_b {
    block.body
  } else {
    vec![Body::Block(block)]
  };

  function.body = Some(body);
  Impl::new(r#type)
    .impl_trait(Type::new(None, "XsdGen"))
    .push_fn(function)
}

pub fn fromxml_impl(r#type: Type, block: Block) -> Impl {
  let mut function = Function::new("from_xml")
    .arg("string", Type::new(None, "&str"))
    .ret(Type::new(None, "Result<Self, String>"));

  let mut skip_b = false;
  if let Some(b) = &block.before {
    skip_b = b.is_empty();
  } else if block.before.is_none() {
    skip_b = true;
  }

  let mut skip_a = false;
  if let Some(a) = &block.after {
    skip_a = a.is_empty();
  } else if block.after.is_none() {
    skip_a = true;
  }

  let body = if skip_a && skip_b {
    block.body
  } else {
    vec![Body::Block(block)]
  };

  function.body = Some(body);
  Impl::new(r#type)
    .impl_trait(Type::new(None, "FromXmlString"))
    .push_fn(function)
}
