use crate::codegen::{self, Block, Enum, Fields, Module, Struct, Type, TypeDef, Variant};
use heck::{CamelCase, SnakeCase};

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::io::Cursor;
use std::iter::FromIterator;
use xml::namespace::Namespace;
use xml::reader::{EventReader, XmlEvent};

use crate::codegen::{Field, Impl};

use super::XsdError;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct XsdName {
  pub namespace: Option<String>,
  pub local_name: String,
}

impl std::fmt::Display for XsdName {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if let Some(namespace) = &self.namespace {
      write!(f, "{}:{}", namespace, self.local_name)
    } else {
      write!(f, "{}", self.local_name)
    }
  }
}

impl XsdName {
  pub fn new(name: &str) -> Self {
    Self {
      namespace: None,
      local_name: name.to_string(),
    }
  }

  pub fn to_struct_name(&self) -> String {
    to_struct_name(&self.local_name)
  }

  pub fn to_field_name(&self) -> String {
    to_field_name(&self.local_name)
  }
}

pub fn to_struct_name(name: &str) -> String {
  let output = name.replace(".", "_").to_camel_case();
  if let Some(char) = output.chars().next() {
    if char.is_numeric() {
      return format!("_{output}");
    }
  }

  output
}

pub fn to_field_name(name: &str) -> String {
  let name = name.to_snake_case();

  if name == "type" {
    "r#type".to_string()
  } else {
    name
  }
}

#[derive(Clone, Debug)]
pub enum XsdElement {
  Struct(Struct),
  Enum(Enum),
  Field(Field),
  Type(Type),
}

impl XsdElement {
  pub fn fmt(&self, f: &mut crate::codegen::Formatter) -> core::fmt::Result {
    match &self {
      XsdElement::Struct(r#struct) => r#struct.fmt(f),
      XsdElement::Enum(r#enum) => r#enum.fmt(f),
      XsdElement::Type(r#type) => r#type.fmt(f),
      XsdElement::Field(_) => unreachable!("Should have packed field into an enum or struct."),
    }
  }

  pub fn get_last_added_field(&self) -> Option<(String, String)> {
    match self {
      XsdElement::Struct(a) => match &a.fields {
        crate::codegen::Fields::Tuple(a) => a.last().map(|v| (v.1.name.clone(), v.1.name.clone())),
        crate::codegen::Fields::Named(a) => a.last().map(|v| (v.name.clone(), v.ty.to_string())),
        _ => None,
      },
      XsdElement::Enum(a) => a.variants.last().map(|v| (v.name.clone(), v.name.clone())),
      XsdElement::Field(_) => None,
      XsdElement::Type(_) => None,
    }
  }

  pub fn get_type(&self) -> Type {
    self.try_get_type().unwrap()
  }

  pub fn try_get_type(&self) -> Option<Type> {
    match &self {
      XsdElement::Struct(r#struct) => Some(r#struct.ty().to_owned()),
      XsdElement::Enum(r#enum) => Some(r#enum.ty().to_owned()),
      XsdElement::Type(r#type) => Some(r#type.clone()),
      XsdElement::Field(field) => Some(field.ty.clone()),
    }
  }

  pub fn set_type(&mut self, name: impl Into<Type>) {
    match self {
      XsdElement::Struct(r#struct) => {
        r#struct.type_def.ty = name.into();
      }
      XsdElement::Enum(r#enum) => {
        r#enum.type_def.ty = name.into();
      }
      _ => {}
    }
  }

  pub fn add_doc(&mut self, doc: &str) {
    match self {
      XsdElement::Struct(r#struct) => {
        r#struct.doc(doc);
      }
      XsdElement::Enum(r#enum) => {
        r#enum.doc(doc);
      }
      XsdElement::Field(field) => {
        field.doc(vec![doc]);
      }
      _ => {}
    }
  }

  pub fn is_builtin_type(&self) -> bool {
    match &self {
      XsdElement::Type(ty) => [
        "String", "u8", "i8", "u16", "i16", "u32", "i32", "u64", "i64", "u128", "i128", "bool",
        "f32", "f64", "char",
      ]
      .contains(&ty.name.as_str()),
      _ => false,
    }
  }
}

#[derive(Default, Debug, Clone)]
pub struct XsdDeserialize {
  map: BTreeMap<String, (Block, bool)>,
}

#[derive(Debug, Clone)]
pub struct XsdImpl {
  pub name: XsdName,
  pub fieldname_hint: Option<String>,
  pub element: XsdElement,
  pub inner: Vec<XsdImpl>,
  pub implementation: Vec<Impl>,
}

pub enum MergeType {
  Fields,
  Structs,
}

pub struct MergeSettings<'a> {
  pub conflict_prefix: Option<&'a str>,
  pub merge_type: MergeType,
}

impl<'a> MergeSettings<'a> {
  pub const ATTRIBUTE: MergeSettings<'a> = MergeSettings {
    conflict_prefix: Some("attr_"),
    merge_type: MergeType::Structs,
  };
}

impl<'a> Default for MergeSettings<'a> {
  fn default() -> Self {
    Self {
      conflict_prefix: None,
      merge_type: MergeType::Structs,
    }
  }
}

pub fn infer_type_name(this: &[XsdImpl]) -> String {
  let mut output = String::new();

  for i in this {
    if let Some(hint) = &i.fieldname_hint {
      output.push_str(hint);
    } else {
      output.push_str(&i.element.get_type().name);
    }
  }

  output
}

impl XsdImpl {
  fn wrap_inner_mod(&self, existing_module: &mut Module, level: usize) {
    if self.inner.is_empty() {
      return;
    }

    let mod_name = to_field_name(&self.element.get_type().name);
    let module = existing_module.get_or_new_module(&mod_name);

    module.import(
      &(0..level).map(|_| "super").collect::<Vec<_>>().join("::"),
      "*",
    );

    for inner in &self.inner {
      match &inner.element {
        XsdElement::Struct(a) => {
          module.push_struct(a.clone());
        }
        XsdElement::Enum(a) => {
          module.push_enum(a.clone());
        }
        XsdElement::Field(_) => unimplemented!(),
        XsdElement::Type(_) => {}
      }

      for i in &inner.implementation {
        module.push_impl(i.clone());
      }

      inner.wrap_inner_mod(module, level + 1);
    }
  }

  pub fn wrap_inner(&self) -> Option<codegen::Module> {
    if self.inner.is_empty() {
      return None;
    }

    let mut top_level = Module::new("-temp");
    self.wrap_inner_mod(&mut top_level, 1);

    for i in top_level.scope.items {
      if let codegen::Item::Module(m) = i {
        return Some(m);
      };
    }

    unreachable!();
  }

  pub fn fmt(&self, f: &mut crate::codegen::Formatter<'_>) -> core::fmt::Result {
    self.element.fmt(f)?;
    for r#impl in &self.implementation {
      r#impl.fmt(f)?;
    }

    if let Some(module) = self.wrap_inner() {
      module.fmt(f)?;
    }

    Ok(())
  }

  pub fn infer_type_name(&self) -> String {
    match &self.element {
      XsdElement::Struct(a) => match &a.fields {
        crate::codegen::Fields::Empty => unimplemented!(),
        crate::codegen::Fields::Tuple(tup) => {
          tup.iter().map(|v| v.1.name.as_str()).collect::<String>()
        }
        crate::codegen::Fields::Named(names) => names
          .iter()
          .map(|f| to_struct_name(&f.name))
          .collect::<String>(),
      },
      XsdElement::Enum(a) => a.variants.iter().map(|v| v.name.as_str()).collect(),
      XsdElement::Type(ty) => ty.name.clone(),
      XsdElement::Field(_) => unreachable!(),
    }
  }

  pub fn set_name(&mut self, name: &str) {
    self.name = XsdName::new(name);

    let ty = to_struct_name(name);
    self.element.set_type(ty.clone());

    for i in &mut self.implementation {
      i.target = ty.clone().into();
    }

    self.fieldname_hint = None;
  }

  pub fn to_string(&self) -> Result<String, core::fmt::Error> {
    let mut dst = String::new();
    let mut formatter = crate::codegen::Formatter::new(&mut dst);

    self.fmt(&mut formatter)?;

    Ok(dst)
  }

  pub fn merge_into_enum(&mut self, mut other: XsdImpl, merge_as_fields: bool) {
    other.element = match other.element {
      XsdElement::Struct(str) => {
        let mut gen_enum = Enum {
          type_def: str.type_def.clone(),
          variants: vec![],
        };

        match str.fields {
          Fields::Empty => {}
          Fields::Tuple(tup) => {
            if merge_as_fields {
              for t in tup {
                let mut variant = Variant::new(&t.1.name);
                variant.fields.tuple(t.1);
                gen_enum.push_variant(variant);
              }
            } else {
              let mut variant = Variant::new(&str.type_def.ty.name);
              for t in tup {
                variant.fields.tuple(t.1);
              }
              gen_enum.push_variant(variant);
            }
          }
          Fields::Named(names) => {
            if merge_as_fields {
              for t in names {
                let mut variant = Variant::new(&t.name);
                variant.fields.tuple(t.ty);
                gen_enum.push_variant(variant);
              }
            } else {
              let mut variant = Variant::new(&str.type_def.ty.name);
              for t in names {
                variant.fields.push_named(t);
              }
              gen_enum.push_variant(variant);
            }
          }
        }

        XsdElement::Enum(gen_enum)
      }
      _ => other.element,
    };

    self.merge(other, MergeSettings::default())
  }

  fn merge_typedef(this: &mut TypeDef, other: TypeDef) {
    match (&mut this.docs, other.docs) {
      (None, None) => {}
      (None, Some(b)) => {
        this.docs = Some(b);
      }
      (Some(_), None) => {}
      (Some(a), Some(b)) => {
        if !a.docs.ends_with('\n') {
          a.docs.push('\n');
        }
        a.docs.push_str(&b.docs);
      }
    }

    for derive in other.derive {
      if !this.derive.contains(&derive) {
        this.derive.push(derive);
      }
    }

    for allow in other.allow {
      if !this.allow.contains(&allow) {
        this.allow.push(allow);
      }
    }

    for bound in other.bounds {
      let bounded_names = bound
        .bound
        .iter()
        .map(|t| t.name.clone())
        .collect::<Vec<_>>();

      let mut add = true;
      for b in &this.bounds {
        assert!(
          b.name != bound.name
            || (b.bound.len() == bound.bound.len()
              && b.bound.iter().all(|t| bounded_names.contains(&t.name)))
        );

        if b.name == bound.name {
          add = false;
          break;
        }
      }

      if add {
        this.bounds.push(bound);
      }
    }

    for macros in other.macros {
      if !this.macros.contains(&macros) {
        this.macros.push(macros);
      }
    }
  }

  pub fn merge(&mut self, other: XsdImpl, settings: MergeSettings) {
    match &settings.merge_type {
      MergeType::Fields => self.merge_fields(other, settings),
      MergeType::Structs => {
        self.merge_structs(other, settings);
      }
    }
  }

  pub fn merge_structs(&mut self, mut other: XsdImpl, _settings: MergeSettings) {
    match &mut self.element {
      XsdElement::Struct(a) => match &other.element {
        XsdElement::Struct(b) => {
          let field_name = to_field_name(
            other
              .fieldname_hint
              .as_ref()
              .unwrap_or_else(|| &b.ty().name),
          );
          let mut ty = b.ty().clone();

          other.fieldname_hint = Some(field_name.clone());

          ty.name = format!("{}::{}", to_field_name(&a.ty().name), ty.name);

          self.inner.push(other);

          a.push_field(Field::new(&field_name, ty));
        }
        XsdElement::Enum(b) => {
          let field_name = to_field_name(
            other
              .fieldname_hint
              .as_ref()
              .unwrap_or_else(|| &b.ty().name),
          );
          let mut ty = b.ty().clone();

          other.fieldname_hint = Some(field_name.clone());

          ty.name = format!("{}::{}", to_field_name(&a.ty().name), ty.name);

          self.inner.push(other);

          a.push_field(Field::new(&field_name, ty).vis("pub").to_owned());
        }
        XsdElement::Type(b) => {
          let field_name = to_field_name(other.fieldname_hint.as_ref().unwrap_or_else(|| &b.name));
          self.inner.extend(other.inner);
          a.push_field(Field::new(&field_name, b).vis("pub").to_owned());
        }
        XsdElement::Field(b) => {
          a.push_field(b.clone());
        }
      },
      XsdElement::Enum(a) => match &other.element {
        XsdElement::Struct(b) => {
          let field_name = to_field_name(
            other
              .fieldname_hint
              .as_ref()
              .unwrap_or_else(|| &b.ty().name),
          );
          let mut ty = b.ty().clone();

          other.fieldname_hint = Some(field_name.clone());

          ty.name = format!("{}::{}", to_field_name(&a.ty().name), ty.name);

          self.inner.push(other);

          a.new_variant(&field_name).tuple(ty);
        }
        XsdElement::Enum(b) => {
          let field_name = to_field_name(
            other
              .fieldname_hint
              .as_ref()
              .unwrap_or_else(|| &b.ty().name),
          );
          let mut ty = b.ty().clone();

          other.fieldname_hint = Some(field_name.clone());

          ty.name = format!("<{}>::{}", to_field_name(&a.ty().name), ty.name);

          self.inner.push(other);

          a.new_variant(&field_name).tuple(ty);
        }
        XsdElement::Type(b) => {
          let field_name = to_field_name(other.fieldname_hint.as_ref().unwrap_or_else(|| &b.name));
          self.inner.extend(other.inner);
          a.new_variant(&field_name).tuple(b.clone());
        }
        XsdElement::Field(b) => {
          a.new_variant(&b.name).tuple(b.ty.clone());
        }
      },
      XsdElement::Type(_) => unimplemented!("Cannot merge into type."),
      XsdElement::Field(_) => unimplemented!("Cannot merge into field."),
    }
  }

  pub fn merge_fields(&mut self, other: XsdImpl, settings: MergeSettings) {
    match (&mut self.element, other.element) {
      (XsdElement::Struct(a), XsdElement::Struct(b)) => {
        match (&mut a.fields, b.fields) {
          (_, Fields::Empty) => {}
          (Fields::Empty, Fields::Tuple(b)) => {
            a.fields = Fields::Tuple(b);
          }
          (Fields::Empty, Fields::Named(b)) => {
            a.fields = Fields::Named(b);
          }
          (Fields::Tuple(a), Fields::Tuple(b)) => {
            for field in b {
              for f in a.iter() {
                if field.1.name == f.1.name {
                  panic!("Merge conflict in field!");
                }
              }
              a.push(field);
            }
          }
          (Fields::Named(a), Fields::Named(b)) => {
            for mut field in b {
              let mut conflict = false;
              for f in a.iter() {
                if field.name == f.name {
                  conflict = true;
                  break;
                }
              }

              if conflict {
                if let Some(prefix) = settings.conflict_prefix {
                  field.name = format!("{}{}", prefix, field.name);
                  a.push(field)
                } else {
                  panic!("Merge conflict in field!");
                }
              } else {
                a.push(field);
              }
            }
          }
          (Fields::Tuple(_), Fields::Named(_)) => {
            panic!("[ERROR] Tried to merge named field into tuple!")
          }
          (Fields::Named(_), Fields::Tuple(_)) => {
            panic!("[ERROR] Tried to merge tuple field into named!")
          }
        }
        Self::merge_typedef(&mut a.type_def, b.type_def);
      }
      (XsdElement::Enum(a), XsdElement::Enum(b)) => {
        for variant in b.variants {
          for v in &a.variants {
            if variant.name == v.name {
              panic!("Merge conflict in variant!");
            }
          }
          a.push_variant(variant);
        }
        Self::merge_typedef(&mut a.type_def, b.type_def);
      }
      (XsdElement::Type(_a), XsdElement::Type(_b)) => {
        panic!("Tried to merge type with type");
      }
      _ => panic!("Invalid merge"),
    }

    // Check trait impls, use current on conflict (will need to change)
    //  Have special handling for known traits
    // Check normal impls, raise error on conflicting names
    // for j in other.implementation {
    //   for i in &self.implementation {
    //     i.fns.extend(j.fns);
    //     i.macros.extend(j.macros);
    //   }

    //   i.fns.extend(j.fns);
    //   i.macros.extend(j.macros);
    // }

    // Check for two things that are named the same
    for j in other.inner {
      for i in &self.inner {
        assert!(match (i.element.try_get_type(), j.element.try_get_type()) {
          (None, None) => false,
          (None, Some(_)) => true,
          (Some(_), None) => true,
          (Some(a), Some(b)) => a.name != b.name,
        });
      }
      self.inner.push(j);
    }
  }
}

#[derive(Clone, Debug)]
pub struct XsdContext {
  module_namespace_mappings: BTreeMap<String, String>,
  pub namespace: Namespace,
  xml_schema_prefix: Option<String>,
  pub groups: BTreeMap<XsdName, Vec<Field>>,
  pub structs: BTreeMap<XsdName, XsdImpl>,
  pub allow_unknown_type: bool,
}

impl XsdContext {
  pub fn new(content: &str) -> Result<Self, XsdError> {
    let cursor = Cursor::new(content);
    let parser = EventReader::new(cursor);

    for xml_element in parser {
      match xml_element {
        Ok(XmlEvent::StartElement {
          name, namespace, ..
        }) => {
          if name.namespace == Some("http://www.w3.org/2001/XMLSchema".to_string())
            && name.local_name == "schema"
          {
            let module_namespace_mappings = BTreeMap::new();
            let xml_schema_prefix = name.prefix;

            let impl_basic_type = |name: &str, ty: &str| -> (XsdName, XsdImpl) {
              let xsd_name = XsdName {
                namespace: None,
                local_name: format!(
                  "{}{}{}",
                  xml_schema_prefix.as_deref().unwrap_or(""),
                  if xml_schema_prefix.is_some() { ":" } else { "" },
                  name
                ),
              };

              // let mut r#impl = Impl::new(ty).impl_trait("ParseXsd").to_owned();
              // let func = r#impl.new_fn("parse");
              // func.line("element.get_content()?");
              // let func = r#impl.new_fn("parse_attribute");
              // func.line("element.get_attribute()?");

              let imp = XsdImpl {
                name: xsd_name.clone(),
                fieldname_hint: None,
                element: XsdElement::Type(Type::new(ty)),
                inner: vec![],
                implementation: vec![],
              };

              (xsd_name, imp)
            };

            return Ok(XsdContext {
              module_namespace_mappings,
              namespace,
              xml_schema_prefix: xml_schema_prefix.clone(),
              allow_unknown_type: false,
              groups: BTreeMap::new(),
              structs: BTreeMap::from_iter(
                [
                  ("bool", "bool"),
                  ("boolean", "bool"),
                  ("positiveInteger", "u64"),
                  ("byte", "u8"),
                  ("unsignedByte", "u8"),
                  ("short", "i16"),
                  ("unsignedShort", "u16"),
                  ("int", "i32"),
                  ("integer", "i32"),
                  ("unsignedInt", "u32"),
                  ("long", "i64"),
                  ("unsignedLong", "u64"),
                  ("nonNegativeInteger", "u64"),
                  ("double", "f64"),
                  ("decimal", "f64"),
                  ("string", "String"),
                  ("normalizedString", "String"),
                  ("anyURI", "String"),
                  ("NMTOKEN", "String"),
                  ("token", "String"),
                  ("language", "String"),
                  ("hexBinary", "String"),
                  ("dateTime", "String"),
                  ("base64Binary", "String"),
                  ("duration", "String"),
                  ("gYear", "u16"),
                  ("ID", "String"),
                  ("IDREF", "String"),
                  ("IDREFS", "String"),
                  ("anyType", "String"),
                  ("date", "chrono::Date<chrono::Utc>"),
                ]
                .map(|(n, t)| impl_basic_type(n, t)),
              ),
            });
          }
        }
        Err(_) => break,
        _ => {}
      }
    }

    Err(XsdError::XsdParseError(
      "Bad XML Schema, unable to found schema element.".to_string(),
    ))
  }

  pub fn with_module_namespace_mappings(
    mut self,
    module_namespace_mappings: &BTreeMap<String, String>,
  ) -> Self {
    self.module_namespace_mappings = module_namespace_mappings.clone();
    self
  }

  pub fn has_xml_schema_prefix(&self) -> bool {
    self.xml_schema_prefix.is_some()
  }

  pub fn match_xml_schema_prefix(&self, value: &str) -> bool {
    self.xml_schema_prefix == Some(value.to_string())
  }

  pub fn get_module(&self, prefix: &str) -> Option<String> {
    self
      .namespace
      .get(prefix)
      .map(|namespace| {
        self
          .module_namespace_mappings
          .get(namespace)
          .map(|module| module.to_owned())
      })
      .unwrap_or_else(|| None)
  }
}

#[test]
fn get_module() {
  let context = XsdContext::new(
    r#"
    <xs:schema
      xmlns:xs="http://www.w3.org/2001/XMLSchema"
      xmlns:example="http://example.com"
      >
    </xs:schema>
  "#,
  )
  .unwrap();

  let mut mapping = BTreeMap::new();
  mapping.insert(
    "http://example.com".to_string(),
    "crate::example".to_string(),
  );
  let context = context.with_module_namespace_mappings(&mapping);

  assert_eq!(
    context.get_module("example"),
    Some("crate::example".to_string())
  );
  assert_eq!(context.get_module("other"), None);
}

#[test]
fn bad_schema_definition() {
  let context = XsdContext::new(
    r#"
    <xs:schema
      xmlns="http://www.w3.org/2001/XMLSchema"
      >
    </xs:schema>
  "#,
  );

  assert!(context.is_err());
}
