# Evidence PR 1874 (merge of main)
Where the user finds it: Check > Calculations > Busbars tab. After the merge the tabs read Circuits, Motor starters, Panel SCCR, Busbars, Mechanical (Mechanical from main keeps working; its scenario passes).
Code is unchanged from 570c395b except the merge (tab order, model label rows, anchors doc). Earlier evidence for the sweep worker, error text and "?" changes: see the 570c395b notes in the PR conversation.
Not tested: Windows, real busbars against a measurement, an actual worker panic in the GUI, full workspace tests (CI).
