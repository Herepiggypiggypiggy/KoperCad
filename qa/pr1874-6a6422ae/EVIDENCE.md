# Evidence PR 1874 (merge of main at 6a6422ae)
Where the user finds it: Check > Calculations > Busbar impedance tab (renamed from Busbars; main's Busbar heat tab sits beside it).
Code is unchanged from the reviewed fe9bc688 except the merge, the tab rename and the anchor id calc.tab.busbar_impedance.
Not in this PR: the Mechanical and Busbar heat tabs belong to other threads (R6's text-behind-? asks for Mechanical go to that owner). No picture of Busbar heat in this folder beyond the tab label in the row.
Not tested: Windows, real busbars against a measurement, an actual worker panic in the GUI, full workspace tests (CI).
