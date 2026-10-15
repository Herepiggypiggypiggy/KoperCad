# Golden samples for the maker recipes

One folder per maker recipe, each with a `sample.*` input and a recorded
`sample.expected.json`. `tests/maker_packs.rs` imports the sample through the
real import path and compares the result with the recorded one.

**These samples are synthetic.** No network access to the makers was available
when they were written, and a maker's own export files are covered by that
maker's terms, so none is copied here. Each sample is written by hand in the
format the maker publishes it in (BMEcat 2005 XML, or a CSV table, German ones
in Windows-1252 with semicolons) using the feature wording that maker's
public datasheets use, and technical values that are plain published facts
(a 2.5 mm² terminal block carries 24 A). The recipes are therefore tested
against the format and the wording, not against a real export. Expect to
correct a mapping the first time a real file shows a different name.

## Adding a real file

1. Download the file yourself from the maker (see the steps the Parts window
   shows for that maker).
2. Remove or replace anything you may not republish: keep a handful of rows,
   and make sure the maker's terms allow keeping them in a public repository.
   If they do not, keep the file on your machine only and run the importer on
   it to see the report (the Parts window shows the same lines).
3. Save it as `golden/<maker>/sample.xml` (or `.csv`) and run
   `UPDATE_GOLDEN=1 cargo test -p novo-parts --test maker_packs`.
4. Read the diff of `sample.expected.json`: the `skipped` and `unmapped` lists
   say which of the maker's features the recipe did not understand. Add a
   phrase to the rule table in `src/recipes/apply.rs` for each one that
   matters, run again, and read the diff again.

A new recipe needs a folder here: the test fails for a recipe without a sample.
