//! Provides a builder API for generating Rust code.
//!
//! The general strategy for using the crate is as follows:
//!
//! 1. Create a `Scope` instance.
//! 2. Use the builder API to add elements to the scope.
//! 3. Call `Scope::to_string()` to get the generated code.
//!
//! For example:
//!
//! ```rust
//! use codegen::Scope;
//!
//! let mut scope = Scope::new();
//!
//! scope.new_struct("Foo")
//!     .derive("Debug")
//!     .field("one", "usize")
//!     .field("two", "String");
//!
//! println!("{}", scope.to_string());
//! ```

use std::{
  collections::BTreeMap,
  fmt::{self, Write},
};

use xsd_types::XsdName;

/// Defines a scope.
///
/// A scope contains modules, types, etc...
#[derive(Debug, Clone)]
pub struct Scope {
  /// Scope documentation
  pub docs: Option<Docs>,

  /// Imports
  pub imports: BTreeMap<String, BTreeMap<String, Import>>,

  /// Contents of the documentation,
  pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeAlias {
  pub doc: Option<String>,
  pub alias: Type,
  pub value: Type,
}

impl TypeAlias {
  pub fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
    if let Some(doc) = &self.doc {
      for line in doc.lines() {
        writeln!(fmt, "/// {}", line)?;
      }
    }
    write!(fmt, "pub type ")?;
    self.alias.fmt(fmt)?;
    write!(fmt, " = ")?;
    self.value.fmt(fmt)?;
    writeln!(fmt, ";")?;
    Ok(())
  }
}

#[derive(Debug, Clone)]
pub enum Item {
  Module(Module),
  Struct(Struct),
  Function(Function),
  Trait(Trait),
  Enum(Enum),
  Impl(Impl),
  TypeAlias(TypeAlias),
  Raw(String),
}

/// Defines a module.
#[derive(Debug, Clone)]
pub struct Module {
  /// Module name
  pub name: String,

  /// Visibility
  pub vis: Option<String>,

  /// Module documentation
  pub docs: Option<Docs>,

  /// Contents of the module
  pub scope: Scope,
}

/// Defines an enumeration.
#[derive(Debug, Clone, PartialEq)]
pub struct Enum {
  pub type_def: TypeDef,
  pub variants: Vec<Variant>,
}

/// Defines a struct.
#[derive(Debug, Clone, PartialEq)]
pub struct Struct {
  pub type_def: TypeDef,

  /// Struct fields
  pub fields: Fields,
}

/// Define a trait.
#[derive(Debug, Clone)]
pub struct Trait {
  pub type_def: TypeDef,
  pub parents: Vec<Type>,
  pub associated_tys: Vec<AssociatedType>,
  pub fns: Vec<Function>,
  pub macros: Vec<String>,
}

/// Defines a type.
#[derive(Debug, Clone, PartialEq)]
pub struct Type {
  pub name: String,
  pub generics: Vec<Type>,
  pub xml_name: Option<XsdName>,
  pub docs: Option<Docs>,
}

/// Defines a type definition.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeDef {
  pub ty: Type,
  pub vis: Option<String>,
  pub docs: Option<Docs>,
  pub derive: Vec<String>,
  pub allow: Vec<String>,
  pub repr: Option<String>,
  pub bounds: Vec<Bound>,
  pub macros: Vec<String>,
}

/// Defines an enum variant.
#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
  pub name: String,
  pub fields: Fields,
  pub attributes: String,
  pub xml_name: Option<XsdName>,
  pub doc: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TupleField {
  pub vis: Option<String>,
  pub ty: Type,
  pub attribute: bool,
  pub flatten: bool,
}

/// Defines a set of fields.
#[derive(Debug, Clone, PartialEq)]
pub enum Fields {
  Empty,
  Tuple(Vec<TupleField>),
  Named(Vec<Field>),
}

/// Defines a struct field.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
  /// Field name
  pub name: String,

  /// Field visibility
  pub vis: Option<String>,

  /// Field type
  pub ty: Type,

  /// Field documentation
  pub documentation: Vec<String>,

  /// Field annotation
  pub annotation: Vec<String>,

  /// XML NAME
  pub xml_name: Option<XsdName>,

  /// Should the field and children be parsed as attributes
  pub attribute: bool,

  /// Should the current xml element be changed when parsing this field
  pub flatten: bool,
}

/// Defines an associated type.
#[derive(Debug, Clone)]
pub struct AssociatedType(pub Bound);

#[derive(Debug, Clone, PartialEq)]
pub struct Bound {
  pub name: String,
  pub bound: Vec<Type>,
}

/// Defines an impl block.
#[derive(Debug, Clone, PartialEq)]
pub struct Impl {
  /// The struct being implemented
  pub target: Type,

  /// Impl level generics
  pub generics: Vec<String>,

  /// If implementing a trait
  pub impl_trait: Option<Type>,

  /// Associated types
  pub assoc_tys: Vec<Field>,

  /// Bounds
  pub bounds: Vec<Bound>,

  pub fns: Vec<Function>,

  pub macros: Vec<String>,
}

/// Defines an import (`use` statement).
#[derive(Debug, Clone)]
pub struct Import {
  line: String,
  vis: Option<String>,
}

/// Defines a function.
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
  /// Name of the function
  name: String,

  /// Function documentation
  docs: Option<Docs>,

  /// A lint attribute used to suppress a warning or error
  allow: Option<String>,

  /// Function visibility
  vis: Option<String>,

  /// Function generics
  generics: Vec<String>,

  /// If the function takes `&self` or `&mut self`
  arg_self: Option<String>,

  /// Function arguments
  args: Vec<Field>,

  /// Return type
  ret: Option<Type>,

  /// Where bounds
  bounds: Vec<Bound>,

  /// Body contents
  pub body: Option<Vec<Body>>,

  /// Function attributes, e.g., `#[no_mangle]`.
  attributes: Vec<String>,

  /// Function `extern` ABI
  extern_abi: Option<String>,

  /// Whether or not this function is `async` or not
  r#async: bool,
}

/// Defines a code block. This is used to define a function body.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Block {
  pub before: Option<String>,
  pub after: Option<String>,
  pub body: Vec<Body>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Body {
  String(String),
  Block(Block),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Docs {
  pub docs: String,
}

/// Configures how a scope is formatted.
#[derive(Debug)]
pub struct Formatter<'a> {
  /// Write destination
  dst: &'a mut String,

  /// Number of spaces to start a new line with.
  spaces: usize,

  /// Number of spaces per indentiation
  indent: usize,
}

const DEFAULT_INDENT: usize = 4;

// ===== impl Scope =====

impl Scope {
  /// Returns a new scope
  pub fn new() -> Self {
    Scope {
      docs: None,
      imports: BTreeMap::new(),
      items: vec![],
    }
  }

  /// Import a type into the scope.
  ///
  /// This results in a new `use` statement being added to the beginning of
  /// the scope.
  pub fn import(&mut self, path: &str, ty: &str) -> &mut Import {
    // handle cases where the caller wants to refer to a type namespaced
    // within the containing namespace, like "a::B".
    let ty = ty.split("::").next().unwrap_or(ty);
    self
      .imports
      .entry(path.to_string())
      .or_insert_with(BTreeMap::new)
      .entry(ty.to_string())
      .or_insert_with(|| Import::new(path, ty))
  }

  /// Push a new module definition, returning a mutable reference to it.
  ///
  /// # Panics
  ///
  /// Since a module's name must uniquely identify it within the scope in
  /// which it is defined, pushing a module whose name is already defined
  /// in this scope will cause this function to panic.
  ///
  /// In many cases, the [`get_or_new_module`] function is preferrable, as it
  /// will return the existing definition instead.
  ///
  /// [`get_or_new_module`]: #method.get_or_new_module
  pub fn new_module(&mut self, name: &str) -> &mut Module {
    self.push_module(Module::new(name));

    match *self.items.last_mut().unwrap() {
      Item::Module(ref mut v) => v,
      _ => unreachable!(),
    }
  }

  /// Returns a mutable reference to a module if it is exists in this scope.
  pub fn get_module_mut<Q: ?Sized>(&mut self, name: &Q) -> Option<&mut Module>
  where
    String: PartialEq<Q>,
  {
    self.items.iter_mut().find_map(|item| match *item {
      Item::Module(ref mut module) if module.name == *name => Some(module),
      _ => None,
    })
  }

  /// Returns a mutable reference to a module if it is exists in this scope.
  pub fn get_module<Q: ?Sized>(&self, name: &Q) -> Option<&Module>
  where
    String: PartialEq<Q>,
  {
    self.items.iter().find_map(|item| match *item {
      Item::Module(ref module) if module.name == *name => Some(module),
      _ => None,
    })
  }

  /// Returns a mutable reference to a module, creating it if it does
  /// not exist.
  pub fn get_or_new_module(&mut self, name: &str) -> &mut Module {
    if self.get_module(name).is_some() {
      self.get_module_mut(name).unwrap()
    } else {
      self.new_module(name)
    }
  }

  /// Push a module definition.
  ///
  /// # Panics
  ///
  /// Since a module's name must uniquely identify it within the scope in
  /// which it is defined, pushing a module whose name is already defined
  /// in this scope will cause this function to panic.
  ///
  /// In many cases, the [`get_or_new_module`] function is preferrable, as it will
  /// return the existing definition instead.
  ///
  /// [`get_or_new_module`]: #method.get_or_new_module
  pub fn push_module(&mut self, item: Module) -> &mut Self {
    assert!(self.get_module(&item.name).is_none());
    self.items.push(Item::Module(item));
    self
  }

  pub fn push_type_alias(&mut self, item: TypeAlias) {
    self.items.push(Item::TypeAlias(item));
  }

  /// Push a new struct definition, returning a mutable reference to it.
  pub fn new_struct(&mut self, xml_name: Option<XsdName>, name: &str) -> &mut Struct {
    self.push_struct(Struct::new(xml_name, name));

    match *self.items.last_mut().unwrap() {
      Item::Struct(ref mut v) => v,
      _ => unreachable!(),
    }
  }

  /// Push a struct definition
  pub fn push_struct(&mut self, item: Struct) -> &mut Self {
    self.items.push(Item::Struct(item));
    self
  }

  /// Push a new function definition, returning a mutable reference to it.
  pub fn new_fn(&mut self, name: &str) -> &mut Function {
    self.push_fn(Function::new(name));

    match *self.items.last_mut().unwrap() {
      Item::Function(ref mut v) => v,
      _ => unreachable!(),
    }
  }

  /// Push a function definition
  pub fn push_fn(&mut self, item: Function) -> &mut Self {
    self.items.push(Item::Function(item));
    self
  }

  /// Push a new trait definition, returning a mutable reference to it.
  pub fn new_trait(&mut self, xml_name: Option<XsdName>, name: &str) -> &mut Trait {
    self.push_trait(Trait::new(name, xml_name));

    match *self.items.last_mut().unwrap() {
      Item::Trait(ref mut v) => v,
      _ => unreachable!(),
    }
  }

  /// Push a trait definition
  pub fn push_trait(&mut self, item: Trait) -> &mut Self {
    self.items.push(Item::Trait(item));
    self
  }

  /// Push a new struct definition, returning a mutable reference to it.
  pub fn new_enum(&mut self, xml_name: Option<XsdName>, name: &str) -> &mut Enum {
    self.push_enum(Enum::new(xml_name, name));

    match *self.items.last_mut().unwrap() {
      Item::Enum(ref mut v) => v,
      _ => unreachable!(),
    }
  }

  /// Push a structure definition
  pub fn push_enum(&mut self, item: Enum) -> &mut Self {
    self.items.push(Item::Enum(item));
    self
  }

  /// Push a new `impl` block, returning a mutable reference to it.
  pub fn new_impl(&mut self, target: &Type) -> &mut Impl {
    self.push_impl(Impl::new(target));

    match *self.items.last_mut().unwrap() {
      Item::Impl(ref mut v) => v,
      _ => unreachable!(),
    }
  }

  /// Push an `impl` block.
  pub fn push_impl(&mut self, item: Impl) -> &mut Self {
    self.items.push(Item::Impl(item));
    self
  }

  /// Push a raw string to the scope.
  ///
  /// This string will be included verbatim in the formatted string.
  pub fn raw(&mut self, val: &str) -> &mut Self {
    self.items.push(Item::Raw(val.to_string()));
    self
  }

  /// Return a string representation of the scope.
  pub fn to_string(&self) -> String {
    let mut ret = String::new();

    self.fmt(&mut Formatter::new(&mut ret)).unwrap();

    // Remove the trailing newline
    if ret.as_bytes().last() == Some(&b'\n') {
      ret.pop();
    }

    ret
  }

  /// Formats the scope using the given formatter.
  pub fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
    self.fmt_imports(fmt)?;

    if !self.imports.is_empty() {
      writeln!(fmt)?;
    }

    for (i, item) in self.items.iter().enumerate() {
      if i != 0 {
        writeln!(fmt)?;
      }

      match *item {
        Item::Module(ref v) => v.fmt(fmt)?,
        Item::Struct(ref v) => v.fmt(fmt)?,
        Item::Function(ref v) => v.fmt(false, fmt)?,
        Item::Trait(ref v) => v.fmt(fmt)?,
        Item::Enum(ref v) => v.fmt(fmt)?,
        Item::Impl(ref v) => v.fmt(fmt)?,
        Item::TypeAlias(ref v) => v.fmt(fmt)?,
        Item::Raw(ref v) => {
          writeln!(fmt, "{}", v)?;
        }
      }
    }

    Ok(())
  }

  fn fmt_imports(&self, fmt: &mut Formatter) -> fmt::Result {
    // First, collect all visibilities
    let mut visibilities = vec![];

    for imports in self.imports.values() {
      for import in imports.values() {
        if !visibilities.contains(&import.vis) {
          visibilities.push(import.vis.clone());
        }
      }
    }

    let mut tys = vec![];

    // Loop over all visibilities and format the associated imports
    for vis in &visibilities {
      for (path, imports) in &self.imports {
        tys.clear();

        for (ty, import) in imports {
          if *vis == import.vis {
            tys.push(ty);
          }
        }

        if !tys.is_empty() {
          if let Some(ref vis) = *vis {
            write!(fmt, "{} ", vis)?;
          }

          write!(fmt, "use {}::", path)?;

          if tys.len() > 1 {
            write!(fmt, "{{")?;

            for (i, ty) in tys.iter().enumerate() {
              if i != 0 {
                write!(fmt, ", ")?;
              }
              write!(fmt, "{}", ty)?;
            }

            writeln!(fmt, "}};")?;
          } else if tys.len() == 1 {
            writeln!(fmt, "{};", tys[0])?;
          }
        }
      }
    }

    Ok(())
  }
}

// ===== impl Module =====

impl Module {
  /// Return a new, blank module
  pub fn new(name: &str) -> Self {
    Module {
      name: name.to_string(),
      vis: None,
      docs: None,
      scope: Scope::new(),
    }
  }

  /// Returns a mutable reference to the module's scope.
  pub fn scope(&mut self) -> &mut Scope {
    &mut self.scope
  }

  /// Set the module visibility.
  pub fn vis(mut self, vis: &str) -> Self {
    self.vis = Some(vis.to_string());
    self
  }

  /// Import a type into the module's scope.
  ///
  /// This results in a new `use` statement bein added to the beginning of the
  /// module.
  pub fn import(&mut self, path: &str, ty: &str) -> &mut Self {
    self.scope.import(path, ty);
    self
  }

  /// Push a new module definition, returning a mutable reference to it.
  ///
  /// # Panics
  ///
  /// Since a module's name must uniquely identify it within the scope in
  /// which it is defined, pushing a module whose name is already defined
  /// in this scope will cause this function to panic.
  ///
  /// In many cases, the [`get_or_new_module`] function is preferrable, as it
  /// will return the existing definition instead.
  ///
  /// [`get_or_new_module`]: #method.get_or_new_module
  pub fn new_module(&mut self, name: &str) -> &mut Module {
    self.scope.new_module(name)
  }

  /// Returns a reference to a module if it is exists in this scope.
  pub fn get_module<Q: ?Sized>(&self, name: &Q) -> Option<&Module>
  where
    String: PartialEq<Q>,
  {
    self.scope.get_module(name)
  }

  /// Returns a mutable reference to a module if it is exists in this scope.
  pub fn get_module_mut<Q: ?Sized>(&mut self, name: &Q) -> Option<&mut Module>
  where
    String: PartialEq<Q>,
  {
    self.scope.get_module_mut(name)
  }

  /// Returns a mutable reference to a module, creating it if it does
  /// not exist.
  pub fn get_or_new_module(&mut self, name: &str) -> &mut Module {
    self.scope.get_or_new_module(name)
  }

  // pub fn merge_module(&mut self, other: Module) {
  //   if let Some(module) = self.get_module_mut(&other.name) {
  //     if let Some(doc) = &mut module.docs {
  //       if let Some(other_doc) = other.docs {
  //         if let Some(last_char) = doc.docs.chars().last() {
  //           if last_char != '\n' {
  //             doc.docs.push('\n');
  //           }
  //         }
  //         doc.docs.push_str(&other_doc.docs)
  //       }
  //     } else {
  //       module.docs = other.docs;
  //     }

  //     if let Some(doc) = &mut module.scope.docs {
  //       if let Some(other_doc) = other.scope.docs {
  //         if let Some(last_char) = doc.docs.chars().last() {
  //           if last_char != '\n' {
  //             doc.docs.push('\n');
  //           }
  //         }
  //         doc.docs.push_str(&other_doc.docs)
  //       }
  //     } else {
  //       module.scope.docs = other.scope.docs;
  //     }

  //     other.scope.imports
  //   } else {
  //     self.push_module(other);
  //   }
  // }

  /// Push a module definition.
  ///
  /// # Panics
  ///
  /// Since a module's name must uniquely identify it within the scope in
  /// which it is defined, pushing a module whose name is already defined
  /// in this scope will cause this function to panic.
  ///
  /// In many cases, the [`get_or_new_module`] function is preferrable, as it will
  /// return the existing definition instead.
  ///
  /// [`get_or_new_module`]: #method.get_or_new_module
  pub fn push_module(&mut self, item: Module) -> &mut Self {
    self.scope.push_module(item);
    self
  }

  /// Push a new struct definition, returning a mutable reference to it.
  pub fn new_struct(&mut self, xml_name: Option<XsdName>, name: &str) -> &mut Struct {
    self.scope.new_struct(xml_name, name)
  }

  /// Push a structure definition
  pub fn push_struct(&mut self, item: Struct) -> &mut Self {
    self.scope.push_struct(item);
    self
  }

  /// Push a new function definition, returning a mutable reference to it.
  pub fn new_fn(&mut self, name: &str) -> &mut Function {
    self.scope.new_fn(name)
  }

  /// Push a function definition
  pub fn push_fn(&mut self, item: Function) -> &mut Self {
    self.scope.push_fn(item);
    self
  }

  /// Push a new enum definition, returning a mutable reference to it.
  pub fn new_enum(&mut self, xml_name: Option<XsdName>, name: &str) -> &mut Enum {
    self.scope.new_enum(xml_name, name)
  }

  /// Push an enum definition
  pub fn push_enum(&mut self, item: Enum) -> &mut Self {
    self.scope.push_enum(item);
    self
  }

  pub fn push_type_alias(&mut self, item: TypeAlias) -> &mut Self {
    self.scope.push_type_alias(item);
    self
  }

  /// Push a new `impl` block, returning a mutable reference to it.
  pub fn new_impl(&mut self, target: &Type) -> &mut Impl {
    self.scope.new_impl(target)
  }

  /// Push an `impl` block.
  pub fn push_impl(&mut self, item: Impl) -> &mut Self {
    self.scope.push_impl(item);
    self
  }

  /// Push a trait definition
  pub fn push_trait(&mut self, item: Trait) -> &mut Self {
    self.scope.push_trait(item);
    self
  }

  /// Formats the module using the given formatter.
  pub fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
    if let Some(ref vis) = self.vis {
      write!(fmt, "{} ", vis)?;
    }

    write!(fmt, "mod {}", self.name)?;
    fmt.block(|fmt| self.scope.fmt(fmt))
  }
}

// ===== impl Struct =====

impl Struct {
  /// Return a structure definition with the provided name
  pub fn new(xml_name: Option<XsdName>, name: &str) -> Self {
    Struct {
      type_def: TypeDef::new(xml_name, name),
      fields: Fields::Empty,
    }
  }

  /// Returns a reference to the type
  pub fn ty(&self) -> &Type {
    &self.type_def.ty
  }

  /// Set the structure visibility.
  pub fn vis(mut self, vis: &str) -> Self {
    self.type_def.vis(vis);
    self
  }

  /// Add a generic to the struct.
  pub fn generic(mut self, ty: &Type) -> Self {
    self.type_def.ty = self.type_def.ty.generic(ty);
    self
  }

  /// Add a `where` bound to the struct.
  pub fn bound<T>(&mut self, name: &str, ty: T) -> &mut Self
  where
    T: Into<Type>,
  {
    self.type_def.bound(name, ty);
    self
  }

  /// Set the structure documentation.
  pub fn doc(&mut self, docs: &str) -> &mut Self {
    self.type_def.doc(docs);
    self
  }

  /// Add a new type that the struct should derive.
  pub fn derive(&mut self, name: &str) -> &mut Self {
    self.type_def.derive(name);
    self
  }

  /// Add new types that the struct should derive.
  pub fn derives(mut self, name: &[&str]) -> Self {
    for n in name {
      self.type_def.derive(n);
    }
    self
  }

  /// Specify lint attribute to supress a warning or error.
  pub fn allow(&mut self, allow: &str) -> &mut Self {
    self.type_def.allow(allow);
    self
  }

  /// Specify representation.
  pub fn repr(&mut self, repr: &str) -> &mut Self {
    self.type_def.repr(repr);
    self
  }

  /// Push a named field to the struct.
  ///
  /// A struct can either set named fields with this function or tuple fields
  /// with `push_tuple_field`, but not both.
  pub fn push_field(&mut self, field: Field) -> &mut Self {
    self.fields.push_named(field);
    self
  }

  /// Add a named field to the struct.
  ///
  /// A struct can either set named fields with this function or tuple fields
  /// with `tuple_field`, but not both.
  pub fn field<T>(
    &mut self,
    xml_name: Option<XsdName>,
    name: &str,
    ty: T,
    attribute: bool,
    flatten: bool,
  ) -> &mut Self
  where
    T: Into<Type>,
  {
    self.fields.named(xml_name, name, ty, attribute, flatten);
    self
  }

  /// Add a tuple field to the struct.
  ///
  /// A struct can either set tuple fields with this function or named fields
  /// with `field`, but not both.
  pub fn tuple_field<T>(mut self, vis: Option<&str>, ty: T, attribute: bool, flatten: bool) -> Self
  where
    T: Into<Type>,
  {
    self.fields.tuple(vis, ty, attribute, flatten);
    self
  }

  /// Formats the struct using the given formatter.
  pub fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
    self.type_def.fmt_head("struct", &[], fmt)?;
    self.fields.fmt(fmt)?;

    match self.fields {
      Fields::Empty => {
        writeln!(fmt, ";")?;
      }
      Fields::Tuple(..) => {
        writeln!(fmt, ";")?;
      }
      _ => {}
    }

    Ok(())
  }
}

// ===== impl Trait =====

impl Trait {
  /// Return a trait definition with the provided name
  pub fn new(name: &str, xml_name: Option<XsdName>) -> Self {
    Trait {
      type_def: TypeDef::new(xml_name, name),
      parents: vec![],
      associated_tys: vec![],
      fns: vec![],
      macros: vec![],
    }
  }

  /// Returns a reference to the type
  pub fn ty(&self) -> &Type {
    &self.type_def.ty
  }

  /// Set the trait visibility.
  pub fn vis(mut self, vis: &str) -> Self {
    self.type_def.vis(vis);
    self
  }

  /// Add a generic to the trait
  pub fn generic(mut self, name: &Type) -> Self {
    self.type_def.ty = self.type_def.ty.generic(name);
    self
  }

  /// Add a `where` bound to the trait.
  pub fn bound<T>(&mut self, name: &str, ty: T) -> &mut Self
  where
    T: Into<Type>,
  {
    self.type_def.bound(name, ty);
    self
  }

  /// Add a macro to the trait def (e.g. `"#[async_trait]"`)
  pub fn r#macro(&mut self, r#macro: &str) -> &mut Self {
    self.type_def.r#macro(r#macro);
    self
  }

  /// Add a parent trait.
  pub fn parent<T>(&mut self, ty: T) -> &mut Self
  where
    T: Into<Type>,
  {
    self.parents.push(ty.into());
    self
  }

  /// Set the trait documentation.
  pub fn doc(&mut self, docs: &str) -> &mut Self {
    self.type_def.doc(docs);
    self
  }

  /// Add an associated type. Returns a mutable reference to the new
  /// associated type for futher configuration.
  pub fn associated_type(&mut self, name: &str) -> &mut AssociatedType {
    self.associated_tys.push(AssociatedType(Bound {
      name: name.to_string(),
      bound: vec![],
    }));

    self.associated_tys.last_mut().unwrap()
  }

  /// Push a new function definition, returning a mutable reference to it.
  pub fn new_fn(&mut self, name: &str) -> &mut Function {
    let mut func = Function::new(name);
    func.body = None;

    self.push_fn(func);
    self.fns.last_mut().unwrap()
  }

  /// Push a function definition.
  pub fn push_fn(&mut self, item: Function) -> &mut Self {
    self.fns.push(item);
    self
  }

  /// Formats the scope using the given formatter.
  pub fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
    self.type_def.fmt_head("trait", &self.parents, fmt)?;

    fmt.block(|fmt| {
      let assoc = &self.associated_tys;

      // format associated types
      if !assoc.is_empty() {
        for ty in assoc {
          let ty = &ty.0;

          write!(fmt, "type {}", ty.name)?;

          if !ty.bound.is_empty() {
            write!(fmt, ": ")?;
            fmt_bound_rhs(&ty.bound, fmt)?;
          }

          writeln!(fmt, ";")?;
        }
      }

      for (i, func) in self.fns.iter().enumerate() {
        if i != 0 || !assoc.is_empty() {
          writeln!(fmt)?;
        }

        func.fmt(true, fmt)?;
      }

      Ok(())
    })
  }
}

// ===== impl Enum =====

impl Enum {
  /// Return a enum definition with the provided name.
  pub fn new(xml_name: Option<XsdName>, name: &str) -> Self {
    Enum {
      type_def: TypeDef::new(xml_name, name),
      variants: vec![],
    }
  }

  /// Returns a reference to the type.
  pub fn ty(&self) -> &Type {
    &self.type_def.ty
  }

  /// Set the enum visibility.
  pub fn vis(mut self, vis: &str) -> Self {
    self.type_def.vis(vis);
    self
  }

  /// Add a generic to the enum.
  pub fn generic(mut self, name: &Type) -> Self {
    self.type_def.ty = self.type_def.ty.generic(name);
    self
  }

  /// Add a `where` bound to the enum.
  pub fn bound<T>(&mut self, name: &str, ty: T) -> &mut Self
  where
    T: Into<Type>,
  {
    self.type_def.bound(name, ty);
    self
  }

  /// Set the enum documentation.
  pub fn doc(&mut self, docs: &str) -> &mut Self {
    self.type_def.doc(docs);
    self
  }

  /// Add new types that the struct should derive.
  pub fn derives(mut self, name: &[&str]) -> Self {
    for n in name {
      self.type_def.derive(n);
    }
    self
  }

  /// Add a new type that the struct should derive.
  pub fn derive(&mut self, name: &str) -> &mut Self {
    self.type_def.derive(name);
    self
  }

  /// Specify lint attribute to supress a warning or error.
  pub fn allow(&mut self, allow: &str) -> &mut Self {
    self.type_def.allow(allow);
    self
  }

  /// Specify representation.
  pub fn repr(&mut self, repr: &str) -> &mut Self {
    self.type_def.repr(repr);
    self
  }

  /// Push a variant to the enum, returning a mutable reference to it.
  pub fn new_variant(&mut self, xml_name: Option<XsdName>, name: &str) -> &mut Variant {
    self.variants.push(Variant::new(xml_name, name));
    self.variants.last_mut().unwrap()
  }

  /// Push a variant to the enum.
  pub fn push_variant(mut self, item: Variant) -> Self {
    self.variants.push(item);
    self
  }

  /// Formats the enum using the given formatter.
  pub fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
    self.type_def.fmt_head("enum", &[], fmt)?;

    fmt.block(|fmt| {
      for variant in &self.variants {
        variant.fmt(fmt)?;
      }

      Ok(())
    })
  }
}

// ===== impl Variant =====

impl Variant {
  /// Return a new enum variant with the given name.
  pub fn new(xml_name: Option<XsdName>, name: &str) -> Self {
    Variant {
      name: name.to_string(),
      fields: Fields::Empty,
      attributes: String::new(),
      xml_name,
      doc: None,
    }
  }

  pub fn attribute(mut self, attribute: &str) -> Self {
    self.attributes.push_str(attribute);
    self
  }

  /// Add a named field to the variant.
  pub fn named<T>(
    mut self,
    xml_name: Option<XsdName>,
    name: &str,
    ty: T,
    attribute: bool,
    flatten: bool,
  ) -> Self
  where
    T: Into<Type>,
  {
    self.fields.named(xml_name, name, ty, attribute, flatten);
    self
  }

  /// Add a tuple field to the variant.
  pub fn tuple(
    mut self,
    vis: Option<&str>,
    ty: impl Into<Type>,
    attribute: bool,
    flatten: bool,
  ) -> Self {
    self.fields.tuple(vis, ty, attribute, flatten);
    self
  }

  /// Formats the variant using the given formatter.
  pub fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
    write!(fmt, "{}", self.name)?;
    self.fields.fmt(fmt)?;
    writeln!(fmt, ",")?;

    Ok(())
  }
}

// ===== impl Type =====

impl Type {
  /// Return a new type with the given name.
  pub fn new(xml_name: Option<XsdName>, name: &str) -> Self {
    Type {
      xml_name,
      name: name.to_string(),
      generics: vec![],
      docs: None,
    }
  }

  pub fn doc(&mut self, docs: &str) {
    self.docs = Some(Docs::new(docs));
  }

  pub fn xml_name(mut self, xml_name: Option<XsdName>) -> Self {
    self.xml_name = xml_name;
    self
  }

  pub fn prefix(mut self, prefix: &str) -> Self {
    self.name = format!("{}{}", prefix, self.name);

    self
  }

  pub fn wrap(mut self, ty: &str) -> Self {
    self.generics = vec![self.clone()];
    self.name = ty.to_string();

    self
  }

  /// Add a generic to the type.
  pub fn generic<T>(mut self, ty: T) -> Self
  where
    T: Into<Type>,
  {
    // Make sure that the name doesn't already include generics
    assert!(
      !self.name.contains('<'),
      "type name already includes generics"
    );

    self.generics.push(ty.into());
    self
  }

  /// Rewrite the `Type` with the provided path
  pub fn path(&self, path: &str) -> Type {
    assert!(!self.name.contains("::"));

    let mut name = path.to_string();
    name.push_str("::");
    name.push_str(&self.name);

    Type {
      name,
      generics: self.generics.clone(),
      xml_name: self.xml_name.clone(),
      docs: self.docs.clone(),
    }
  }

  pub fn to_string(&self) -> String {
    let mut dst = String::new();
    let mut formatter = Formatter::new(&mut dst);
    self.fmt(&mut formatter);

    dst
  }

  /// Formats the struct using the given formatter.
  pub fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
    write!(fmt, "{}", self.name)?;
    Type::fmt_slice(&self.generics, fmt)
  }

  fn fmt_slice(generics: &[Type], fmt: &mut Formatter) -> fmt::Result {
    if !generics.is_empty() {
      write!(fmt, "<")?;

      for (i, ty) in generics.iter().enumerate() {
        if i != 0 {
          write!(fmt, ", ")?
        }
        ty.fmt(fmt)?;
      }

      write!(fmt, ">")?;
    }

    Ok(())
  }
}

impl<'a> From<&'a str> for Type {
  fn from(src: &'a str) -> Self {
    Type::new(None, src)
  }
}

impl From<String> for Type {
  fn from(src: String) -> Self {
    Type {
      name: src,
      generics: vec![],
      xml_name: None,
      docs: None,
    }
  }
}

impl<'a> From<&'a String> for Type {
  fn from(src: &'a String) -> Self {
    Type::new(None, src)
  }
}

impl<'a> From<&'a Type> for Type {
  fn from(src: &'a Type) -> Self {
    src.clone()
  }
}

// ===== impl TypeDef =====

impl TypeDef {
  /// Return a structure definition with the provided name
  fn new(xml_name: Option<XsdName>, name: &str) -> Self {
    TypeDef {
      ty: Type::new(xml_name, name),
      vis: Some("pub".to_string()),
      docs: None,
      derive: vec![],
      allow: vec![],
      repr: None,
      bounds: vec![],
      macros: vec![],
    }
  }

  fn vis(&mut self, vis: &str) {
    self.vis = Some(vis.to_string());
  }

  fn bound<T>(&mut self, name: &str, ty: T)
  where
    T: Into<Type>,
  {
    self.bounds.push(Bound {
      name: name.to_string(),
      bound: vec![ty.into()],
    });
  }

  fn r#macro(&mut self, r#macro: &str) {
    self.macros.push(r#macro.to_string());
  }

  fn doc(&mut self, docs: &str) {
    self.docs = Some(Docs::new(docs));
  }

  fn derive(&mut self, name: &str) {
    self.derive.push(name.to_string());
  }

  fn allow(&mut self, allow: &str) {
    self.allow.push(allow.to_string());
  }

  fn repr(&mut self, repr: &str) {
    self.repr = Some(repr.to_string());
  }

  fn fmt_head(&self, keyword: &str, parents: &[Type], fmt: &mut Formatter) -> fmt::Result {
    if let Some(ref docs) = self.docs {
      docs.fmt(fmt)?;
    }

    self.fmt_allow(fmt)?;
    self.fmt_derive(fmt)?;
    self.fmt_repr(fmt)?;
    self.fmt_macros(fmt)?;

    if let Some(ref vis) = self.vis {
      write!(fmt, "{} ", vis)?;
    }

    write!(fmt, "{} ", keyword)?;
    self.ty.fmt(fmt)?;

    if !parents.is_empty() {
      for (i, ty) in parents.iter().enumerate() {
        if i == 0 {
          write!(fmt, ": ")?;
        } else {
          write!(fmt, " + ")?;
        }

        ty.fmt(fmt)?;
      }
    }

    fmt_bounds(&self.bounds, fmt)?;

    Ok(())
  }

  fn fmt_allow(&self, fmt: &mut Formatter) -> fmt::Result {
    if !self.allow.is_empty() {
      writeln!(fmt, "#[allow({})]", self.allow.join(", "))?;
    }

    Ok(())
  }

  fn fmt_repr(&self, fmt: &mut Formatter) -> fmt::Result {
    if let Some(ref repr) = self.repr {
      writeln!(fmt, "#[repr({})]", repr)?;
    }

    Ok(())
  }

  fn fmt_derive(&self, fmt: &mut Formatter) -> fmt::Result {
    if !self.derive.is_empty() {
      write!(fmt, "#[derive(")?;

      for (i, name) in self.derive.iter().enumerate() {
        if i != 0 {
          write!(fmt, ", ")?
        }
        write!(fmt, "{}", name)?;
      }

      writeln!(fmt, ")]")?;
    }

    Ok(())
  }

  fn fmt_macros(&self, fmt: &mut Formatter) -> fmt::Result {
    for m in self.macros.iter() {
      writeln!(fmt, "{}", m)?;
    }
    Ok(())
  }
}

fn fmt_generics(generics: &[String], fmt: &mut Formatter) -> fmt::Result {
  if !generics.is_empty() {
    write!(fmt, "<")?;

    for (i, ty) in generics.iter().enumerate() {
      if i != 0 {
        write!(fmt, ", ")?
      }
      write!(fmt, "{}", ty)?;
    }

    write!(fmt, ">")?;
  }

  Ok(())
}

fn fmt_bounds(bounds: &[Bound], fmt: &mut Formatter) -> fmt::Result {
  if !bounds.is_empty() {
    writeln!(fmt)?;

    // Write first bound
    write!(fmt, "where {}: ", bounds[0].name)?;
    fmt_bound_rhs(&bounds[0].bound, fmt)?;
    writeln!(fmt, ",")?;

    for bound in &bounds[1..] {
      write!(fmt, "      {}: ", bound.name)?;
      fmt_bound_rhs(&bound.bound, fmt)?;
      writeln!(fmt, ",")?;
    }
  }

  Ok(())
}

fn fmt_bound_rhs(tys: &[Type], fmt: &mut Formatter) -> fmt::Result {
  for (i, ty) in tys.iter().enumerate() {
    if i != 0 {
      write!(fmt, " + ")?
    }
    ty.fmt(fmt)?;
  }

  Ok(())
}

// ===== impl AssociatedType =====

impl AssociatedType {
  /// Add a bound to the associated type.
  pub fn bound<T>(&mut self, ty: T) -> &mut Self
  where
    T: Into<Type>,
  {
    self.0.bound.push(ty.into());
    self
  }
}

// ===== impl Field =====

impl Field {
  /// Return a field definition with the provided name and type
  pub fn new<T>(
    xml_name: Option<XsdName>,
    name: &str,
    ty: T,
    attribute: bool,
    flatten: bool,
  ) -> Self
  where
    T: Into<Type>,
  {
    Field {
      name: name.into(),
      ty: ty.into(),
      vis: None,
      documentation: Vec::new(),
      annotation: Vec::new(),
      xml_name,
      attribute,
      flatten,
    }
  }

  /// Set field's documentation.
  pub fn doc(&mut self, documentation: Vec<&str>) -> &mut Self {
    self.documentation = documentation.iter().map(|doc| doc.to_string()).collect();
    self
  }

  /// Set field's annotation.
  pub fn annotation(mut self, annotation: Vec<&str>) -> Self {
    self.annotation = annotation.iter().map(|ann| ann.to_string()).collect();
    self
  }

  pub fn vis(mut self, vis: &str) -> Self {
    self.vis = Some(vis.to_string());
    self
  }
}

// ===== impl Fields =====

impl Fields {
  pub fn push_named(&mut self, field: Field) -> &mut Self {
    match *self {
      Fields::Empty => {
        *self = Fields::Named(vec![field]);
      }
      Fields::Named(ref mut fields) => {
        fields.push(field);
      }
      _ => panic!("field list is named"),
    }

    self
  }

  pub fn named<T>(
    &mut self,
    xml_name: Option<XsdName>,
    name: &str,
    ty: T,
    attribute: bool,
    flatten: bool,
  ) -> &mut Self
  where
    T: Into<Type>,
  {
    self.push_named(Field {
      name: name.to_string(),
      ty: ty.into(),
      vis: None,
      documentation: Vec::new(),
      annotation: Vec::new(),
      xml_name,
      attribute,
      flatten,
    })
  }

  pub fn tuple_vis<T>(&mut self, vis: &str, ty: T, attribute: bool, flatten: bool) -> &mut Self
  where
    T: Into<Type>,
  {
    match *self {
      Fields::Empty => {
        *self = Fields::Tuple(vec![TupleField {
          vis: Some(vis.to_string()),
          ty: ty.into(),
          attribute,
          flatten,
        }]);
      }
      Fields::Tuple(ref mut fields) => {
        fields.push(TupleField {
          vis: Some(vis.to_string()),
          ty: ty.into(),
          attribute,
          flatten,
        });
      }
      _ => panic!("field list is tuple"),
    }

    self
  }

  pub fn tuple<T>(&mut self, vis: Option<&str>, ty: T, attribute: bool, flatten: bool) -> &mut Self
  where
    T: Into<Type>,
  {
    match *self {
      Fields::Empty => {
        *self = Fields::Tuple(vec![TupleField {
          vis: vis.map(|v| v.to_string()),
          ty: ty.into(),
          attribute,
          flatten,
        }]);
      }
      Fields::Tuple(ref mut fields) => {
        fields.push(TupleField {
          vis: vis.map(|v| v.to_string()),
          ty: ty.into(),
          attribute,
          flatten,
        });
      }
      _ => panic!("field list is tuple"),
    }

    self
  }

  fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
    match *self {
      Fields::Named(ref fields) => {
        assert!(!fields.is_empty());

        fmt.block(|fmt| {
          for f in fields {
            if !f.documentation.is_empty() {
              for doc in &f.documentation {
                writeln!(fmt, "/// {}", doc)?;
              }
            }
            if !f.annotation.is_empty() {
              for ann in &f.annotation {
                writeln!(fmt, "{}", ann)?;
              }
            }
            write!(
              fmt,
              "{}{}{}: ",
              f.vis.as_deref().unwrap_or(""),
              if f.vis.is_some() { " " } else { "" },
              f.name
            )?;
            f.ty.fmt(fmt)?;
            writeln!(fmt, ",")?;
          }

          Ok(())
        })?;
      }
      Fields::Tuple(ref tys) => {
        assert!(!tys.is_empty());

        write!(fmt, "(")?;

        for (i, TupleField { vis, ty, .. }) in tys.iter().enumerate() {
          if i != 0 {
            write!(fmt, ", ")?;
          }
          if let Some(vis) = vis {
            write!(fmt, "{} ", vis)?;
          }

          ty.fmt(fmt)?;
        }

        write!(fmt, ")")?;
      }
      Fields::Empty => {}
    }

    Ok(())
  }
}

// ===== impl Impl =====

impl Impl {
  /// Return a new impl definition
  pub fn new<T>(target: T) -> Self
  where
    T: Into<Type>,
  {
    Impl {
      target: target.into(),
      generics: vec![],
      impl_trait: None,
      assoc_tys: vec![],
      bounds: vec![],
      fns: vec![],
      macros: vec![],
    }
  }

  /// Add a generic to the impl block.
  ///
  /// This adds the generic for the block (`impl<T>`) and not the target type.
  pub fn generic(mut self, name: &str) -> Self {
    self.generics.push(name.to_string());
    self
  }

  /// Add a generic to the target type.
  pub fn target_generic<T>(mut self, ty: T) -> Self
  where
    T: Into<Type>,
  {
    self.target = self.target.generic(ty);
    self
  }

  /// Set the trait that the impl block is implementing.
  pub fn impl_trait<T>(mut self, ty: T) -> Self
  where
    T: Into<Type>,
  {
    self.impl_trait = Some(ty.into());
    self
  }

  /// Add a macro to the impl block (e.g. `"#[async_trait]"`)
  pub fn r#macro(&mut self, r#macro: &str) -> &mut Self {
    self.macros.push(r#macro.to_string());
    self
  }

  /// Set an associated type.
  pub fn associate_type<T>(
    &mut self,
    xml_name: Option<XsdName>,
    name: &str,
    ty: T,
    attribute: bool,
    flatten: bool,
  ) -> &mut Self
  where
    T: Into<Type>,
  {
    self.assoc_tys.push(Field {
      name: name.to_string(),
      ty: ty.into(),
      vis: None,
      documentation: Vec::new(),
      annotation: Vec::new(),
      xml_name,
      attribute,
      flatten,
    });

    self
  }

  /// Add a `where` bound to the impl block.
  pub fn bound<T>(&mut self, name: &str, ty: T) -> &mut Self
  where
    T: Into<Type>,
  {
    self.bounds.push(Bound {
      name: name.to_string(),
      bound: vec![ty.into()],
    });
    self
  }

  /// Push a function definition.
  pub fn push_fn(mut self, item: Function) -> Self {
    self.fns.push(item);
    self
  }

  /// Formats the impl block using the given formatter.
  pub fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
    for m in self.macros.iter() {
      writeln!(fmt, "{}", m)?;
    }
    write!(fmt, "impl")?;
    fmt_generics(&self.generics[..], fmt)?;

    if let Some(ref t) = self.impl_trait {
      write!(fmt, " ")?;
      t.fmt(fmt)?;
      write!(fmt, " for")?;
    }

    write!(fmt, " ")?;
    self.target.fmt(fmt)?;

    fmt_bounds(&self.bounds, fmt)?;

    fmt.block(|fmt| {
      // format associated types
      if !self.assoc_tys.is_empty() {
        for ty in &self.assoc_tys {
          write!(fmt, "type {} = ", ty.name)?;
          ty.ty.fmt(fmt)?;
          writeln!(fmt, ";")?;
        }
      }

      for (i, func) in self.fns.iter().enumerate() {
        if i != 0 || !self.assoc_tys.is_empty() {
          writeln!(fmt)?;
        }

        func.fmt(false, fmt)?;
      }

      Ok(())
    })
  }
}

// ===== impl Import =====

impl Import {
  /// Return a new import.
  pub fn new(path: &str, ty: &str) -> Self {
    Import {
      line: format!("{}::{}", path, ty),
      vis: None,
    }
  }

  /// Set the import visibility.
  pub fn vis(&mut self, vis: &str) -> &mut Self {
    self.vis = Some(vis.to_string());
    self
  }
}

// ===== impl Function =====

impl Function {
  /// Return a new function definition.
  pub fn new(name: &str) -> Self {
    Function {
      name: name.to_string(),
      docs: None,
      allow: None,
      vis: None,
      generics: vec![],
      arg_self: None,
      args: vec![],
      ret: None,
      bounds: vec![],
      body: Some(vec![]),
      attributes: vec![],
      extern_abi: None,
      r#async: false,
    }
  }

  /// Set the function documentation.
  pub fn doc(mut self, docs: &str) -> Self {
    self.docs = Some(Docs::new(docs));
    self
  }

  /// Specify lint attribute to supress a warning or error.
  pub fn allow(mut self, allow: &str) -> Self {
    self.allow = Some(allow.to_string());
    self
  }

  /// Set the function visibility.
  pub fn vis(mut self, vis: &str) -> Self {
    self.vis = Some(vis.to_string());
    self
  }

  /// Set whether this function is async or not
  pub fn set_async(mut self, r#async: bool) -> Self {
    self.r#async = r#async;
    self
  }

  /// Add a generic to the function.
  pub fn generic(mut self, name: &str) -> Self {
    self.generics.push(name.to_string());
    self
  }

  /// Add `self` as a function argument.
  pub fn arg_self(mut self) -> Self {
    self.arg_self = Some("self".to_string());
    self
  }

  /// Add `&self` as a function argument.
  pub fn arg_ref_self(mut self) -> Self {
    self.arg_self = Some("&self".to_string());
    self
  }

  /// Add `&mut self` as a function argument.
  pub fn arg_mut_self(&mut self) -> &mut Self {
    self.arg_self = Some("&mut self".to_string());
    self
  }

  /// Add a function argument.
  pub fn arg<T>(mut self, name: &str, ty: T) -> Self
  where
    T: Into<Type>,
  {
    self.args.push(Field {
      name: name.to_string(),
      ty: ty.into(),
      vis: None,
      // While a `Field` is used here, both `documentation`
      // and `annotation` does not make sense for function arguments.
      // Simply use empty strings.
      documentation: Vec::new(),
      annotation: Vec::new(),
      xml_name: None,
      attribute: false,
      flatten: false,
    });

    self
  }

  /// Set the function return type.
  pub fn ret<T>(mut self, ty: T) -> Self
  where
    T: Into<Type>,
  {
    self.ret = Some(ty.into());
    self
  }

  /// Add a `where` bound to the function.
  pub fn bound<T>(&mut self, name: &str, ty: T) -> &mut Self
  where
    T: Into<Type>,
  {
    self.bounds.push(Bound {
      name: name.to_string(),
      bound: vec![ty.into()],
    });
    self
  }

  /// Push a line to the function implementation.
  pub fn line<T>(mut self, line: T) -> Self
  where
    T: ToString,
  {
    self
      .body
      .get_or_insert(vec![])
      .push(Body::String(line.to_string()));

    self
  }

  /// Add an attribute to the function.
  ///
  /// ```
  /// use codegen::Function;
  ///
  /// let mut func = Function::new("test");
  ///
  /// // add a `#[test]` attribute
  /// func.attr("test");
  /// ```
  pub fn attr(&mut self, attribute: &str) -> &mut Self {
    self.attributes.push(attribute.to_string());
    self
  }

  /// Specify an `extern` ABI for the function.
  /// ```
  /// use codegen::Function;
  ///
  /// let mut extern_func = Function::new("extern_func");
  ///
  /// // use the "C" calling convention
  /// extern_func.extern_abi("C");
  /// ```
  pub fn extern_abi(&mut self, abi: &str) -> &mut Self {
    self.extern_abi.replace(abi.to_string());
    self
  }

  /// Push a block to the function implementation
  pub fn push_block(mut self, block: Block) -> Self {
    self.body.get_or_insert(vec![]).push(Body::Block(block));

    self
  }

  /// Formats the function using the given formatter.
  pub fn fmt(&self, is_trait: bool, fmt: &mut Formatter) -> fmt::Result {
    if let Some(ref docs) = self.docs {
      docs.fmt(fmt)?;
    }

    if let Some(ref allow) = self.allow {
      writeln!(fmt, "#[allow({})]", allow)?;
    }

    for attr in self.attributes.iter() {
      writeln!(fmt, "#[{}]", attr)?;
    }

    if is_trait {
      assert!(
        self.vis.is_none(),
        "trait fns do not have visibility modifiers"
      );
    }

    if let Some(ref vis) = self.vis {
      write!(fmt, "{} ", vis)?;
    }

    if let Some(ref extern_abi) = self.extern_abi {
      write!(fmt, "extern \"{extern_abi}\" ", extern_abi = extern_abi)?;
    }

    if self.r#async {
      write!(fmt, "async ")?;
    }

    write!(fmt, "fn {}", self.name)?;
    fmt_generics(&self.generics, fmt)?;

    write!(fmt, "(")?;

    if let Some(ref s) = self.arg_self {
      write!(fmt, "{}", s)?;
    }

    for (i, arg) in self.args.iter().enumerate() {
      if i != 0 || self.arg_self.is_some() {
        write!(fmt, ", ")?;
      }

      write!(fmt, "{}: ", arg.name)?;
      arg.ty.fmt(fmt)?;
    }

    write!(fmt, ")")?;

    if let Some(ref ret) = self.ret {
      write!(fmt, " -> ")?;
      ret.fmt(fmt)?;
    }

    fmt_bounds(&self.bounds, fmt)?;

    match self.body {
      Some(ref body) => fmt.block(|fmt| {
        for b in body {
          b.fmt(fmt)?;
        }

        Ok(())
      }),
      None => {
        if !is_trait {
          panic!("impl blocks must define fn bodies");
        }

        writeln!(fmt, ";")
      }
    }
  }
}

// ===== impl Block =====

impl Block {
  /// Returns an empty code block.
  pub fn new(before: &str) -> Self {
    Block {
      before: Some(before.to_string()),
      after: None,
      body: vec![],
    }
  }

  /// Push a line to the code block.
  pub fn line<T>(mut self, line: T) -> Self
  where
    T: ToString,
  {
    self.body.push(Body::String(line.to_string()));
    self
  }

  /// Push a nested block to this block.
  pub fn push_block(mut self, block: Block) -> Self {
    self.body.push(Body::Block(block));
    self
  }

  /// Add a snippet after the block.
  pub fn after(mut self, after: &str) -> Self {
    self.after = Some(after.to_string());
    self
  }

  /// Formats the block using the given formatter.
  pub fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
    if let Some(ref before) = self.before {
      write!(fmt, "{}", before)?;
    }

    // Inlined `Formatter::fmt`

    if !fmt.is_start_of_line() {
      write!(fmt, " ")?;
    }

    writeln!(fmt, "{{")?;

    fmt.indent(|fmt| {
      for b in &self.body {
        b.fmt(fmt)?;
      }

      Ok(())
    })?;

    write!(fmt, "}}")?;

    if let Some(ref after) = self.after {
      write!(fmt, "{}", after)?;
    }

    writeln!(fmt)?;
    Ok(())
  }
}

// ===== impl Body =====

impl Body {
  fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
    match *self {
      Body::String(ref s) => writeln!(fmt, "{}", s),
      Body::Block(ref b) => b.fmt(fmt),
    }
  }
}

// ===== impl Docs =====

impl Docs {
  fn new(docs: &str) -> Self {
    Docs {
      docs: docs.to_string(),
    }
  }

  fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
    for line in self.docs.lines() {
      writeln!(fmt, "/// {}", line)?;
    }

    Ok(())
  }
}

// ===== impl Formatter =====

impl<'a> Formatter<'a> {
  /// Return a new formatter that writes to the given string.
  pub fn new(dst: &'a mut String) -> Self {
    Formatter {
      dst,
      spaces: 0,
      indent: DEFAULT_INDENT,
    }
  }

  fn block<F>(&mut self, f: F) -> fmt::Result
  where
    F: FnOnce(&mut Self) -> fmt::Result,
  {
    if !self.is_start_of_line() {
      write!(self, " ")?;
    }

    writeln!(self, "{{")?;
    self.indent(f)?;
    writeln!(self, "}}")?;
    Ok(())
  }

  /// Call the given function with the indentation level incremented by one.
  fn indent<F, R>(&mut self, f: F) -> R
  where
    F: FnOnce(&mut Self) -> R,
  {
    self.spaces += self.indent;
    let ret = f(self);
    self.spaces -= self.indent;
    ret
  }

  fn is_start_of_line(&self) -> bool {
    self.dst.is_empty() || self.dst.as_bytes().last() == Some(&b'\n')
  }

  fn push_spaces(&mut self) {
    for _ in 0..self.spaces {
      self.dst.push(' ');
    }
  }
}

impl<'a> fmt::Write for Formatter<'a> {
  fn write_str(&mut self, s: &str) -> fmt::Result {
    let mut first = true;
    let mut should_indent = self.is_start_of_line();

    for line in s.lines() {
      if !first {
        self.dst.push('\n');
      }

      first = false;

      let do_indent = should_indent && !line.is_empty() && line.as_bytes()[0] != b'\n';

      if do_indent {
        self.push_spaces();
      }

      // If this loops again, then we just wrote a new line
      should_indent = true;

      self.dst.push_str(line);
    }

    if s.as_bytes().last() == Some(&b'\n') {
      self.dst.push('\n');
    }

    Ok(())
  }
}
