# Starknet JSON-RPC Codegen

Tool for generating the Starknet JSON-RPC code used in `starknet-rs`. Starknet specs are shipped with this repo so it should work out of the box.

Run the tool and choose which version of the specification to use:

```console
$ cargo run -- generate --spec 0.8.1
```

and generated code will be emitted to `stdout`.

## Supported spec versions

The following versions are supported:

- `0.1.0`
- `0.2.1`
- `0.3.0`
- `0.4.0`
- `0.5.1`
- `0.6.0`
- `0.7.1`
- `0.8.1`
- `0.9.0-rc.0`

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](./LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
