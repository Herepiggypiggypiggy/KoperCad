# Handoff: Place all places every device

Branch `claude/place-all-every-device`, from main. Cause: `novo_layout::model::candidates` only took devices whose tag location equals the enclosure's, so `-X1` (made on the terminal screen without a location) was in no cabinet's list and nothing said so. Now `candidates` takes location-less devices when the enclosure is the only one; `without_enclosure` lists them when there are several, and Place all warns "Not placed: -X1 (...)" (`left_out_text` in `cabinet.rs`), also for devices that do not fit, with `UnplacedWhy::text`. The Place all button is enabled while such devices exist.

Tests: `novo-layout/tests/model.rs` and `cabinet_tests.rs` (two tests).
