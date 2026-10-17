# ui-protection-sccr handoff

- `novo-checks/src/protection/model.rs`: reads `current_limiting`, `fuse_class`, `let_through` (`rms A : peak A : A2s` points separated by `;`), `withstand_peak_ka`, `withstand_i2t` from the device (or its part) and passes them to the SCCR engine (`sccr.rs`, unchanged). `parse_let_through` / `format_let_through` are the text form.
- `novo-app/src/app/protection.rs`: the kA box saves on release through `set_info_property`; "Current-limiter credit" section in Panel rating writes device properties with `Tx::set_property`. No new command, no novo-core change.
- The credit is a planning value from the designer's maker data, never a certified rating. A limiter's own holder rating (default 10 kA) still caps the credit unless `sccr_ka` is marked on it.
