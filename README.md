# Stagix

Stagix is a static git page generator providing a simple static view onto self-hosted git repos.

It is based on [stagit](https://codemadness.org/stagit.html), built in [Rust](https://www.rust-lang.org/) leveraging the new implementation of git in Rust [gitoxide](https://github.com/GitoxideLabs/gitoxide).

## Usage

Stagix provides two binaries: `stagix-repo` and `stagix-index`.

`stagix-repo` builds a tree of html pages for a single git repo.

`stagix-index` builds a single html document as a root page for linking together multiple repos processed with `stagix-repo`.

## Installing

### With Cargo

Stagix is build in Rust, you can use `cargo` to install it:

```sh
cargo install --git https://github.com/jeffa5/stagix
```

### With nix

Stagix can also be built with [nix](https://nixos.org/) and provides a flake to do so:

```sh
# from a clone of this repo
nix build .#stagix
# then run it with
./result/bin/stagix-repo
# or
./result/bin/stagix-index

# without cloning
nix shell github:jeffa5/stagix#stagix
# now stagix-repo and stagix-index are available in your path
```

## Building

Stagix is built in Rust, you can use `cargo` to build it like any other Rust project.

```sh
# for unoptimised development builds
cargo build
# for optimised release builds
cargo build --release
```
