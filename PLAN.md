Build me a complete MVP for a Nostr-native first person shooter called Oyabaun.

Core concept:
Oyabaun is a gritty early-90s-feeling FPS with a dark neo-Tokyo / yakuza atmosphere. The game should be multiplayer-first and built specifically around Nostr identity and relay-based communication. I want the architecture designed from the ground up for this exact game, not a generic game engine demo.

Tech stack requirements:
- Game client in Rust
- Compile to WebAssembly so it runs in the browser
- Rendering should use a browser-friendly Rust graphics stack suitable for wasm
- Networking and identity should be Nostr-native
- Backend should be a custom Go relay purpose-built for this game
- Do not use a general purpose off-the-shelf Nostr relay as the main design. I want a stripped down specialized relay optimized for fast game-state message flow
- Use Docker and Docker Compose for local development
- Use Postgres only if truly needed. Prefer in-memory or ultra-light persistence for the relay where possible
- Keep dependencies lean

High level product requirements:
- Name: Oyabaun
- Theme: dark, grounded, serious, yakuza power struggle
- Perspective: first person shooter
- Initial scope: multiplayer arena deathmatch MVP
- Browser playable
- Fast spawn, movement, shooting, hitscan weapons, health, death, respawn, scoreboard
- Retro aesthetic direction inspired by early 90s shooters, but technically modern under the hood
- The game should feel like it was built specifically for Nostr, not like a game with Nostr bolted on

Design principles:
- Optimize for responsiveness and simplicity over feature bloat
- Keep the MVP narrow and actually runnable
- Separate authoritative gameplay logic from presentation
- Be explicit about what is relay-authoritative versus client-predicted
- Design around latency and cheating concerns from the beginning
- All code should be clean, production-minded, and easy to extend

Identity and Nostr requirements:
- Users authenticate with Nostr
- Support browser extension signing flow if possible
- Player identity should be tied to pubkey
- Relay should accept only the event/message types needed for this game
- Define custom event kinds or a purpose-built message schema for:
  - session join
  - match state
  - player input
  - snapshots
  - shots fired
  - hits / damage
  - deaths
  - respawns
  - chat or emotes if lightweight enough
- Clearly separate signed identity/auth events from ephemeral low-latency gameplay messages
- Think carefully about which messages should be signed, which can be session-scoped, and how to avoid unnecessary cryptographic overhead on every frame
- Include a lightweight session token or relay-issued match token design after Nostr auth, if that is the right tradeoff

Relay requirements:
Build a custom Go relay specifically for Oyabaun with these goals:
- minimal protocol surface
- websocket-based
- accepts only game-relevant message types
- room / match based routing
- fast fanout of position and action updates
- simple matchmaking or lobby support for MVP
- authoritative validation for core events where possible
- rate limiting and abuse protections
- basic anti-cheat protections
- metrics/logging hooks
- modular code structure so it can evolve later

Game architecture requirements:
I want you to make good concrete decisions here, not ask me endless questions.

Please design:
1. Client architecture in Rust/wasm
2. Rendering/input loop
3. Player controller and camera
4. Collision and map representation
5. Weapon firing model
6. Netcode model
7. Match / room lifecycle
8. Nostr login flow
9. Relay protocol and message schema
10. State synchronization strategy
11. Cheat resistance strategy
12. Local development workflow

Gameplay requirements for MVP:
- one small playable arena map
- WASD movement
- mouse look
- jump if feasible
- one hitscan pistol or SMG
- health and damage
- kill/death/respawn loop
- scoreboard
- basic HUD
- basic match join flow
- at least 2+ players supported in a room
- placeholder art is fine, but it should look cohesive and intentionally retro

Visual direction:
- gritty retro FPS vibe
- dark neon Tokyo underworld atmosphere
- low-res textures
- chunky geometry
- simple lighting
- UI should be minimal and sharp
- no goofy placeholder styling
- even if art is temporary, keep the tone consistent with the name Oyabaun

Project structure:
Create a monorepo with something like:
- /client for Rust wasm game client
- /relay for Go game relay
- /shared or /protocol for message schemas / docs if needed
- /infra for docker/dev setup
- /docs for architecture notes

What I want from you in phases:

Phase 1:
- Propose the architecture
- Explain the key technical decisions
- Define the protocol between client and relay
- Define which parts are authoritative
- Define the crate/package layout
- Define the MVP scope clearly
- Call out major risks and tradeoffs

Phase 2:
- Generate the actual codebase skeleton
- Set up the Rust wasm client scaffolding
- Set up the Go relay scaffolding
- Set up Docker Compose
- Set up local run instructions

Phase 3:
- Implement a playable local prototype
- A browser client that can connect to the relay
- basic login/auth bootstrap
- room join
- movement replication
- shooting and damage
- respawn and scoreboard

Phase 4:
- tighten code quality
- remove dead code
- improve ergonomics
- write concise docs
- add tests where practical for protocol / relay logic

Important implementation guidance:
- Make concrete choices for libraries and explain why
- Prefer battle-tested Rust crates and lightweight Go libraries
- Avoid overengineering
- Avoid giant abstractions
- Build the smallest thing that can actually become a real game
- Keep the relay specialized and opinionated
- Treat this as a serious product foundation, not a toy tutorial

Coding style:
- clean and minimal
- no unnecessary comments
- no fake placeholder enterprise patterns
- no XML
- no massive framework magic
- keep functions and modules purposeful
- favor readability and performance

Output format:
Start by giving me:
1. a concise architecture overview
2. the protocol design
3. the monorepo structure
4. the MVP milestone plan
5. then begin generating the actual codebase files

Do not stop at high-level ideas. I want you to actually start building the project.