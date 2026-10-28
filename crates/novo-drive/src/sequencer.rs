//! Contactor sequencing of a two-speed pole-change motor (lane L2a).
//!
//! Rules (a model of the usual interlocked contactor chain, times are user inputs):
//! - step 1 closes the fine contactor, step 2 the coarse contactor;
//! - from standstill step 2 starts on fine and goes to coarse after `fine_first_s`
//!   (slow before fast);
//! - fine and coarse are never closed together: every change between them, and every
//!   change of direction, passes an open dead time;
//! - releasing step 2 back to step 1 drops coarse and closes fine; releasing step 1 opens all.

use crate::error::{self, InputError};
use crate::motor::{Direction, WindingSel};

/// What the operator asks for: the rocker step (0 = off, 1 = slow, 2 = fast) and direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedStep {
    Off,
    One,
    Two,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    pub direction: Direction,
    pub step: SpeedStep,
}

impl Command {
    pub const OFF: Self = Self {
        direction: Direction::Positive,
        step: SpeedStep::Off,
    };
}

/// The winding that is fed and the sense of its field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Energised {
    pub winding: WindingSel,
    pub direction: Direction,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SequencerConfig {
    /// Open time between fine and coarse (either way), in seconds.
    pub changeover_dead_time_s: f64,
    /// Time on fine before coarse takes over when step 2 is given from standstill.
    pub fine_first_s: f64,
    /// Open time when the direction is reversed.
    pub reversal_dead_time_s: f64,
}

impl SequencerConfig {
    pub fn validate(&self) -> Result<(), InputError> {
        error::non_negative("changeover_dead_time_s", self.changeover_dead_time_s)?;
        error::non_negative("fine_first_s", self.fine_first_s)?;
        error::non_negative("reversal_dead_time_s", self.reversal_dead_time_s)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sequencer {
    config: SequencerConfig,
    active: Option<Energised>,
    dead_remaining_s: f64,
    fine_hold_remaining_s: f64,
    /// The fine-first period is over for this press of step 2.
    fine_done: bool,
}

impl Sequencer {
    pub fn new(config: SequencerConfig) -> Result<Self, InputError> {
        config.validate()?;
        Ok(Self {
            config,
            active: None,
            dead_remaining_s: 0.0,
            fine_hold_remaining_s: 0.0,
            fine_done: false,
        })
    }

    pub fn energised(&self) -> Option<Energised> {
        self.active
    }

    /// Advance by `dt_s` with the operator command; returns what is fed during this step.
    pub fn step(&mut self, command: Command, dt_s: f64) -> Option<Energised> {
        let want = match command.step {
            SpeedStep::Off => None,
            SpeedStep::One => Some(Energised {
                winding: WindingSel::Fine,
                direction: command.direction,
            }),
            SpeedStep::Two => Some(Energised {
                winding: WindingSel::Coarse,
                direction: command.direction,
            }),
        };
        let Some(want) = want else {
            self.active = None;
            self.dead_remaining_s = 0.0;
            self.fine_done = false;
            return None;
        };
        if self.dead_remaining_s > 0.0 {
            self.dead_remaining_s = (self.dead_remaining_s - dt_s).max(0.0);
            if self.dead_remaining_s > 0.0 {
                self.active = None;
                return None;
            }
        }
        match self.active {
            None => {
                if want.winding == WindingSel::Coarse && !self.fine_done {
                    self.fine_hold_remaining_s = self.config.fine_first_s;
                    self.active = Some(Energised {
                        winding: WindingSel::Fine,
                        direction: want.direction,
                    });
                } else {
                    self.active = Some(want);
                }
            }
            Some(now) if now.direction != want.direction => {
                self.active = None;
                self.dead_remaining_s = self.config.reversal_dead_time_s;
                self.fine_done = false;
            }
            Some(now) if now.winding == want.winding => {}
            Some(now) => {
                // Fine -> coarse waits for the fine-first period; coarse -> fine is immediate.
                let leave = if now.winding == WindingSel::Fine && !self.fine_done {
                    self.fine_hold_remaining_s -= dt_s;
                    self.fine_hold_remaining_s <= 0.0
                } else {
                    true
                };
                if leave {
                    self.fine_done = want.winding == WindingSel::Coarse || self.fine_done;
                    self.active = None;
                    self.dead_remaining_s = self.config.changeover_dead_time_s;
                    if self.dead_remaining_s == 0.0 {
                        self.active = Some(want);
                    }
                }
            }
        }
        self.active
    }
}
