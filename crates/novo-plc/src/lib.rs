//! PLC cards and channels (WS-17). Design: `docs/design/plc.md`.
//!
//! A card is a [`Device`](novo_core::Device) with
//! [`DeviceKind::Plc`](novo_core::DeviceKind::Plc); a channel is one of its
//! non-main symbols, exactly as a contact is one function of a contactor.
//! Everything PLC-specific lives in `plc.*` properties ([`card`] lists the
//! keys), so this crate needs no change to the model.
//!
//! - [`CardLayout`]: which channels a card has, parsed from `DI16 DQ8 AI4`.
//! - [`Card`] and [`cards`]: cards read from a project, with their stored
//!   start addresses and slot.
//! - [`allocate`] / [`next_start`]: a new card's start, after every card
//!   already in its station, so adding a card never moves an address the
//!   PLC program uses.
//! - [`Address`]: a channel's address, always derived from its card's start
//!   and its place in the layout, written in the project's [`AddressStyle`].
//! - [`PlcIndex`]: which symbol draws which channel (`plc.channel = "DI 3"` on a
//!   non-main symbol of the card), address labels, free channels and the
//!   next one to place, and the [`Issue`]s the design checks report.
//! - [`commands`]: add a card, place the next channel, `to plc` and
//!   `renumber io`, each one transaction of existing model commands.
//! - [`PlcIndex::io_rows`]: the I/O list, one [`IoRow`] per channel with
//!   spares, for the `io` report and the Devices drawer.
//!
//! ```
//! use novo_plc::{AddressStyle, CardLayout, ChannelType, Space};
//!
//! let layout: CardLayout = "DI16 AI2".parse()?;
//! assert_eq!(layout.bytes(Space::Input), 6);
//! let ai1 = layout.channel(ChannelType::AnalogInput, 1).unwrap();
//! let address = novo_plc::Address::of(&ai1, 64, Some(3));
//! assert_eq!(address.format(AddressStyle::Siemens), "%IW68");
//! # Ok::<(), novo_plc::LayoutError>(())
//! ```

pub mod address;
pub mod card;
pub mod commands;
pub mod generate;
pub mod index;
pub mod io;
pub mod layout;
pub mod link;

pub use address::{Address, AddressStyle};
pub use card::{Card, LayoutSource, address_style, allocate, cards, next_start};
pub use commands::{RenumberPlan, StartMove};
pub use index::{
    DrawnChannel, FromKey, Functions, Issue, PlcIndex, READS, channel_key, parse_channel,
};
pub use io::IoRow;
pub use layout::{CardLayout, Channel, ChannelType, Group, LayoutError, Space};
