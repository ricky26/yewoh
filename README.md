# Yewoh
Yewoh (pronounced yew-oh) is an Ultima Online protocol library & server,
written in Rust, with bevy.

🚨🚧 This project is currently just a proof-of-concept and doesn't really
implement any game logic. 🚧 🚨

## _Why?_
I'm not sure there's a compelling reason for this to exist, however, I can give
my excuses.
- Rust is very good for protocols
- ECS is a great model for composable game logic (in contrast to OOP which is
  most commonly used in this arena)
- bevy is an awesome ECS game engine in Rust (and I wanted to use it for
  something)
- There's no standalone protocol library for UO

## What works?
At the moment, you can connect and walk around.
- Login is currently a no-op
- There is no persistence/ validation or game logic

## Starting the server
    cargo run --bin=yewoh-default-server --uo-data-path /path/to/UOClassic

The recommended (and only supported) client is ClassicUO.

By default the server will run **with encryption on**, so make sure you enable
that when creating your ClassicUO profile.

## Architecture
Crates:
- [yewoh](crates/core)
  - The protocol library which should be unopinonated and make it easy to send
    or parse any of the known Ultima Online packets.
- [yewoh-server](crates/server)
  - This crate contains a server implementation built against bevy.
  - It should implement all of the primitives which are shared between the
    client and server but none of the "game-specific" server-side logic.
    (e.g. AI, quests, spawners, etc)
- [yewoh-default-game](crates/default-game)
  - This create contains the extra "game-specific" functionality required to
    run a server which acts like stock Ultima Online.
- [yewoh-default-server](crates/default-server)
  - This crate builds a server binary for `yewoh-default-game`.
- [yewoh-client](crates/client) / [yewoh-bevy-client](crates/bevy-client)
  - These are earmarked to contain a simple client state tracker & client
    written against bevy.

## [License](LICENSE)
This project is licensed under the MIT license.

## [Contributing](CONTRIBUTING.md)
See the [contribution](CONTRIBUTING.md) page for information about contributing
to the project.

