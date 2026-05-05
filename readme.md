# NetForge

Building blocks for bespoke networking protocols. Each block (component) is responsible for doing
only one thing, while imposing minimal runtime overhead and architecture constraints.

## Disclaimer

The crate documentation assumes solid knowledge of [network](https://en.wikipedia.org/wiki/OSI_model#Layer_3:_Network_layer)
and [transport](https://en.wikipedia.org/wiki/OSI_model#Layer_4:_Transport_layer) OSI layers, as
well common networking pitfalls and dangers. It does not attempt to be an educational resource at
the moment.

## Supported functionality

This crate provides:
- [x] Reliable delivery over an unreliable link layer
- [x] Connection management
- [ ] Channel multiplexing
- [ ] Data validation
- [ ] Packet fragmentation and re-assembly

## Features

name       |description                                            |default
-----------|-------------------------------------------------------|-------
bincode    |derive bincode Encode and Decode traits for crate types|❌ no
wincode    |derive wincode Encode and Decode traits for crate types|❌ no
serde      |derive serde Serialize and Deserialize traits          |❌ no
bevy       |derive bevy_reflect Reflect trait for crate types      |❌ no
reliability|enable reliable delivery functionality                 |✔ yes
server     |enable server-side connection management               |✔ yes
client     |enable client-side connection management               |✔ yes

## Reasoning

The crate started off because the author didn't like existing rust networking libraries:
* [renet](https://github.com/lucaspoffo/renet) - provides an inflexible protocol, forcing the overhead for possibly unnecessary features
* [renet2](github.com/UkoeHB/renet2) - same as `renet`
* [aeronet](https://github.com/aecsocket/aeronet) - based on Bevy ECS, forcing a particular app architecture
* [carrier-pigeon](https://github.com/MitchellMarinoDev/carrier-pigeon) - based on Bevy ECS and mixes UDP and TCP sockets
* [bevy_quinnet](https://github.com/Henauxg/bevy_quinnet) - based on Bevy ECS and forces additional memcpy overhead to handle `quinn` async API
* [quinn](https://github.com/quinn-rs/quinn) - async API, forcing a particular app architecture. Additionally only supports full QUIC protocol, which may be too much for some applications
* 
## License

This work is dual-licensed under Apache 2.0 and GPL 2.0 (or any later version).
You can choose between one of them if you use this work.

`SPDX-License-Identifier: Apache-2.0 OR GPL-2.0-or-later`
