- XsdName doesn't make sense to be the type of parent_name

[] Make parser namespace aware
[] Make parser prefix aware
[] Make generator namespace aware
[] Make parser prefix aware

[] Spit group into def and ref types
  - Will make generation easier to read through

[] Improve error handling to more easily trace errors
  - Ideally could trace back to initial document positions
[] Turn parsing into trait to reduce boilerplate

[] Make proc macro?

When name is not available.
  - If the parent-name would only be applied to a single element the parent-name is the name
  - The type is inferred
When ref is not available the name is both the typename and fieldname.
When ref is available the name is the fieldname; ref is the typename

Rewrite system, parent names are just strings.
XsdElementRef is a name, the XsdType and the namespace

Fix the element lookup, figure out any easy way to search for multiple types at once.
