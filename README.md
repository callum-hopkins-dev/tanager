<div align="center">

# tanager

Parse Rust literal struct expressions using syn.

[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/callum-hopkins-dev/tanager/build.yaml?branch=main&event=push&style=for-the-badge)](https://github.com/callum-hopkins-dev/tanager/actions/workflows/build.yaml)
[![Crates.io Version](https://img.shields.io/crates/v/tanager?style=for-the-badge)](https://crates.io/crates/tanager)
[![docs.rs](https://img.shields.io/docsrs/tanager?style=for-the-badge)](https://docs.rs/tanager/latest/tanager)
[![Crates.io Total Downloads](https://img.shields.io/crates/d/tanager?style=for-the-badge)](https://crates.io/crates/tanager)
[![GitHub License](https://img.shields.io/github/license/callum-hopkins-dev/tanager?style=for-the-badge)](https://github.com/callum-hopkins-dev/tanager/blob/main/LICENSE)

</div>

## about

`tanager` is intended for proc-macro authors who want to accept syntax that
resembles ordinary Rust struct expressions.

The crate provides a `Parse` trait and a derive macro for implementing it.
Parsed values are constructed directly from Rust literals and nested structures
rather than from custom attribute syntax.

## Example

```rust
use tanager::Parse;

#[derive(Parse)]
struct Config {
    name: String,
    enabled: bool,
}
```

This can be used to parse syntax such as:

```rust
{
    name: "example",
    enabled: true,
}
```

Most users will derive `Parse` and then use `tanager::parse` to parse a
`proc_macro2::TokenStream`.

The derive macro is available with the `macros` feature enabled.

## Derive Attributes

### `#[tanager(crate = path)]`

Overrides the path used to refer to the `tanager` crate in generated code.

This is primarily useful when re-exporting `tanager` or when the crate is
available under a different name.

```rust
#[derive(Parse)]
#[tanager(crate = my_tanager)]
struct Config {
    enabled: bool,
}
```

### `#[tanager(default = expr)]`

Provides a default value when a named field is omitted from the input.

```rust
#[derive(Parse)]
struct Config {
    required: String,

    #[tanager(default = true)]
    enabled: bool,
}
```

The supplied expression is evaluated whenever the field is omitted.

## License

Tanager is licensed under the MIT License. See `LICENSE` for details.

## Contributing

Contributions are welcome.

Please follow the existing code style and conventions used throughout the
project. If you're proposing a new feature or API, opening an issue first is
often the easiest way to discuss the design.

Suggestions and new ideas are appreciated, but maintainership and final design
decisions remain with the project owner.
