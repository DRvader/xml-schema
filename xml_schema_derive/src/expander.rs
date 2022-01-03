use crate::attribute::XmlSchemaAttribute;
use log::info;
use proc_macro2::TokenStream;
use xml_schema_parser::Xsd;

pub fn expand_derive(ast: &syn::DeriveInput) -> Result<TokenStream, String> {
  let attributes = XmlSchemaAttribute::parse(&ast.attrs);
  let _ = simple_logger::init_with_level(attributes.log_level);

  info!("{:?}", attributes);

  let mut xsd = Xsd::new_from_file(&attributes.source, &attributes.module_namespace_mappings)?;
  let generated = quote!(xsd.generate(&attributes.target_prefix));

  if let Some(store_generated_code) = &attributes.store_generated_code {
    std::fs::write(store_generated_code, generated.to_string()).map_err(|e| e.to_string())?;
  }

  Ok(generated)
}
