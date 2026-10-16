# Reviewer R5 progress
- 2026-10-16 09:50 READY: #597 e16377d, #599 278b103, #602 73ef748, #605 3be5b33, #608 419ce7a, #609 57b8ac2, #611 47c9bd9 (Windows GPU look still open), #591 144db92.
- 2026-10-16 09:50 CHANGES NEEDED: #612 a5e1c44 (Ctrl+Shift+R dropped in the Cabinet). Waiting for the new head SHA from the author.
- Not yet reviewed from the queue: #604, #600, #601 (R3 assigned; take if asked).
- 2026-10-16 09:50 #621 READY 54f1dec (docs); #623 READY 6d918a1 (CI pending); #612 READY 79786e4 (CI pending); #626 READY a8495c3 (CI pending).
- 2026-10-16 09:55 #629 READY cb7854d (CI pending); #600 READY ffcbe25; #604 CHANGES NEEDED 4b7190d (jump leaves Commissioning; lock follows outline tab). Waiting for new SHA.
- Not reviewed: #601 (R3).
- 2026-10-16 18:10 R5 verdicts since #629 (heads are full-SHA prefixes): READY #712 fe1d471, #714 f54f7cd, #717 9b69617, #729 a300a0f, #730 dc1a571, #738 cf35525, #739 f52b552, #747 3bdb6e7, #742 54840e77, #744 6cb1dcbe, #750 eb72ab16, #755 5c9a6ca4, #756 95df9b75 (merge after #748), #758 ec88aaea, #765 4fd9b6db. CHANGES NEEDED, waiting for new heads: #743 d843105 (GPU test passes without AO), #748 4f6425a (select-by-type silent before scene built), #757 510c595 (popup-open test, plate-only feedback), #762 e10d744 (AO radius uses view_proj[1][1]; one governor with #755), #764 0bf11ec (heat.simulate must open Heat panel). Merge-order note: #742 rewrites view3d_controls::toolbar as a Panel; #748/#750/#755/#758/#764/#765 conflict with it and each other.
- Not reviewed: #601 (R3); heat tools #753/#754/#760/#763/#767 go to another reviewer.
