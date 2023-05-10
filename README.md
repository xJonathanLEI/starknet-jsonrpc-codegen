# StarkNet JSON-RPC Codegen

Tool for generating the StarkNet JSON-RPC code used in `starknet-rs`. StarkNet specs are shipped with this repo so it should work out of the box.

Run the tool and choose which version of the specification to use:

```console
$ cargo run -- --spec 0.2.1
```

and generated code will be emitted to `stdout`.

## Supported spec versions

The following versions are supported:

- `0.1.0`
- `0.2.1`
- `0.3.0` (this is an unreleased version; currently tracking commit [`94a9697`](https://github.com/starkware-libs/starknet-specs/commit/94a969751b31f5d3e25a0c6850c723ddadeeb679))

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](./LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](./LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
