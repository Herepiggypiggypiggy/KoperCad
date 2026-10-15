//! The standard function blocks of IEC 61131-3 part 3: timers, counters,
//! edge detectors and bistables. Each declared instance keeps its own state.

use super::value::{DataType, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FbKind {
    Ton,
    Tof,
    Tp,
    Ctu,
    Ctd,
    Ctud,
    RTrig,
    FTrig,
    Rs,
    Sr,
}

/// A parameter of a block: inputs are set by the caller, outputs are read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    In(usize),
    Out(usize),
}

impl FbKind {
    pub fn parse(name: &str) -> Option<FbKind> {
        Some(match name.to_ascii_uppercase().as_str() {
            "TON" => FbKind::Ton,
            "TOF" => FbKind::Tof,
            "TP" => FbKind::Tp,
            "CTU" => FbKind::Ctu,
            "CTD" => FbKind::Ctd,
            "CTUD" => FbKind::Ctud,
            "R_TRIG" => FbKind::RTrig,
            "F_TRIG" => FbKind::FTrig,
            "RS" => FbKind::Rs,
            "SR" => FbKind::Sr,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            FbKind::Ton => "TON",
            FbKind::Tof => "TOF",
            FbKind::Tp => "TP",
            FbKind::Ctu => "CTU",
            FbKind::Ctd => "CTD",
            FbKind::Ctud => "CTUD",
            FbKind::RTrig => "R_TRIG",
            FbKind::FTrig => "F_TRIG",
            FbKind::Rs => "RS",
            FbKind::Sr => "SR",
        }
    }

    /// Input parameters with their types, in declaration order.
    pub fn inputs(self) -> &'static [(&'static str, DataType)] {
        use DataType::{Bool, Int, Time};
        match self {
            FbKind::Ton | FbKind::Tof | FbKind::Tp => &[("IN", Bool), ("PT", Time)],
            FbKind::Ctu => &[("CU", Bool), ("R", Bool), ("PV", Int)],
            FbKind::Ctd => &[("CD", Bool), ("LD", Bool), ("PV", Int)],
            FbKind::Ctud => &[
                ("CU", Bool),
                ("CD", Bool),
                ("R", Bool),
                ("LD", Bool),
                ("PV", Int),
            ],
            FbKind::RTrig | FbKind::FTrig => &[("CLK", Bool)],
            FbKind::Rs => &[("S", Bool), ("R1", Bool)],
            FbKind::Sr => &[("S1", Bool), ("R", Bool)],
        }
    }

    pub fn outputs(self) -> &'static [(&'static str, DataType)] {
        use DataType::{Bool, Int, Time};
        match self {
            FbKind::Ton | FbKind::Tof | FbKind::Tp => &[("Q", Bool), ("ET", Time)],
            FbKind::Ctu | FbKind::Ctd => &[("Q", Bool), ("CV", Int)],
            FbKind::Ctud => &[("QU", Bool), ("QD", Bool), ("CV", Int)],
            FbKind::RTrig | FbKind::FTrig => &[("Q", Bool)],
            FbKind::Rs | FbKind::Sr => &[("Q1", Bool)],
        }
    }

    pub fn field(self, name: &str) -> Option<Field> {
        let find = |list: &[(&str, DataType)]| {
            list.iter()
                .position(|(n, _)| n.eq_ignore_ascii_case(name))
        };
        find(self.inputs())
            .map(Field::In)
            .or_else(|| find(self.outputs()).map(Field::Out))
    }

    /// The input a ladder box feeds with power flow.
    pub fn power_input(self) -> usize {
        0
    }

    /// The output that carries power flow on to the right of a ladder box.
    pub fn power_output(self) -> usize {
        0
    }
}

/// The state of one declared block instance.
#[derive(Debug, Clone, PartialEq)]
pub struct FbState {
    pub kind: FbKind,
    pub inputs: Vec<Value>,
    pub outputs: Vec<Value>,
    prev: bool,
    prev2: bool,
    start: i64,
    running: bool,
}

impl FbState {
    pub fn new(kind: FbKind) -> Self {
        FbState {
            kind,
            inputs: kind.inputs().iter().map(|(_, t)| t.zero()).collect(),
            outputs: kind.outputs().iter().map(|(_, t)| t.zero()).collect(),
            prev: false,
            prev2: false,
            start: 0,
            running: false,
        }
    }

    fn b(&self, i: usize) -> bool {
        self.inputs[i].as_bool()
    }

    fn set_out(&mut self, i: usize, v: Value) {
        self.outputs[i] = v;
    }

    /// Runs the block once with the clock at `now` microseconds.
    pub fn run(&mut self, now: i64) {
        match self.kind {
            FbKind::Ton => self.ton(now),
            FbKind::Tof => self.tof(now),
            FbKind::Tp => self.tp(now),
            FbKind::Ctu => {
                let (cu, r) = (self.b(0), self.b(1));
                let pv = self.inputs[2].as_int();
                let mut cv = self.outputs[1].as_int();
                if r {
                    cv = 0;
                } else if cu && !self.prev && cv < i64::from(i16::MAX) {
                    cv += 1;
                }
                self.prev = cu;
                self.set_out(0, Value::Bool(cv >= pv));
                self.set_out(1, Value::Int(cv));
            }
            FbKind::Ctd => {
                let (cd, ld) = (self.b(0), self.b(1));
                let pv = self.inputs[2].as_int();
                let mut cv = self.outputs[1].as_int();
                if ld {
                    cv = pv;
                } else if cd && !self.prev && cv > i64::from(i16::MIN) {
                    cv -= 1;
                }
                self.prev = cd;
                self.set_out(0, Value::Bool(cv <= 0));
                self.set_out(1, Value::Int(cv));
            }
            FbKind::Ctud => {
                let (cu, cd, r, ld) = (self.b(0), self.b(1), self.b(2), self.b(3));
                let pv = self.inputs[4].as_int();
                let mut cv = self.outputs[2].as_int();
                if r {
                    cv = 0;
                } else if ld {
                    cv = pv;
                } else {
                    let up = cu && !self.prev;
                    let down = cd && !self.prev2;
                    if up && !down && cv < i64::from(i16::MAX) {
                        cv += 1;
                    } else if down && !up && cv > i64::from(i16::MIN) {
                        cv -= 1;
                    }
                }
                self.prev = cu;
                self.prev2 = cd;
                self.set_out(0, Value::Bool(cv >= pv));
                self.set_out(1, Value::Bool(cv <= 0));
                self.set_out(2, Value::Int(cv));
            }
            FbKind::RTrig => {
                let clk = self.b(0);
                self.set_out(0, Value::Bool(clk && !self.prev));
                self.prev = clk;
            }
            FbKind::FTrig => {
                let clk = self.b(0);
                self.set_out(0, Value::Bool(!clk && self.prev));
                self.prev = clk;
            }
            // Reset dominates.
            FbKind::Rs => {
                let q = (self.b(0) || self.outputs[0].as_bool()) && !self.b(1);
                self.set_out(0, Value::Bool(q));
            }
            // Set dominates.
            FbKind::Sr => {
                let q = self.b(0) || (self.outputs[0].as_bool() && !self.b(1));
                self.set_out(0, Value::Bool(q));
            }
        }
    }

    fn pt(&self) -> i64 {
        self.inputs[1].as_time().max(0)
    }

    fn ton(&mut self, now: i64) {
        let pt = self.pt();
        if self.b(0) {
            if !self.running {
                self.running = true;
                self.start = now;
            }
            let et = (now - self.start).min(pt);
            self.set_out(0, Value::Bool(et >= pt));
            self.set_out(1, Value::Time(et));
        } else {
            self.running = false;
            self.set_out(0, Value::Bool(false));
            self.set_out(1, Value::Time(0));
        }
    }

    fn tof(&mut self, now: i64) {
        let pt = self.pt();
        let input = self.b(0);
        if input {
            self.running = false;
            self.set_out(0, Value::Bool(true));
            self.set_out(1, Value::Time(0));
        } else {
            if self.prev && !self.running {
                self.running = true;
                self.start = now;
            }
            if self.running {
                let et = (now - self.start).min(pt);
                self.set_out(1, Value::Time(et));
                if et >= pt {
                    self.running = false;
                    self.set_out(0, Value::Bool(false));
                }
            }
        }
        self.prev = input;
    }

    fn tp(&mut self, now: i64) {
        let pt = self.pt();
        let input = self.b(0);
        if !self.running && input && !self.prev {
            self.running = true;
            self.start = now;
        }
        if self.running {
            let et = (now - self.start).min(pt);
            self.set_out(1, Value::Time(et));
            if et >= pt {
                self.running = false;
                self.set_out(0, Value::Bool(false));
                // ET holds at PT while IN stays high, else returns to 0.
                if !input {
                    self.set_out(1, Value::Time(0));
                }
            } else {
                self.set_out(0, Value::Bool(true));
            }
        } else if !input {
            self.set_out(1, Value::Time(0));
        }
        self.prev = input;
    }
}
