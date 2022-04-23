use xsd_codegen::{
  Block, Enum, Field, Fields, Formatter, Impl, Item, Module, Struct, Type, TypeDef, Variant,
};
use xsd_types::{to_field_name, to_struct_name, XsdIoError, XsdName, XsdParseError, XsdType};

use std::collections::BTreeMap;
use std::fmt::{Debug, Write};
use std::io::Cursor;
use std::iter::FromIterator;
use xml::namespace::Namespace;
use xml::reader::{EventReader, XmlEvent};

use super::XsdError;

#[derive(Clone, Debug)]
pub enum XsdElement {
  Struct(Struct),
  Enum(Enum),
  Field(Field),
  Type(Type),
  TypeAlias(Type, Type),
}

impl XsdElement {
  pub fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    match &self {
      XsdElement::Struct(r#struct) => r#struct.fmt(f),
      XsdElement::Enum(r#enum) => r#enum.fmt(f),
      XsdElement::TypeAlias(alias, r#type) => {
        write!(f, "pub type ")?;
        alias.fmt(f)?;
        write!(f, " = ")?;
        r#type.fmt(f)?;
        writeln!(f, ";")?;
        Ok(())
      }
      XsdElement::Field(_) => unreachable!("Should have packed field into an enum or struct."),
      _ => Ok(()),
    }
  }

  pub fn get_last_added_field(&self) -> Option<(String, String)> {
    match self {
      XsdElement::Struct(a) => match &a.fields {
        Fields::Tuple(a) => a.last().map(|v| (v.1.name.clone(), v.1.name.clone())),
        Fields::Named(a) => a.last().map(|v| (v.name.clone(), v.ty.to_string())),
        _ => None,
      },
      XsdElement::Enum(a) => a.variants.last().map(|v| (v.name.clone(), v.name.clone())),
      XsdElement::Field(_) => None,
      XsdElement::Type(_) => None,
      XsdElement::TypeAlias(_, _) => None,
    }
  }

  pub fn get_type(&self) -> Type {
    self.try_get_type().unwrap()
  }

  pub fn try_get_type(&self) -> Option<Type> {
    match &self {
      XsdElement::Struct(r#struct) => Some(r#struct.ty().to_owned()),
      XsdElement::Enum(r#enum) => Some(r#enum.ty().to_owned()),
      XsdElement::Type(r#type) | XsdElement::TypeAlias(r#type, _) => Some(r#type.clone()),
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
  Field,
  Attribute,
}

pub struct MergeSettings<'a> {
  pub conflict_prefix: Option<&'a str>,
  pub merge_type: MergeType,
}

impl<'a> MergeSettings<'a> {
  pub const ATTRIBUTE: MergeSettings<'a> = MergeSettings {
    conflict_prefix: Some("attr_"),
    merge_type: MergeType::Attribute,
  };
}

impl<'a> Default for MergeSettings<'a> {
  fn default() -> Self {
    Self {
      conflict_prefix: None,
      merge_type: MergeType::Field,
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
        XsdElement::TypeAlias(..) => {}
      }

      for i in &inner.implementation {
        module.push_impl(i.clone());
      }

      inner.wrap_inner_mod(module, level + 1);
    }
  }

  pub fn wrap_inner(&self) -> Option<Module> {
    if self.inner.is_empty() {
      return None;
    }

    let mut top_level = Module::new("-temp");
    self.wrap_inner_mod(&mut top_level, 1);

    for i in top_level.scope.items {
      if let Item::Module(m) = i {
        return Some(m);
      };
    }

    unreachable!();
  }

  pub fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
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
        Fields::Empty => unimplemented!(),
        Fields::Tuple(tup) => tup.iter().map(|v| v.1.name.as_str()).collect::<String>(),
        Fields::Named(names) => names
          .iter()
          .map(|f| to_struct_name(&f.name))
          .collect::<String>(),
      },
      XsdElement::Enum(a) => a.variants.iter().map(|v| v.name.as_str()).collect(),
      XsdElement::Type(ty) | XsdElement::TypeAlias(ty, _) => ty.name.clone(),
      XsdElement::Field(_) => unreachable!(),
    }
  }

  pub fn set_name(&mut self, name: &str) {
    self.name = XsdName {
      namespace: None,
      local_name: name.to_string(),
      ty: self.name.ty.clone(),
    };

    let ty = to_struct_name(name);
    self.element.set_type(ty.clone());

    for i in &mut self.implementation {
      i.target = ty.clone().into();
    }

    self.fieldname_hint = None;
  }

  pub fn to_string(&self) -> Result<String, core::fmt::Error> {
    let mut dst = String::new();
    let mut formatter = Formatter::new(&mut dst);

    self.fmt(&mut formatter)?;

    Ok(dst)
  }

  pub fn to_field(&self) -> XsdImpl {
    if let XsdElement::Field(_) = self.element {
      self.clone()
    } else {
      XsdImpl {
        name: self.name.clone(),
        fieldname_hint: self.fieldname_hint.clone(),
        element: XsdElement::Field(Field::new(
          self.element.get_type().xml_name.clone(),
          &self
            .fieldname_hint
            .clone()
            .unwrap_or_else(|| self.name.to_field_name()),
          self.element.get_type(),
        )),
        inner: vec![],
        implementation: vec![],
      }
    }
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
                let mut variant = Variant::new(t.1.xml_name.clone(), &t.1.name);
                variant.fields.tuple(t.1);
                gen_enum = gen_enum.push_variant(variant);
              }
            } else {
              let mut variant = Variant::new(str.type_def.ty.xml_name, &str.type_def.ty.name);
              for t in tup {
                variant.fields.tuple(t.1);
              }
              gen_enum = gen_enum.push_variant(variant);
            }
          }
          Fields::Named(names) => {
            if merge_as_fields {
              for t in names {
                let mut variant = Variant::new(t.xml_name, &t.name);
                variant.fields.tuple(t.ty);
                gen_enum = gen_enum.push_variant(variant);
              }
            } else {
              let mut variant = Variant::new(str.type_def.ty.xml_name, &str.type_def.ty.name);
              for t in names {
                variant.fields.push_named(t);
              }
              gen_enum = gen_enum.push_variant(variant);
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

  pub fn merge(&mut self, mut other: XsdImpl, settings: MergeSettings) {
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

          let field = Field::new(ty.xml_name.clone(), &field_name, ty).vis("pub");
          // let field = match settings.merge_type {
          //   MergeType::Field => field,
          //   MergeType::Attribute => field.annotation(vec!["#[yaserde(attribute)]"]),
          // };
          a.push_field(field);
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

          let field = Field::new(ty.xml_name.clone(), &field_name, ty).vis("pub");
          // let field = match settings.merge_type {
          //   MergeType::Field => field,
          //   MergeType::Attribute => field.annotation(vec!["#[yaserde(attribute)]"]),
          // };
          a.push_field(field);
        }
        XsdElement::Type(b) | XsdElement::TypeAlias(b, _) => {
          let field_name = to_field_name(other.fieldname_hint.as_ref().unwrap_or_else(|| &b.name));
          self.inner.extend(other.inner);
          let field = Field::new(b.xml_name.clone(), &field_name, b).vis("pub");
          // let field = match settings.merge_type {
          //   MergeType::Field => field,
          //   MergeType::Attribute => field.annotation(vec!["#[yaserde(attribute)]"]),
          // };
          a.push_field(field);
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

          let variant = Variant::new(b.ty().xml_name.clone(), &field_name).tuple(ty);
          // let variant = match settings.merge_type {
          //   MergeType::Field => variant,
          //   MergeType::Attribute => variant.attribute("#[yaserde(attribute)]"),
          // };
          a.variants.push(variant);

          self.inner.push(other);
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

          let variant = Variant::new(None, &to_struct_name(&field_name)).tuple(ty);
          // let variant = match settings.merge_type {
          //   MergeType::Field => variant,
          //   MergeType::Attribute => variant.attribute("#[yaserde(attribute)]"),
          // };

          a.variants.push(variant);
        }
        XsdElement::Type(b) | XsdElement::TypeAlias(b, _) => {
          let field_name = to_struct_name(other.fieldname_hint.as_ref().unwrap_or_else(|| &b.name));
          self.inner.extend(other.inner);
          let variant = Variant::new(None, &field_name).tuple(b.clone());
          // let variant = match settings.merge_type {
          //   MergeType::Field => variant,
          //   MergeType::Attribute => variant.attribute("#[yaserde(attribute)]"),
          // };
          a.variants.push(variant);
        }
        XsdElement::Field(b) => {
          let variant = Variant::new(None, &to_struct_name(&b.name)).tuple(b.ty.clone());
          // let variant = match settings.merge_type {
          //   MergeType::Field => variant,
          //   MergeType::Attribute => variant.attribute("#[yaserde(attribute)]"),
          // };
          a.variants.push(variant);
        }
      },
      XsdElement::Type(_) => unimplemented!("Cannot merge into type."),
      XsdElement::TypeAlias(..) => unimplemented!("Cannot merge into type alias."),
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
          a.variants.push(variant);
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

pub enum SearchResult<'a> {
  MultipleMatches,
  NoMatches,
  SingleMatch(&'a XsdImpl),
}

#[derive(Clone, Debug)]
pub struct XsdContext {
  pub namespace: Namespace,
  pub xml_schema_prefix: Option<String>,
  pub structs: BTreeMap<XsdName, XsdImpl>,
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
            let namespace_uri = &name.namespace.unwrap();
            let impl_basic_type = |name: &str, ty: &str| -> (XsdName, XsdImpl) {
              let xsd_name = XsdName {
                namespace: Some(namespace_uri.clone()),
                local_name: name.to_string(),
                ty: XsdType::SimpleType,
              };

              // let mut r#impl = Impl::new(ty).impl_trait("ParseXsd").to_owned();
              // let func = r#impl.new_fn("parse");
              // func.line("element.get_content()?");
              // let func = r#impl.new_fn("parse_attribute");
              // func.line("element.get_attribute()?");

              let imp = XsdImpl {
                name: xsd_name.clone(),
                fieldname_hint: None,
                element: XsdElement::Type(Type::new(None, ty)),
                inner: vec![],
                implementation: vec![],
              };

              (xsd_name, imp)
            };

            return Ok(XsdContext {
              namespace,
              xml_schema_prefix: None,
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
                  ("NCName", "String"),
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

    Err(XsdIoError::XsdParseError(XsdParseError {
      node_name: "schema".to_string(),
      msg: "Bad XML Schema, unable to found schema element.".to_string(),
    }))?
  }

  fn resolve_namespace(&self, namespace: Option<&str>) -> Option<String> {
    if let Some(ns) = namespace {
      if let Some(ns) = self.namespace.get(ns).map(|v| v.to_string()) {
        Some(ns)
      } else {
        namespace.map(|v| v.to_string())
      }
    } else {
      namespace.map(|v| v.to_string())
    }
  }

  pub fn remove_impl(&mut self, name: &XsdName) -> Option<XsdImpl> {
    let namespace = self.resolve_namespace(name.namespace.as_ref().map(|v| v.as_str()));

    self.structs.remove(&XsdName {
      namespace: namespace.clone(),
      local_name: name.local_name.clone(),
      ty: name.ty.clone(),
    })
  }

  pub fn insert_impl(&mut self, name: XsdName, value: XsdImpl) {
    let namespace = self.resolve_namespace(name.namespace.as_ref().map(|v| v.as_str()));

    self.structs.insert(
      XsdName {
        namespace: namespace.clone(),
        local_name: name.local_name.clone(),
        ty: name.ty.clone(),
      },
      value,
    );
  }

  pub fn search(&self, name: &XsdName) -> Option<&XsdImpl> {
    let namespace = self.resolve_namespace(name.namespace.as_ref().map(|v| v.as_str()));

    self.structs.get(&XsdName {
      namespace: namespace.clone(),
      local_name: name.local_name.clone(),
      ty: name.ty.clone(),
    })
  }

  pub fn multi_search(
    &self,
    namespace: Option<String>,
    name: String,
    types: &[XsdType],
  ) -> SearchResult {
    let mut output = SearchResult::NoMatches;
    for ty in types {
      if let Some(result) = self.search(&XsdName {
        namespace: namespace.clone(),
        local_name: name.clone(),
        ty: *ty,
      }) {
        if let SearchResult::SingleMatch(_) = output {
          return SearchResult::MultipleMatches;
        }
        output = SearchResult::SingleMatch(result);
      }
    }

    output
  }

  pub fn has_xml_schema_prefix(&self) -> bool {
    self.xml_schema_prefix.is_some()
  }

  pub fn match_xml_schema_prefix(&self, value: &str) -> bool {
    self.xml_schema_prefix == Some(value.to_string())
  }
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
