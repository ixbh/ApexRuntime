//! # apex-core
//!
//! Engine-independent player movement for ApexRuntime.
//!
//! ```text
//! MovementState + MovementInput + &impl CollisionWorld + dt  ->  MovementState
//! ```
//!
//! Nothing here knows about rendering, ECS, windows, maps or any game. Hosts
//! (the sandbox, future IW4L/Unity/... adapters) own the loop and the world,
//! translate their input into [`MovementInput`], implement
//! [`apex_collision::CollisionWorld`], and copy the resulting state back.
//!
//! Phase 1 rules are Source-lineage placeholders (see `docs/MOVEMENT-MODEL.md`).
//! They prove the architecture; they are **not** yet Apex behaviour.

pub mod clock;
pub mod collide;
pub mod config;
pub mod input;
pub mod sheet;
pub mod state;
pub mod step;

pub use apex_collision::{CollisionWorld, Trace};
pub use apex_math::{Aabb, Vec3};
pub use clock::FixedClock;
pub use config::MovementConfig;
pub use input::{Buttons, MovementInput};
pub use state::{MoveMode, MovementState};
pub use step::{StepReport, step, step_with_report};
