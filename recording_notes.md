# Rough outline of the server

- https://github.com/security-union/videocall-rs/blob/main/actix-api/src/webtransport/mod.rs#L383-L511 handles webtransport connections to the server
  - https://github.com/security-union/videocall-rs/blob/main/actix-api/src/webtransport/mod.rs#L392-L444 sends messages from an actor to the client
  - https://github.com/security-union/videocall-rs/blob/main/actix-api/src/webtransport/mod.rs#L446-L489 sends messages from the client to an actor
- PACKET: https://github.com/security-union/videocall-rs/blob/main/actix-api/src/actors/chat_session.rs#L105-L123 handles incoming messages from the client
- CLIENT MESSAGE https://github.com/security-union/videocall-rs/blob/main/actix-api/src/actors/chat_server.rs#L76-L100 similar to PACKET, but works on a per room, rather than per user basis

PACKET and CLIENT MESSAGE both have access to the video data, and if E2EE is turned off, then it can be read. At the moment it is treated as a opaque blob, so it will need parsing and packing into a video container before being saved.
