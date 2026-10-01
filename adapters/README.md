# Adapters

Host adapters live here (one crate each), e.g. `iw4l/`. An adapter translates
host input -> `MovementInput`, implements `CollisionWorld` over the host
physics, and writes `MovementState` back. See docs/ARCHITECTURE.md.
