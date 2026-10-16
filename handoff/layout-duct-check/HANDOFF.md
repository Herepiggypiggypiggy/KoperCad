# Handoff: layout duct check

Branch `claude/project-thread-sos9i9` from main (76bca5a). One PR; names where the user finds it: Cabinet tab status strip (problems pill), Issues panel ("Duct over rail or device"), Right-click plate > Place all.
Resume: `cargo test -p novo-layout --test duct_overlap`; app test `place_all_keeps_clear_of_a_duct_already_on_the_plate...` in cabinet_rules_tests.rs.
Trap: `cargo test -p novo-checks` alone fails to build novo-editor (novo_canvas) on main; build with -p novo-app together.
