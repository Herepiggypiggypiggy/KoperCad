//! Kitting and stock: what to take from the shelf for each build phase and
//! what to buy, in one list with the bill of materials.
//!
//! [`cutting`] is the one-dimensional cutting-stock engine (bars with a saw
//! kerf, offcuts held and kept, mixed bar lengths), [`KitSettings`] the stock
//! the shop buys and holds, [`kit_plan`] the plan for a project and
//! [`Kitting`] the report that prints it: a pick list per phase, the saw plan
//! per bar, the reels, the offcuts to keep and the merged shopping list.

pub mod cutting;
mod plan;
mod report;
mod settings;

pub use plan::{
    Basis, KEEP_REEL_MM, KitPlan, Phase, PickLine, ReelGroup, ReelRow, SawBar, SawCut, SawPlan,
    Shop, ShopLine, kit_plan, num as num_text,
};
pub use report::{KITTING_BOX, Kitting};
pub use settings::{KitSettings, OnHand, SETTINGS_KEY};
