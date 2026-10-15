use crate::model::Scenario;
use crate::result::*;
use crate::setup::Error;

pub fn recommend(_scn: &Scenario, _res: &ThermalResult) -> Result<Vec<Recommendation>, Error> {
    Ok(Vec::new())
}
